# wiretrail — Smart Summary & `auto` Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `diagnose`'s composition into a shared `recommender` core, surface its ranked recommendations inside `summary`, and add an `auto` command that runs the top recommendations and inlines their full scoped output.

**Architecture:** A pure lib module `recommender` produces `Vec<Recommendation>` by running the existing `compute_*` analyses (this is `diagnose`'s current body). `diagnose` is refactored to render over it (output unchanged — the parity guard). `summary` gains a `recommendations` field + a rendered section. `auto` lives in `main.rs`, reuses `compute_summary` (which now carries the recommendations) and a bounded drill-down executor that dispatches each recommendation's `command` to the matching `compute_+render_`.

**Tech Stack:** Rust 2024, serde / serde_json, clap, ahash. No new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-smart-summary-auto-design.md`.
New command count: **32**.

## Prerequisites (verified against source)

- `diagnose` body (the recommender source): `src/analysis/diagnose.rs` builds `Vec<Diagnosis>` from `errors`/`auth`/`rate_limit`/`retries`/`storms`/`duplicates`/`redirects`/`slowest`, sorts by `sev_rank` desc → evidence-len desc → kind asc, truncates to `top`. Only the `5xx-cluster` finding carries a filter today (`errors --filter "host:{host}"`); every other `suggested_command` is the bare command name.
- Target command signatures:
  - `errors::compute_errors(cap, filter, top, unsafe_include) -> ErrorsResult` + `render_errors_text`
  - `auth::compute_auth(cap, filter, top) -> AuthResult` + `render_auth_text`
  - `rate_limit::compute_rate_limit(cap, filter, top) -> RateLimitResult` + `render_rate_limit_text`
  - `retries::compute_retries(cap, filter, top) -> RetriesResult` + `render_retries_text`
  - `storms::compute_storms(cap, filter, window_ms, min_count, top) -> StormsResult` + `render_storms_text` (diagnose uses `1000, 5`)
  - `diff::compute_diff(cap, filter, top, unsafe_include) -> DiffResult` + `render_diff_text`
  - `redirects::compute_redirects(cap, filter, top) -> RedirectsResult` + `render_redirects_text`
  - `slowest::compute_slowest(cap, filter, top) -> SlowestResult` + `render_slowest_text`
- `main.rs` already imports every `compute_*`/`render_*`. It has `SeverityArg` (clap `ValueEnum` Critical/High/Medium/Low with `as_str() -> &'static str` lowercase, added in M7), `emit`/`exit` helpers, `Envelope::new(command, meta, result).with_next_commands(...)` + `.to_json()`, `Filter::parse(&[String])`, and builds `cap: har::model::Capture` via `assemble(load(...))`. The `report` arm (main.rs ~434) is the precedent for a command that builds its own `Envelope` and prints text/JSON itself instead of using `emit`.
- `summary` arm in `main.rs` uses `emit(... "summary" ... &["duplicates","errors","slowest"])`.
- `ErrorGroup.host`, `RetryGroup`/`DuplicateGroup` fields per diagnose's current usage.

## File structure

```
src/recommender.rs        # NEW (lib): Recommendation + recommend() + sev_rank()
src/lib.rs                # MODIFY: pub mod recommender;
src/analysis/diagnose.rs  # MODIFY: compute_diagnose renders over recommend() (output identical)
src/analysis/summary.rs   # MODIFY: recommendations field + render section
src/main.rs               # MODIFY: dynamic summary footer; Auto subcommand + executor + JSON
tests/cli_auto.rs         # NEW: integration tests
```

---

### Task 1: Extract the `recommender` core; refactor `diagnose` onto it

**Files:**
- Create: `src/recommender.rs`
- Modify: `src/lib.rs`, `src/analysis/diagnose.rs`

- [ ] **Step 1: Declare the module.** In `src/lib.rs`, the `pub mod` list is alphabetical (`… raw; redact; render; …`). `recommender` sorts between `raw` and `redact`, so insert it directly **before** the `pub mod redact;` line:

```rust
pub mod recommender;
```

- [ ] **Step 2: Write the failing tests** at the top of `src/recommender.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::recommend;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn err(index: usize, path: &str, status: i64, off: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", path, status);
        e.started_offset_ms = off;
        e
    }

    #[test]
    fn surfaces_5xx_cluster_as_high_with_host_filter() {
        let cap = sample_capture(vec![
            err(0, "/bulk", 500, 0.0),
            err(1, "/bulk", 500, 10.0),
            err(2, "/bulk", 500, 20.0),
        ]);
        let recs = recommend(&cap, &Filter::parse(&[]).unwrap(), 20);
        let top = &recs[0];
        assert_eq!(top.severity, "high");
        assert_eq!(top.kind, "5xx-cluster");
        assert_eq!(top.command, "errors");
        assert_eq!(top.filter.as_deref(), Some("host:api.x"));
        assert_eq!(top.command_line(), "errors --filter \"host:api.x\"");
    }

    #[test]
    fn clean_capture_yields_no_recommendations() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/ok", 200)]);
        assert!(recommend(&cap, &Filter::parse(&[]).unwrap(), 20).is_empty());
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib recommender 2>&1 | tail -10`
Expected: FAIL with "cannot find function `recommend`".

- [ ] **Step 4: Implement `src/recommender.rs`** above the test module. This is `compute_diagnose`'s body verbatim, pushing `Recommendation` instead of `Diagnosis` (split `suggested_command` into `command` + `filter`):

```rust
use crate::analysis::{auth, duplicates, errors, rate_limit, redirects, retries, slowest, storms};
use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub severity: String, // "critical" | "high" | "medium" | "low"
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub command: String,        // drill-down subcommand
    pub filter: Option<String>, // scoping filter expression, if any
}

impl Recommendation {
    /// The reproducing command tail, e.g. `errors --filter "host:api.x"` or `auth`.
    pub fn command_line(&self) -> String {
        match &self.filter {
            Some(f) => format!("{} --filter \"{}\"", self.command, f),
            None => self.command.clone(),
        }
    }
}

/// Severity ordering shared across the recommender, diagnose, summary, and auto.
pub fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

/// Rank actionable recommendations by composing the existing analyses.
pub fn recommend(cap: &Capture, filter: &Filter, top: usize) -> Vec<Recommendation> {
    let mut f: Vec<Recommendation> = Vec::new();

    // 5xx clusters / 4xx groups
    for g in errors::compute_errors(cap, filter, top, false).groups {
        if (500..600).contains(&g.status) && g.count >= 3 {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "5xx-cluster".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "server error cluster".into()),
                evidence_ids: g.entry_ids.clone(),
                command: "errors".into(),
                filter: Some(format!("host:{}", g.host)),
            });
        } else if (400..500).contains(&g.status) {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "4xx".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "client error".into()),
                evidence_ids: g.entry_ids.clone(),
                command: "errors".into(),
                filter: None,
            });
        }
    }

    // auth: refresh races + failures
    let a = auth::compute_auth(cap, filter, top);
    for rf in &a.refreshes {
        if rf.old_token_reused || !rf.success {
            let why = if rf.old_token_reused {
                "refresh succeeded but later calls reused the old token"
            } else {
                "token refresh failed"
            };
            let mut ids = vec![rf.id.clone()];
            ids.extend(rf.reusing_ids.clone());
            f.push(Recommendation {
                severity: "high".into(),
                kind: "token-refresh-race".into(),
                title: format!("suspicious token refresh on {}", rf.host),
                detail: why.into(),
                evidence_ids: ids,
                command: "auth".into(),
                filter: None,
            });
        }
    }
    if !a.failures.is_empty() {
        let total: usize = a.failures.iter().map(|x| x.count).sum();
        let ids: Vec<String> = a.failures.iter().flat_map(|x| x.entry_ids.clone()).collect();
        f.push(Recommendation {
            severity: "medium".into(),
            kind: "auth-failures".into(),
            title: format!("{total} auth failures (401/403)"),
            detail: "requests rejected for authentication/authorization".into(),
            evidence_ids: ids,
            command: "auth".into(),
            filter: None,
        });
    }

    // rate-limit without backoff
    for g in rate_limit::compute_rate_limit(cap, filter, top).groups {
        if g.cooldown_violated {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "rate-limit-no-backoff".into(),
                title: format!("calls during 429 cooldown on {} {}", g.host, g.norm_path),
                detail: format!("{} 429s, follow-ups before Retry-After elapsed", g.count_429),
                evidence_ids: g.entry_ids.clone(),
                command: "rate-limit".into(),
                filter: None,
            });
        }
    }

    // retry exhaustion
    for g in retries::compute_retries(cap, filter, top).groups {
        if g.retry_count >= 3 && !(200..300).contains(&g.final_status) {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "retry-exhaustion".into(),
                title: format!(
                    "{} retries, final {} on {} {}",
                    g.retry_count, g.final_status, g.method, g.norm_path
                ),
                detail: "repeated retries did not recover".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "retries".into(),
                filter: None,
            });
        }
    }

    // request storms
    for s in storms::compute_storms(cap, filter, 1000, 5, top).storms {
        if s.peak_count >= 10 {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "request-storm".into(),
                title: format!("{} {} calls/s burst to {}", s.peak_count, s.scope_kind, s.scope),
                detail: "burst of calls in a 1s window".into(),
                evidence_ids: s.entry_ids.clone(),
                command: "storms".into(),
                filter: None,
            });
        }
    }

    // wasteful duplicates (not retries)
    for g in duplicates::compute_duplicates(cap, filter, top).groups {
        if g.count >= 10 && !g.is_retry_pattern {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "wasteful-duplicates".into(),
                title: format!("{}x identical {} {}", g.count, g.method, g.norm_path),
                detail: "repeated identical calls (not retries)".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "diff".into(),
                filter: None,
            });
        }
    }

    // redirect storms
    for g in redirects::compute_redirects(cap, filter, top).groups {
        if g.is_storm {
            f.push(Recommendation {
                severity: "low".into(),
                kind: "redirect-storm".into(),
                title: format!("{}x [{}] redirect on {} {}", g.count, g.status, g.host, g.norm_path),
                detail: "repeated redirects".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "redirects".into(),
                filter: None,
            });
        }
    }

    // slow backend
    if let Some(s) = slowest::compute_slowest(cap, filter, top).entries.first()
        && s.duration_ms > 1000.0
        && s.bottleneck == "server wait/TTFB"
    {
        f.push(Recommendation {
            severity: "low".into(),
            kind: "slow-backend".into(),
            title: format!(
                "slowest call {}ms on {} {}",
                s.duration_ms as i64, s.host, s.norm_path
            ),
            detail: "dominated by server wait (TTFB)".into(),
            evidence_ids: vec![s.id.clone()],
            command: "slowest".into(),
            filter: None,
        });
    }

    f.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.evidence_ids.len().cmp(&a.evidence_ids.len()))
            .then(a.kind.cmp(&b.kind))
    });
    f.truncate(top);
    f
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib recommender 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 6: Refactor `diagnose` onto the core.** Replace the entire body of `compute_diagnose` in `src/analysis/diagnose.rs` (the function, not the structs/`render`/tests) with this adapter, and replace the file's first `use` line and the private `sev_rank`:

Change the imports at the top of `src/analysis/diagnose.rs` from:

```rust
use crate::analysis::{auth, duplicates, errors, rate_limit, redirects, retries, slowest, storms};
use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;
```

to:

```rust
use crate::filter::Filter;
use crate::model::Capture;
use crate::recommender::{Recommendation, recommend};
use serde::Serialize;
```

Delete the private `fn sev_rank(...)` from `diagnose.rs` (it now lives in `recommender`; `render_diagnose_text` does not use it). Replace the whole `compute_diagnose` function with:

```rust
/// Synthesize ranked root-cause findings (renders over the shared recommender).
pub fn compute_diagnose(cap: &Capture, filter: &Filter, top: usize) -> DiagnoseResult {
    let findings = recommend(cap, filter, top)
        .into_iter()
        .map(|r: Recommendation| Diagnosis {
            severity: r.severity,
            kind: r.kind,
            title: r.title,
            detail: r.detail,
            evidence_ids: r.evidence_ids,
            suggested_command: r.command_line(),
        })
        .collect();
    DiagnoseResult { findings }
}
```

Keep `DiagnoseResult`, `Diagnosis`, `render_diagnose_text`, and the existing `#[cfg(test)] mod tests` exactly as they are.

- [ ] **Step 7: Verify diagnose parity (the regression guard)**

Run: `cargo test --lib analysis::diagnose 2>&1 | tail -8 && cargo test --test cli_diagnose 2>&1 | tail -8`
Expected: all existing diagnose unit + CLI tests PASS unchanged. (`surfaces_5xx_cluster_as_high`, `clean_capture_has_no_findings`, `diagnose_json_envelope`.)

- [ ] **Step 8: Commit**

```bash
git add src/lib.rs src/recommender.rs src/analysis/diagnose.rs
git commit -m "refactor: extract recommender core from diagnose (output unchanged)"
```

---

### Task 2: Surface recommendations in `summary`

**Files:**
- Modify: `src/analysis/summary.rs`

- [ ] **Step 1: Add the failing test.** In `src/analysis/summary.rs`, inside `mod tests`, after `filter_reduces_filtered_count`, add:

```rust
    #[test]
    fn populates_recommendations_when_errors_present() {
        use crate::model::{sample_capture, sample_entry};
        let cap = sample_capture(vec![
            sample_entry(0, "api.x", "POST", "/bulk", 500),
            sample_entry(1, "api.x", "POST", "/bulk", 500),
            sample_entry(2, "api.x", "POST", "/bulk", 500),
        ]);
        let s = compute_summary(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(s.recommendations.iter().any(|r| r.kind == "5xx-cluster"));
        let text = super::render_summary_text(&s);
        assert!(text.contains("recommended next steps"));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::summary::tests::populates_recommendations_when_errors_present 2>&1 | tail -10`
Expected: FAIL — `no field recommendations on type SummaryResult`.

- [ ] **Step 3: Add the field.** In `src/analysis/summary.rs`, add the import near the top (after `use crate::model::Capture;`):

```rust
use crate::recommender::{Recommendation, recommend};
```

Add the field to `SummaryResult` (after `pub hints: Vec<String>,`):

```rust
    pub recommendations: Vec<Recommendation>,
```

- [ ] **Step 4: Populate it.** In `compute_summary`, change the final `SummaryResult { ... }` construction to compute and include recommendations. Add this line just before the `SummaryResult {` literal:

```rust
    let recommendations = recommend(cap, filter, top);
```

and add `recommendations,` as the last field in the `SummaryResult { ... }` literal (after `hints,`).

- [ ] **Step 5: Render the section.** In `render_summary_text`, immediately before the final `out` return, add:

```rust
    if !s.recommendations.is_empty() {
        out.push_str("\nrecommended next steps:\n");
        for r in &s.recommendations {
            out.push_str(&format!(
                "  [{}] {}\n         {} — {}\n",
                r.severity.to_ascii_uppercase(),
                r.command_line(),
                r.title,
                r.detail
            ));
        }
    }
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib analysis::summary 2>&1 | tail -8`
Expected: PASS (3 tests — the two existing plus the new one).

- [ ] **Step 7: Commit**

```bash
git add src/analysis/summary.rs
git commit -m "feat: surface ranked recommendations in summary"
```

---

### Task 3: `auto` command — drill-down executor + text output

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, add (alongside the other `har::` imports):

```rust
use har::model::Capture;
use har::recommender::{Recommendation, recommend, sev_rank};
```

(`compute_summary`/`render_summary_text` and every drill-down `compute_*`/`render_*` are already imported.)

- [ ] **Step 2: Add the two executor helpers.** In `src/main.rs`, add these free functions next to `emit`/`exit` (near the bottom, before the closing of the file):

```rust
/// Render one recommended drill-down command's full text output, scoped by `filter`.
fn drilldown_text(
    cmd: &str,
    cap: &Capture,
    filter: &Filter,
    top: usize,
    unsafe_include: bool,
) -> String {
    match cmd {
        "errors" => render_errors_text(&compute_errors(cap, filter, top, unsafe_include)),
        "auth" => render_auth_text(&compute_auth(cap, filter, top)),
        "rate-limit" => render_rate_limit_text(&compute_rate_limit(cap, filter, top)),
        "retries" => render_retries_text(&compute_retries(cap, filter, top)),
        "storms" => render_storms_text(&compute_storms(cap, filter, 1000, 5, top)),
        "diff" => render_diff_text(&compute_diff(cap, filter, top, unsafe_include)),
        "redirects" => render_redirects_text(&compute_redirects(cap, filter, top)),
        "slowest" => render_slowest_text(&compute_slowest(cap, filter, top)),
        _ => String::new(),
    }
}

/// Serialize one drill-down command's result object as JSON (for `auto --json`).
fn drilldown_json(
    cmd: &str,
    cap: &Capture,
    filter: &Filter,
    top: usize,
    unsafe_include: bool,
) -> serde_json::Value {
    use serde_json::to_value;
    let v = match cmd {
        "errors" => to_value(compute_errors(cap, filter, top, unsafe_include)),
        "auth" => to_value(compute_auth(cap, filter, top)),
        "rate-limit" => to_value(compute_rate_limit(cap, filter, top)),
        "retries" => to_value(compute_retries(cap, filter, top)),
        "storms" => to_value(compute_storms(cap, filter, 1000, 5, top)),
        "diff" => to_value(compute_diff(cap, filter, top, unsafe_include)),
        "redirects" => to_value(compute_redirects(cap, filter, top)),
        "slowest" => to_value(compute_slowest(cap, filter, top)),
        _ => Ok(serde_json::Value::Null),
    };
    v.unwrap_or(serde_json::Value::Null)
}

/// Build the Filter for a drill-down: the global `--filter` clauses AND the
/// recommendation's own scoping clause (if any). Falls back to the global filter
/// alone if the combined expression somehow fails to parse.
fn scoped_filter(global_clauses: &[String], rec: &Recommendation) -> Filter {
    let mut clauses = global_clauses.to_vec();
    if let Some(f) = &rec.filter {
        clauses.push(f.clone());
    }
    match Filter::parse(&clauses) {
        Ok(f) => f,
        Err(_) => Filter::parse(global_clauses).expect("global filter already validated"),
    }
}
```

- [ ] **Step 3: Add the `Auto` subcommand variant.** In `src/main.rs`, inside `enum Command { ... }`, after the `Rules { ... }` variant (the last one), add:

```rust
    /// Smart one-shot: summary + auto-drill the top recommendations inline.
    Auto {
        /// Drill into every triggered recommendation, including LOW.
        #[arg(long)]
        all: bool,
        /// Only drill recommendations at or above this severity (default: medium).
        #[arg(long = "min-severity", value_enum)]
        min_severity: Option<SeverityArg>,
    },
```

- [ ] **Step 4: Add the dispatch arm (text only for now).** In `src/main.rs`, inside the `match`, after the `Command::Rules { .. }` arm, add:

```rust
        Command::Auto { all, min_severity } => {
            let summary = compute_summary(&cap, &filter, cli.top);
            let floor = if all {
                "low"
            } else {
                min_severity.map(|s| s.as_str()).unwrap_or("medium")
            };
            let floor_rank = sev_rank(floor);
            let findings = !summary.recommendations.is_empty();

            // Text output (JSON added in Task 4).
            print!("{}", render_summary_text(&summary));
            for rec in &summary.recommendations {
                if sev_rank(&rec.severity) >= floor_rank {
                    let sf = scoped_filter(&cli.filter, rec);
                    println!("\n────────────────────────────────────────");
                    println!(
                        "[{}] {} — {}",
                        rec.severity.to_ascii_uppercase(),
                        rec.kind,
                        rec.title
                    );
                    println!("$ wiretrail {} {}", cli.file.display(), rec.command_line());
                    print!(
                        "{}",
                        drilldown_text(&rec.command, &cap, &sf, cli.top, cli.unsafe_include_secrets)
                    );
                }
            }
            let not_drilled: Vec<&Recommendation> = summary
                .recommendations
                .iter()
                .filter(|r| sev_rank(&r.severity) < floor_rank)
                .collect();
            if !not_drilled.is_empty() {
                println!("\nnot drilled (below threshold):");
                for r in &not_drilled {
                    println!(
                        "  [{}] {} — {}   (run: wiretrail {} {})",
                        r.severity.to_ascii_uppercase(),
                        r.kind,
                        r.title,
                        cli.file.display(),
                        r.command_line()
                    );
                }
            }
            exit(findings);
        }
```

- [ ] **Step 5: Build**

Run: `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo build 2>&1 | tail -6` (if `Finished` in ~0s without `Compiling`, run `cargo clean -p wiretrail && cargo build`).
Expected: SUCCESS.

- [ ] **Step 6: Manual smoke**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har auto | head -40
```
Expected: the summary block (with a "recommended next steps" section), then — if the fixture has any HIGH/MED findings — one or more `────` separators each followed by a `[SEV] kind — title` header, a `$ wiretrail …` line, and that command's inlined output. A clean fixture just prints the summary and exits.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat: auto command — summary + inline drill-down of recommendations (text)"
```

---

### Task 4: `auto --json` nesting + finalize flags

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Branch the arm on `cli.json`.** In `src/main.rs`, replace the body of the `Command::Auto { all, min_severity } => { ... }` arm with the version below. It keeps the Task 3 text path and adds the JSON path:

```rust
        Command::Auto { all, min_severity } => {
            let summary = compute_summary(&cap, &filter, cli.top);
            let floor = if all {
                "low"
            } else {
                min_severity.map(|s| s.as_str()).unwrap_or("medium")
            };
            let floor_rank = sev_rank(floor);
            let findings = !summary.recommendations.is_empty();

            if cli.json {
                let mut drilldowns = Vec::new();
                let mut not_drilled = Vec::new();
                for rec in &summary.recommendations {
                    if sev_rank(&rec.severity) >= floor_rank {
                        let sf = scoped_filter(&cli.filter, rec);
                        drilldowns.push(serde_json::json!({
                            "severity": rec.severity,
                            "kind": rec.kind,
                            "command": rec.command,
                            "filter": rec.filter,
                            "title": rec.title,
                            "detail": rec.detail,
                            "evidence_ids": rec.evidence_ids,
                            "result": drilldown_json(
                                &rec.command, &cap, &sf, cli.top, cli.unsafe_include_secrets
                            ),
                        }));
                    } else {
                        not_drilled.push(serde_json::json!({
                            "severity": rec.severity,
                            "kind": rec.kind,
                            "command": rec.command,
                            "filter": rec.filter,
                            "title": rec.title,
                            "detail": rec.detail,
                        }));
                    }
                }
                let result = serde_json::json!({
                    "summary": serde_json::to_value(&summary).unwrap_or(serde_json::Value::Null),
                    "drilldowns": drilldowns,
                    "not_drilled": not_drilled,
                });
                let next: Vec<String> = summary
                    .recommendations
                    .iter()
                    .map(|r| r.command.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let env = Envelope::new("auto", cap.meta.clone(), result).with_next_commands(next);
                println!("{}", env.to_json());
                exit(findings);
            }

            print!("{}", render_summary_text(&summary));
            for rec in &summary.recommendations {
                if sev_rank(&rec.severity) >= floor_rank {
                    let sf = scoped_filter(&cli.filter, rec);
                    println!("\n────────────────────────────────────────");
                    println!(
                        "[{}] {} — {}",
                        rec.severity.to_ascii_uppercase(),
                        rec.kind,
                        rec.title
                    );
                    println!("$ wiretrail {} {}", cli.file.display(), rec.command_line());
                    print!(
                        "{}",
                        drilldown_text(&rec.command, &cap, &sf, cli.top, cli.unsafe_include_secrets)
                    );
                }
            }
            let not_drilled: Vec<&Recommendation> = summary
                .recommendations
                .iter()
                .filter(|r| sev_rank(&r.severity) < floor_rank)
                .collect();
            if !not_drilled.is_empty() {
                println!("\nnot drilled (below threshold):");
                for r in &not_drilled {
                    println!(
                        "  [{}] {} — {}   (run: wiretrail {} {})",
                        r.severity.to_ascii_uppercase(),
                        r.kind,
                        r.title,
                        cli.file.display(),
                        r.command_line()
                    );
                }
            }
            exit(findings);
        }
```

- [ ] **Step 2: Make the `summary` footer dynamic.** Locate the `Command::Summary => { ... }` arm in `src/main.rs`. It currently calls `emit(... &["duplicates", "errors", "slowest"])`. Replace that arm's `emit` call so the footer derives from the recommendations, falling back to the static list when empty:

```rust
        Command::Summary => {
            let result = compute_summary(&cap, &filter, cli.top);
            let findings = result.error_count > 0 || !result.top_duplicates.is_empty();
            let mut next: Vec<String> = result
                .recommendations
                .iter()
                .map(|r| r.command.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if next.is_empty() {
                next = vec!["duplicates".into(), "errors".into(), "slowest".into()];
            }
            let next_refs: Vec<&str> = next.iter().map(|s| s.as_str()).collect();
            emit(
                cli.json,
                "summary",
                &cap.meta,
                &result,
                &render_summary_text(&result),
                &next_refs,
            );
            exit(findings);
        }
```

(If the existing `Summary` arm differs in its `findings` expression, keep that expression — only the `next` derivation and the final `emit` arg change.)

- [ ] **Step 3: Build + smoke both output modes**

Run:
```bash
CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo build 2>&1 | tail -3
cargo run --quiet -- tests/fixtures/someapi123.har auto --json | head -20
cargo run --quiet -- tests/fixtures/someapi123.har auto --min-severity high | tail -15
cargo run --quiet -- tests/fixtures/someapi123.har auto --all | tail -15
cargo run --quiet -- tests/fixtures/someapi123.har summary --json | grep -A4 next_commands
```
Expected: `auto --json` prints an envelope with `"command": "auto"` and a `result` containing `summary`/`drilldowns`/`not_drilled`; `--min-severity high` drills fewer than `--all`; the summary `--json` footer reflects recommended commands (or the static fallback on a clean fixture).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: auto --json nesting, severity flags; dynamic summary footer"
```

---

### Task 5: Integration tests + real-HAR confidence check

**Files:**
- Create: `tests/cli_auto.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_auto.rs`:

```rust
use std::process::Command;

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn auto_json_envelope_has_summary_and_drilldowns() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auto", "--json"]);
    assert!(stdout.contains("\"command\": \"auto\""));
    assert!(stdout.contains("\"summary\""));
    assert!(stdout.contains("\"drilldowns\""));
    assert!(stdout.contains("\"not_drilled\""));
}

