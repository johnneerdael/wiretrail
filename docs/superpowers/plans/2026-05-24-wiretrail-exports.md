# wiretrail Exports (`report`, `curl`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the two export commands that complete wiretrail v1 — `report` (a dossier-style markdown document composed from the existing analyses) and `curl` (sanitized, safety-labeled replay commands).

**Architecture:** Both commands are thin composition layers over the Plan 1–3 analysis modules. `report` calls the existing `compute_*` functions and formats their results as markdown sections + tables. `curl` renders one or many `Entry` values as `curl` command strings, reusing the redaction engine and a small safety classifier. No new parsing or model work.

**Tech Stack:** Rust 2024, clap 4, serde/serde_json, url, plus the Plan 1–3 modules.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-har-analyzer-design.md` (the
`report` and `curl` command rows, "Markdown incident report export", and
"cURL/HTTPie export" + "Replay safety labels").

This is **Plan 4 of 4** — the last plan for v1. Plans 1–3 are complete and on `main`.
After this, all 14 v1 commands exist and the tool reproduces the nexio dossier
from the HAR side.

## Prerequisites (from Plans 1–3)

- `har::model::{Capture, Entry}` + cfg(test) `model::sample_entry` / `model::sample_capture`.
- `har::filter::Filter`, `har::config::Config`.
- `har::redact::{redact_header_value, redact_query_value, redact_body}`.
- `har::render::{Envelope, ExitCode, human_bytes, human_ms}`.
- Analysis computes: `summary::compute_summary`, `subsystems::compute_subsystems`,
  `duplicates::compute_duplicates`, `errors::compute_errors`,
  `redirects::compute_redirects`, `slowest::compute_slowest`.
- `analysis::show_entry::find_entry`.
- `main.rs` helpers `emit::<T: Serialize>(json, command, &CaptureMeta, &T, text, &[&str])` and `exit(findings) -> !`.

## File structure

```
src/analysis/mod.rs        # Modify: add pub mod curl, report
src/analysis/curl.rs       # NEW: entry -> sanitized curl command + safety label
src/analysis/report.rs     # NEW: compose markdown dossier from existing analyses
src/main.rs                # Modify: add Report + Curl subcommands
tests/cli_exports.rs       # NEW: end-to-end binary tests
```

---

### Task 1: Module scaffolding

**Files:**
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Register the two new modules.** Replace the entire contents of `src/analysis/mod.rs` with (alphabetical, adding `curl` and `report`):

```rust
pub mod curl;
pub mod duplicates;
pub mod endpoints;
pub mod errors;
pub mod hosts;
pub mod redirects;
pub mod report;
pub mod retries;
pub mod show_entry;
pub mod slowest;
pub mod subsystems;
pub mod summary;
pub mod timeline;
pub mod transitions;
```

- [ ] **Step 2: Create empty placeholder files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/analysis/curl.rs src/analysis/report.rs
cargo build 2>&1 | tail -5
```
Expected: build SUCCEEDS (empty modules are valid).

- [ ] **Step 3: Commit**

```bash
git add src/analysis/mod.rs src/analysis/curl.rs src/analysis/report.rs
git commit -m "chore: scaffold export modules (curl, report)"
```

---

### Task 2: `curl` command

**Files:**
- Create: `src/analysis/curl.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/curl.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{compute_curl, entry_to_curl};
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn get_is_safe_and_redacts_auth() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![
            (":method".into(), "GET".into()), // HTTP/2 pseudo-header, must be skipped
            ("Authorization".into(), "Bearer secret".into()),
            ("Accept".into(), "application/json".into()),
        ];
        let c = entry_to_curl(&e, false);
        assert!(c.safe);
        assert_eq!(c.label, "safe");
        assert!(c.command.starts_with("curl -X GET 'https://api.x/data'"));
        assert!(c.command.contains("Authorization: <redacted>"));
        assert!(c.command.contains("Accept: application/json"));
        assert!(!c.command.contains(":method"));
    }

    #[test]
    fn redacts_query_in_url() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.url = "https://api.x/data?access_token=leak&page=2".into();
        let c = entry_to_curl(&e, false);
        assert!(!c.command.contains("leak"));
        assert!(c.command.contains("page=2"));
    }

    #[test]
    fn mutating_method_is_unsafe() {
        let e = sample_entry(0, "api.x", "POST", "/things", 200);
        let c = entry_to_curl(&e, false);
        assert!(!c.safe);
        assert!(c.label.contains("mutating"));
    }

    #[test]
    fn payment_path_is_unsafe_even_for_get() {
        let e = sample_entry(0, "api.x", "GET", "/v1/payment/charge", 200);
        let c = entry_to_curl(&e, false);
        assert!(!c.safe);
        assert!(c.label.contains("payment"));
    }

    #[test]
    fn unsafe_flag_shows_raw_auth() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer secret".into())];
        let c = entry_to_curl(&e, true);
        assert!(c.command.contains("Bearer secret"));
    }

    #[test]
    fn compute_curl_bounds_by_top() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/b", 200),
            sample_entry(2, "h", "GET", "/c", 200),
        ]);
        let r = compute_curl(&cap, &Filter::parse(&[]).unwrap(), 2, false);
        assert_eq!(r.commands.len(), 2);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib curl 2>&1 | tail -12`
Expected: FAIL with "cannot find function `entry_to_curl`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/curl.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::{redact_body, redact_header_value, redact_query_value};
use serde::Serialize;

const MUTATING_METHODS: &[&str] = &["POST", "PUT", "PATCH", "DELETE"];
const RISKY_KEYWORDS: &[&str] = &["payment", "pay", "order", "checkout", "charge", "refund", "subscription"];
const BODY_MAX: usize = 4000;

#[derive(Debug, Serialize)]
pub struct CurlResult {
    pub commands: Vec<CurlCommand>,
}

#[derive(Debug, Serialize)]
pub struct CurlCommand {
    pub id: String,
    pub safe: bool,
    pub label: String,
    pub command: String,
}

/// Build a sanitized, safety-labeled curl command for one entry.
pub fn entry_to_curl(e: &Entry, unsafe_include: bool) -> CurlCommand {
    let method = e.method.to_ascii_uppercase();
    let url = build_url(e, unsafe_include);

    let mut parts = vec![format!("curl -X {method} '{url}'")];
    for (k, v) in &e.req_headers {
        if k.starts_with(':') {
            continue; // skip HTTP/2 pseudo-headers (:method, :path, :authority, :scheme)
        }
        let rv = redact_header_value(k, v, unsafe_include);
        parts.push(format!("  -H '{k}: {rv}'"));
    }
    if let Some(body) = e.req_body.as_deref().filter(|b| !b.is_empty()) {
        let rb = redact_body(body, unsafe_include, BODY_MAX);
        parts.push(format!("  --data '{rb}'"));
    }

    let (safe, label) = safety(&method, &e.norm_path);
    CurlCommand {
        id: e.id.clone(),
        safe,
        label,
        command: parts.join(" \\\n"),
    }
}