#[test]
fn auto_text_includes_summary_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auto"]);
    assert!(stdout.contains("== wiretrail summary =="));
}

#[test]
fn min_severity_high_drills_at_most_as_many_as_all() {
    let count = |sev: &str| -> usize {
        let (stdout, _) = run(&[
            &fixture("someapi123.har"),
            "auto",
            if sev == "all" { "--all" } else { "--min-severity" },
            if sev == "all" { "" } else { sev },
        ]
        .iter()
        .filter(|a| !a.is_empty())
        .cloned()
        .collect::<Vec<_>>());
        stdout.matches("$ wiretrail ").count()
    };
    assert!(count("high") <= count("all"));
}

#[test]
fn summary_footer_is_present() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "summary", "--json"]);
    assert!(stdout.contains("\"next_commands\""));
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_auto 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result: FAILED" || echo "all green"`
Expected: `all green` (notably the untouched `cli_diagnose` + `analysis::diagnose` tests).

- [ ] **Step 4: fmt + clippy gate** (CI runs `-D warnings`):

Run:
```bash
cargo fmt --all
CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo clippy --all-targets -- -D warnings 2>&1 | tail -6
```
Expected: clippy `Finished` with no errors. If fmt changed files, re-run the full suite.

- [ ] **Step 5: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
A="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$A" auto 2>/dev/null | head -60
echo "=== min-severity high ==="
./target/release/wiretrail "$A" auto --min-severity high 2>/dev/null | grep -E '^\[|^\$' | head
echo "=== leak scan over full auto output ==="
./target/release/wiretrail "$A" auto --all 2>/dev/null | grep -iE "e2574d74-c23d-4846-a66e-d756b43f94ec|szpwe4fx4ngs8u9q" && echo "!!! LEAK !!!" || echo "no known secrets leaked"
```
Expected: `auto` prints the summary + recommended next steps, then inlines the ntsk 5xx/retry cluster and the Supabase auth story for HIGH/MED findings, lists LOW ones as not-drilled; `--min-severity high` shows fewer drill-downs; the leak scan reports no known secrets.

- [ ] **Step 6: Commit**

```bash
git add tests/cli_auto.rs
git commit -m "test: end-to-end tests for the auto command"
```

---

## Self-review

**Spec coverage:**
- Shared recommender core (`recommend()` + `Recommendation`) → Task 1. ✓
- `diagnose` refactored onto the core, output unchanged (parity guard) → Task 1 Steps 6-7. ✓
- `summary` gains `recommendations` field + rendered section; stats/`hints` kept → Task 2. ✓
- `summary` `next_commands` footer derived from recommendations (fallback to static) → Task 4 Step 2. ✓
- `auto` triggered-only, full inline; scoped by composed filter; reproducing command line printed → Tasks 3-4. ✓
- Severity gate: default medium (incl. critical), `--all` ≡ low, `--min-severity` narrows; LOW listed not drilled → Tasks 3-4. ✓
- `auto --json` nested envelope (`summary`/`drilldowns`/`not_drilled`), heterogeneous sub-results as `Value` → Task 4. ✓
- Exit code 1 when any recommendation exists, else 0 → Tasks 3-4 (`findings`). ✓
- Redaction preserved (each drill-down runs its own redactor; `--unsafe-include-secrets` threaded into `drilldown_text`/`drilldown_json`) → Tasks 3-4. ✓
- Tests: recommender unit, diagnose parity, summary unit, `cli_auto` integration, real-HAR leak scan → Tasks 1,2,5. ✓
- `report` untouched; only `summary`/`auto` use the recommender → no task touches report. ✓

**Placeholder scan:** No TBD/TODO. Every code step is complete; the `diagnose` body is replaced with a full adapter; the `auto` arm is shown in full in both Task 3 (text) and Task 4 (text+json) — Task 4 Step 1 explicitly says "replace the body" so there is no ambiguity about duplication. ✓

**Type consistency:**
- `Recommendation { severity, kind, title, detail, evidence_ids, command, filter }` + `command_line()` + `sev_rank` defined in Task 1, consumed identically in diagnose (Task 1), summary (Task 2), and main (Tasks 3-4). ✓
- `recommend(&Capture,&Filter,usize) -> Vec<Recommendation>` signature consistent across all call sites. ✓
- `drilldown_text`/`drilldown_json`/`scoped_filter` signatures defined in Task 3, called unchanged in Task 4. ✓
- Drill-down `match` covers exactly the commands the recommender emits (`errors,auth,rate-limit,retries,storms,diff,redirects,slowest`); `errors`/`diff` receive `unsafe_include`, `storms` receives `1000,5`, matching the verified `compute_*` signatures. ✓
- `SeverityArg::as_str()` (existing, M7) reused for `--min-severity`; `Envelope::new(...).with_next_commands(...)` (existing) reused for the `auto`/`summary` envelopes. ✓
- `SummaryResult.recommendations` field name identical in Task 2 (definition), Task 3/4 (usage). ✓