/// Render curl for every filtered entry, bounded by `top`.
pub fn compute_curl(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> CurlResult {
    let commands: Vec<CurlCommand> = cap
        .entries
        .iter()
        .filter(|e| filter.matches(e))
        .take(top)
        .map(|e| entry_to_curl(e, unsafe_include))
        .collect();
    CurlResult { commands }
}

fn build_url(e: &Entry, unsafe_include: bool) -> String {
    match url::Url::parse(&e.url) {
        Ok(mut u) => {
            let pairs: Vec<(String, String)> = u
                .query_pairs()
                .map(|(k, v)| {
                    let rv = redact_query_value(k.as_ref(), v.as_ref(), unsafe_include);
                    (k.into_owned(), rv)
                })
                .collect();
            u.set_query(None);
            let mut s = u.to_string();
            if !pairs.is_empty() {
                let q: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
                s.push('?');
                s.push_str(&q.join("&"));
            }
            s
        }
        Err(_) => e.url.clone(),
    }
}

fn safety(method: &str, norm_path: &str) -> (bool, String) {
    let lp = norm_path.to_ascii_lowercase();
    if RISKY_KEYWORDS.iter().any(|k| lp.contains(k)) {
        return (false, "payment/order endpoint".to_string());
    }
    if MUTATING_METHODS.contains(&method) {
        return (false, format!("mutating method {method}"));
    }
    (true, "safe".to_string())
}

/// Render curl commands as terminal text with safety annotations.
pub fn render_curl_text(r: &CurlResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail curl ==\n");
    for c in &r.commands {
        let tag = if c.safe { "SAFE" } else { "UNSAFE" };
        out.push_str(&format!("\n# {} [{}: {}]\n{}\n", c.id, tag, c.label, c.command));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib curl 2>&1 | tail -10`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/curl.rs
git commit -m "feat: curl command (sanitized replay + safety labels)"
```

---

### Task 3: `report` command

**Files:**
- Create: `src/analysis/report.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/report.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compose_report;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let d0 = sample_entry(0, "api.x", "POST", "/resolve", 200);
        let d1 = sample_entry(1, "api.x", "POST", "/resolve", 200);
        let mut err = sample_entry(2, "api.x", "GET", "/missing", 404);
        err.resp_body = Some(r#"{"message":"nope"}"#.to_string());
        sample_capture(vec![d0, d1, err])
    }

    #[test]
    fn report_has_expected_sections() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("# wiretrail report"));
        assert!(md.contains("## Executive Summary"));
        assert!(md.contains("## Subsystems"));
        assert!(md.contains("## Duplicate Index"));
        assert!(md.contains("## Errors"));
    }

    #[test]
    fn duplicate_index_lists_the_repeated_call() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("POST api.x/resolve"));
    }

    #[test]
    fn error_message_is_included() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("nope"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib report 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compose_report`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/report.rs`:

```rust
use crate::analysis::duplicates::compute_duplicates;
use crate::analysis::errors::compute_errors;
use crate::analysis::redirects::compute_redirects;
use crate::analysis::slowest::compute_slowest;
use crate::analysis::subsystems::compute_subsystems;
use crate::analysis::summary::compute_summary;
use crate::config::Config;
use crate::filter::Filter;
use crate::model::Capture;
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReportResult {
    pub markdown: String,
}

/// Compose a dossier-style markdown report from the existing analyses.
pub fn compose_report(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    top: usize,
    unsafe_include: bool,
) -> String {
    let mut md = String::new();

    md.push_str("# wiretrail report\n\n");
    md.push_str(&format!("- Creator: {} {}\n", cap.meta.creator, cap.meta.creator_version));
    md.push_str(&format!("- HAR version: {}\n", cap.meta.har_version));
    md.push_str(&format!("- Entries: {}\n", cap.meta.entry_count));
    md.push_str(&format!("- Window: {}\n\n", human_ms(cap.meta.duration_ms)));

    let summary = compute_summary(cap, filter, top);
    md.push_str("## Executive Summary\n\n");
    md.push_str(&format!(
        "{} requests after filter, {} error responses, {} duplicate groups in the top list.\n\n",
        summary.filtered_entries,
        summary.error_count,
        summary.top_duplicates.len()
    ));

    let subs = compute_subsystems(cap, filter, config, top);
    md.push_str("## Subsystems\n\n");
    md.push_str("| Subsystem | Requests | Window | Errors | Dups |\n");
    md.push_str("|---|---:|---|---:|---:|\n");
    for s in &subs.subsystems {
        md.push_str(&format!(
            "| {} | {} | {} - {} | {} | {} |\n",
            s.name,
            s.count,
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms),
            s.error_count,
            s.duplicate_count
        ));
    }
    md.push('\n');

    let dups = compute_duplicates(cap, filter, top);
    if !dups.groups.is_empty() {
        md.push_str("## Duplicate Index\n\n");
        for g in &dups.groups {
            let tag = if g.is_retry_pattern { " (retry pattern)" } else { "" };
            md.push_str(&format!(
                "- {}x `{} {}{}`{}\n",
                g.count, g.method, g.host, g.norm_path, tag
            ));
        }
        md.push('\n');
    }

    let errs = compute_errors(cap, filter, top, unsafe_include);
    if !errs.groups.is_empty() {
        md.push_str("## Errors\n\n");
        for g in &errs.groups {
            md.push_str(&format!(
                "- {}x [{}] `{} {}{}`",
                g.count, g.status, g.method, g.host, g.norm_path
            ));
            if let Some(m) = &g.error_message {
                md.push_str(&format!(" — {m}"));
            }
            md.push('\n');
        }
        md.push('\n');
    }

    let reds = compute_redirects(cap, filter, top);
    let storms: Vec<_> = reds.groups.iter().filter(|g| g.is_storm).collect();
    if !storms.is_empty() {
        md.push_str("## Redirect Storms\n\n");
        for g in storms {
            md.push_str(&format!(
                "- {}x [{}] `{} {}{}`\n",
                g.count, g.status, g.method, g.host, g.norm_path
            ));
        }
        md.push('\n');
    }

    let slow = compute_slowest(cap, filter, top);
    if !slow.entries.is_empty() {
        md.push_str("## Slowest Requests\n\n");
        for e in &slow.entries {
            md.push_str(&format!(
                "- {} `{} {}{}` [{}] — {}\n",
                human_ms(e.duration_ms),
                e.method,
                e.host,
                e.norm_path,
                e.status,
                e.bottleneck
            ));
        }
        md.push('\n');
    }

    md
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib report 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/report.rs
git commit -m "feat: report command (markdown dossier composition)"
```

---

### Task 4: Wire `report` and `curl` into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::transitions::...;` line, add:

```rust
use har::analysis::curl::{compute_curl, entry_to_curl, render_curl_text, CurlResult};
use har::analysis::report::{compose_report, ReportResult};
```

- [ ] **Step 2: Add the two subcommand variants.** Inside `enum Command { ... }`, after the `ShowEntry { id: String },` variant, add:

```rust
    /// Compose a dossier-style markdown report.
    Report,
    /// Sanitized curl replay commands (one entry, or all filtered).
    Curl {
        /// Optional entry id (e000123) or index; omit to emit all filtered entries.
        id: Option<String>,
    },
```

- [ ] **Step 3: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::ShowEntry { id } => { ... }` arm, add:

```rust
        Command::Report => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let markdown = compose_report(&cap, &filter, &config, cli.top, cli.unsafe_include_secrets);
            if cli.json {
                let result = ReportResult { markdown };
                let env = Envelope::new("report", cap.meta.clone(), &result);
                println!("{}", env.to_json());
            } else {
                print!("{markdown}");
            }
            exit(false);
        }
        Command::Curl { id } => {
            let result = match id {
                Some(id) => {
                    let Some(e) = find_entry(&cap, &id) else {
                        eprintln!("wiretrail: no entry with id or index '{id}'");
                        std::process::exit(ExitCode::InvalidHar as i32);
                    };
                    CurlResult {
                        commands: vec![entry_to_curl(e, cli.unsafe_include_secrets)],
                    }
                }
                None => compute_curl(&cap, &filter, cli.top, cli.unsafe_include_secrets),
            };
            emit(
                cli.json,
                "curl",
                &cap.meta,
                &result,
                &render_curl_text(&result),
                &["show-entry", "errors", "duplicates"],
            );
            exit(false);
        }
```

- [ ] **Step 4: Build**

Run: `cargo build 2>&1 | tail -8`
Expected: SUCCESS.

- [ ] **Step 5: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har report
cargo run --quiet -- tests/fixtures/someapi123.har curl e000000
cargo run --quiet -- tests/fixtures/someapi123.har curl --json | head -8
```
Expected: `report` prints a markdown doc starting with `# wiretrail report` and a `## Subsystems` table; `curl e000000` prints a `curl -X GET 'https://api.someapi123.io...'` block tagged `[SAFE: safe]` with `Authorization`-style headers redacted; `curl --json` prints an envelope with `"command": "curl"` and a `commands` array.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire report and curl export commands into CLI"
```

---

### Task 5: End-to-end binary tests

**Files:**
- Create: `tests/cli_exports.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_exports.rs`:

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
fn report_is_markdown() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "report"]);
    assert!(stdout.contains("# wiretrail report"));
    assert!(stdout.contains("## Subsystems"));
}

#[test]
fn curl_single_entry_redacts() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "curl", "e000000"]);
    assert!(stdout.contains("curl -X GET"));
    // no raw Authorization-style bearer secrets should leak by default
    assert!(!stdout.to_lowercase().contains("bearer "));
}

#[test]
fn curl_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "curl", "--json"]);
    assert!(stdout.contains("\"command\": \"curl\""));
    assert!(stdout.contains("\"commands\""));
}

#[test]
fn curl_unknown_id_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "curl", "e999999"]);
    assert_eq!(code, 2);
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_exports 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_exports.rs
git commit -m "test: end-to-end tests for report and curl"
```

---

## Self-review

**Spec coverage (exports slice):**
- `report` — markdown dossier with summary + subsystem category table + duplicate index + errors + redirect storms + slowest, composed from existing analyses → Tasks 3, 4. ✓
- `curl` — sanitized replay for one entry (by id/index) or all filtered entries; redacts headers/query/body by default; `--unsafe-include-secrets` override; safe/unsafe labels by method + payment/order keywords → Tasks 2, 4. ✓
- `--json` envelopes for both (report wraps the markdown string; curl emits the commands array) → Task 4. ✓
- This completes all 14 v1 commands. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code; every command step states expected output. ✓

**Type consistency:**
- `entry_to_curl(&Entry, bool) -> CurlCommand`, `compute_curl(&Capture, &Filter, usize, bool) -> CurlResult`, `render_curl_text(&CurlResult)` (Task 2) used in Task 4 with matching args. ✓
- `CurlResult { commands: Vec<CurlCommand> }` constructed directly in Task 4's single-entry branch — field name `commands` matches Task 2. ✓
- `compose_report(&Capture, &Filter, &Config, usize, bool) -> String` and `ReportResult { markdown: String }` (Task 3) used in Task 4. ✓
- `find_entry(&Capture, &str) -> Option<&Entry>` (Plan 3, Task 12) reused in Task 4's `Curl` arm — already imported in `main.rs` from Plan 3. ✓
- `compute_*` calls inside `compose_report` (Task 3) match the Plan 1–3 signatures: `compute_summary(cap, filter, top)`, `compute_subsystems(cap, filter, config, top)`, `compute_duplicates(cap, filter, top)`, `compute_errors(cap, filter, top, unsafe_include)`, `compute_redirects(cap, filter, top)`, `compute_slowest(cap, filter, top)`. ✓
- Result fields read in `compose_report` (`summary.filtered_entries`, `summary.error_count`, `summary.top_duplicates`, `subs.subsystems[].{name,count,first_offset_ms,last_offset_ms,error_count,duplicate_count}`, `dups.groups[].{count,method,host,norm_path,is_retry_pattern}`, `errs.groups[].{count,status,method,host,norm_path,error_message}`, `reds.groups[].{is_storm,count,status,method,host,norm_path}`, `slow.entries[].{duration_ms,method,host,norm_path,status,bottleneck}`) all match the structs defined in Plans 1–3. ✓
- `redact_header_value`/`redact_query_value`/`redact_body` (Plans 1, 3) used by curl (Task 2). ✓
- `emit`/`exit` (Plan 2) reused unchanged; `CurlResult` derives `Serialize`; `report` bypasses `emit` to avoid the "next useful commands" footer in the shareable markdown. ✓
