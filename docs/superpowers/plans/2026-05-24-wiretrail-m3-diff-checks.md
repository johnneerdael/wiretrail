# wiretrail M3 — Diff & Checks (`diff`, `checks`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `diff` (what varies across repeated calls to the same endpoint) and `checks` (config-driven missing-header + built-in content-type-mismatch checks), completing the M1+M2+M3 analysis expansion.

**Architecture:** Two analysis modules in the established `compute_* → result + render_*_text` pattern, wired through `emit`/`exit`. `checks` reads an additive `required_headers` field on the existing `Config` (loaded like `subsystems`/`report`). `diff` reuses route grouping + the redaction engine.

**Tech Stack:** Rust 2024, serde/serde_json, ahash, yaml_serde (config) — no new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-analysis-expansion-design.md`,
Phase M3. **Plan 3 of 3** (M1, M2 shipped).

## Prerequisites (verified present)

- `model::{Capture, Entry}`, `Entry.method/host/norm_path/query/req_headers/req_body/resp_body/content_type/status/id`, cfg(test) `sample_entry`/`sample_capture`.
- `filter::Filter`, `redact::redact_query_value`, `glob::glob_match`, `config::Config` (derives `Default`+`Deserialize`, `ownership` field with `#[serde(default)]`).
- `main.rs` `emit`/`exit`, global `--config`/`--unsafe-include-secrets`; `Config::load(Option<&Path>)`.

## File structure

```
src/config.rs            # Modify: add required_headers + RequiredHeaderRule
src/analysis/mod.rs      # Modify: declare checks, diff
src/analysis/diff.rs     # NEW
src/analysis/checks.rs   # NEW
src/main.rs              # Modify: 2 subcommands + dispatch
tests/cli_diff.rs        # NEW: integration tests
```

---

### Task 1: Scaffold modules

**Files:**
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Declare the two new modules.** Replace the entire contents of `src/analysis/mod.rs` with (alphabetical, adding `checks`, `diff`):

```rust
pub mod auth;
pub mod checks;
pub mod curl;
pub mod diff;
pub mod duplicates;
pub mod endpoints;
pub mod errors;
pub mod handoff;
pub mod hosts;
pub mod jwt;
pub mod pagination;
pub mod rate_limit;
pub mod redirects;
pub mod report;
pub mod retries;
pub mod show_entry;
pub mod slowest;
pub mod storms;
pub mod subsystems;
pub mod summary;
pub mod timeline;
pub mod transitions;
```

- [ ] **Step 2: Create empty files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/analysis/checks.rs src/analysis/diff.rs
cargo build 2>&1 | tail -4
```
Expected: build SUCCEEDS.

- [ ] **Step 3: Commit**

```bash
git add src/analysis/mod.rs src/analysis/checks.rs src/analysis/diff.rs
git commit -m "chore: scaffold M3 diff/checks modules"
```

---

### Task 2: `Config.required_headers`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add a failing test.** In `src/config.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add before its closing brace:

```rust
    #[test]
    fn parses_required_headers() {
        let yaml = r#"
required_headers:
  - host: "api.company.com"
    headers: ["Authorization", "X-App-Version"]
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.required_headers.len(), 1);
        assert_eq!(cfg.required_headers[0].host, "api.company.com");
        assert_eq!(cfg.required_headers[0].headers, vec!["Authorization", "X-App-Version"]);
    }

    #[test]
    fn required_headers_defaults_empty() {
        let cfg = Config::from_yaml_str("ownership: []").unwrap();
        assert!(cfg.required_headers.is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib config 2>&1 | tail -12`
Expected: FAIL — `Config` has no field `required_headers`.

- [ ] **Step 3: Implement.** In `src/config.rs`, add the new field to `Config` (after the `ownership` field):

```rust
    #[serde(default)]
    pub required_headers: Vec<RequiredHeaderRule>,
```

Then add the rule struct directly below the `Config` struct definition (before `OwnershipRule`):

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RequiredHeaderRule {
    /// Host glob the rule applies to.
    pub host: String,
    /// Header names that must be present on matching requests.
    #[serde(default)]
    pub headers: Vec<String>,
}
```

- [ ] **Step 4: Run to verify pass** (new + existing config tests).

Run: `cargo test --lib config 2>&1 | tail -10`
Expected: PASS (6 tests: 4 original + 2 new).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: required_headers config rule for checks"
```

---

### Task 3: `diff` command

**Files:**
- Create: `src/analysis/diff.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/diff.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_diff;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn post(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", "/items", 200);
        e.req_body = Some(body.to_string());
        e
    }

    #[test]
    fn body_volatile_only() {
        let cap = sample_capture(vec![
            post(0, r#"{"name":"a","ts":1}"#),
            post(1, r#"{"name":"a","ts":2}"#),
        ]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/items").unwrap();
        assert_eq!(g.body_verdict, "volatile-only");
    }

    #[test]
    fn body_meaningful() {
        let cap = sample_capture(vec![
            post(0, r#"{"name":"a"}"#),
            post(1, r#"{"name":"b"}"#),
        ]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/items").unwrap();
        assert_eq!(g.body_verdict, "meaningful");
    }

    #[test]
    fn varying_query_is_reported_and_redacted() {
        let mut a = sample_entry(0, "api.x", "GET", "/y", 200);
        a.query = vec![("page".into(), "1".into()), ("token".into(), "AAA".into())];
        let mut b = sample_entry(1, "api.x", "GET", "/y", 200);
        b.query = vec![("page".into(), "2".into()), ("token".into(), "BBB".into())];
        let r = compute_diff(&sample_capture(vec![a, b]), &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/y").unwrap();
        let keys: Vec<&str> = g.varying_query.iter().map(|q| q.key.as_str()).collect();
        assert!(keys.contains(&"page"));
        // sensitive query value is redacted in samples
        let tok = g.varying_query.iter().find(|q| q.key == "token").unwrap();
        assert!(tok.samples.iter().all(|s| s == "<redacted>"));
    }

    #[test]
    fn identical_group_is_skipped() {
        let cap = sample_capture(vec![post(0, r#"{"name":"a"}"#), post(1, r#"{"name":"a"}"#)]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        assert!(r.groups.is_empty()); // no variance -> not reported (duplicates covers it)
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::diff 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_diff`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/diff.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::redact_query_value;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

const VOLATILE_KEYS: &[&str] = &[
    "timestamp", "ts", "nonce", "date", "cb", "cachebuster", "requestid", "request_id", "_",
];
const SAMPLE_CAP: usize = 3;

#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub groups: Vec<DiffGroup>,
}

#[derive(Debug, Serialize)]
pub struct DiffGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub count: usize,
    pub varying_query: Vec<QueryVariance>,
    pub varying_header_names: Vec<String>,
    pub body_verdict: String,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryVariance {
    pub key: String,
    pub samples: Vec<String>,
}

/// Show what varies across repeated calls to the same (method, host, norm_path).
pub fn compute_diff(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> DiffResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_route: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut groups: Vec<DiffGroup> = Vec::new();
    for ((method, host, norm_path), mut g) in by_route {
        if g.len() < 2 {
            continue;
        }
        g.sort_by(|a, b| a.index.cmp(&b.index));

        let varying_query = varying_query(&g, unsafe_include);
        let varying_header_names = varying_header_names(&g);
        let body_verdict = body_verdict(&g);

        let has_variance = !varying_query.is_empty()
            || !varying_header_names.is_empty()
            || body_verdict == "volatile-only"
            || body_verdict == "meaningful";
        if !has_variance {
            continue;
        }

        groups.push(DiffGroup {
            host,
            method,
            norm_path,
            count: g.len(),
            varying_query,
            varying_header_names,
            body_verdict,
            entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        });
    }

    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    DiffResult { groups }
}

fn varying_query(members: &[&Entry], unsafe_include: bool) -> Vec<QueryVariance> {
    let all_keys: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.query.iter().map(|(k, _)| k.clone()))
        .collect();
    let mut out = Vec::new();
    for k in all_keys {
        let mut values: Vec<String> = Vec::new();
        let mut distinct: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .query
                .iter()
                .find(|(qk, _)| *qk == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            if distinct.insert(v.clone()) {
                values.push(redact_query_value(&k, &v, unsafe_include));
            }
        }
        if distinct.len() > 1 {
            values.truncate(SAMPLE_CAP);
            out.push(QueryVariance { key: k, samples: values });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

fn varying_header_names(members: &[&Entry]) -> Vec<String> {
    let all_names: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.req_headers.iter().map(|(n, _)| n.to_ascii_lowercase()))
        .collect();
    let mut out = Vec::new();
    for name in all_names {
        let mut distinct: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .req_headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&name))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            distinct.insert(v);
        }
        if distinct.len() > 1 {
            out.push(name);
        }
    }
    out.sort();
    out
}

fn is_volatile(key: &str) -> bool {
    let lk = key.to_ascii_lowercase();
    VOLATILE_KEYS.iter().any(|v| lk == *v || lk.contains(v))
}

fn body_verdict(members: &[&Entry]) -> String {
    let bodies: Vec<&String> = members
        .iter()
        .filter_map(|e| e.req_body.as_ref().filter(|b| !b.is_empty()))
        .collect();
    if bodies.len() < 2 {
        return "none".to_string();
    }
    if bodies.iter().all(|b| *b == bodies[0]) {
        return "identical".to_string();
    }
    // Try parsing every body as a JSON object.
    let objs: Option<Vec<serde_json::Map<String, serde_json::Value>>> = bodies
        .iter()
        .map(|b| {
            serde_json::from_str::<serde_json::Value>(b)
                .ok()
                .and_then(|v| v.as_object().cloned())
        })
        .collect();
    if let Some(objs) = objs {
        let mut keys: AHashSet<String> = AHashSet::new();
        for o in &objs {
            for k in o.keys() {
                keys.insert(k.clone());
            }
        }
        let mut differing: Vec<String> = Vec::new();
        for k in &keys {
            let mut distinct: AHashSet<String> = AHashSet::new();
            for o in &objs {
                distinct.insert(o.get(k).map(|v| v.to_string()).unwrap_or_default());
            }
            if distinct.len() > 1 {
                differing.push(k.clone());
            }
        }
        if differing.iter().all(|k| is_volatile(k)) {
            return "volatile-only".to_string();
        }
        return "meaningful".to_string();
    }
    "meaningful".to_string()
}

/// Render diff groups as deterministic terminal text.
pub fn render_diff_text(r: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail diff ==\n");
    for g in &r.groups {
        out.push_str(&format!(
            "\n{} {}{}  ({} calls, body: {})\n",
            g.method, g.host, g.norm_path, g.count, g.body_verdict
        ));
        for q in &g.varying_query {
            out.push_str(&format!("  query {} varies: {}\n", q.key, q.samples.join(", ")));
        }
        if !g.varying_header_names.is_empty() {
            out.push_str(&format!("  headers vary: {}\n", g.varying_header_names.join(", ")));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::diff 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/diff.rs
git commit -m "feat: diff command (query/header/body variance across repeats)"
```

---

### Task 4: `checks` command

**Files:**
- Create: `src/analysis/checks.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/checks.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_checks;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn cfg_required(host: &str, headers: &[&str]) -> Config {
        let yaml = format!(
            "required_headers:\n  - host: \"{host}\"\n    headers: [{}]\n",
            headers.iter().map(|h| format!("\"{h}\"")).collect::<Vec<_>>().join(", ")
        );
        Config::from_yaml_str(&yaml).unwrap()
    }

    #[test]
    fn flags_missing_required_header() {
        let e = sample_entry(0, "api.x", "GET", "/data", 200); // no Authorization
        let cap = sample_capture(vec![e]);
        let cfg = cfg_required("api.x", &["Authorization"]);
        let r = compute_checks(&cap, &Filter::parse(&[]).unwrap(), &cfg, 50);
        assert!(r.findings.iter().any(|f| f.rule == "missing-header"
            && f.detail.contains("Authorization")
            && f.entry_ids.contains(&"e000000".to_string())));
    }

    #[test]
    fn present_header_not_flagged() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        let cfg = cfg_required("api.x", &["Authorization"]);
        let r = compute_checks(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), &cfg, 50);
        assert!(r.findings.iter().all(|f| f.rule != "missing-header"));
    }

    #[test]
    fn flags_json_body_without_json_content_type() {
        let mut e = sample_entry(0, "api.x", "POST", "/data", 200);
        e.req_headers = vec![("Content-Type".into(), "text/plain".into())];
        e.req_body = Some(r#"{"a":1}"#.to_string());
        let r = compute_checks(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), &Config::default(), 50);
        assert!(r.findings.iter().any(|f| f.rule == "content-type"
            && f.detail.contains("JSON body")));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::checks 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_checks`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/checks.rs`:

```rust
use crate::config::Config;
use crate::filter::Filter;
use crate::glob::glob_match;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChecksResult {
    pub findings: Vec<CheckFinding>,
}

#[derive(Debug, Serialize)]
pub struct CheckFinding {
    pub rule: String,
    pub host: String,
    pub norm_path: String,
    pub detail: String,
    pub entry_ids: Vec<String>,
}

fn req_content_type(e: &Entry) -> Option<String> {
    e.req_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_ascii_lowercase())
}

fn looks_like_json(body: &str) -> bool {
    let t = body.trim_start();
    t.starts_with('{') || t.starts_with('[')
}

fn content_type_issues(e: &Entry) -> Vec<String> {
    let mut v = Vec::new();
    let req_ct = req_content_type(e).unwrap_or_default();
    if let Some(b) = e.req_body.as_deref().filter(|b| !b.is_empty()) {
        if looks_like_json(b) && !req_ct.contains("json") {
            v.push("request JSON body without application/json content-type".to_string());
        }
    }
    let resp_ct = e.content_type.clone().unwrap_or_default().to_ascii_lowercase();
    match e.resp_body.as_deref().filter(|b| !b.is_empty()) {
        Some(b) => {
            if looks_like_json(b) && resp_ct.contains("html") {
                v.push("JSON response served as text/html".to_string());
            }
        }
        None => {
            if resp_ct.contains("json") && e.status == 200 {
                v.push("empty body with JSON content-type".to_string());
            }
        }
    }
    v
}

/// Run built-in checks: missing required headers (config) + content-type mismatch.
pub fn compute_checks(cap: &Capture, filter: &Filter, config: &Config, top: usize) -> ChecksResult {
    // key = (rule, host, norm_path, detail) -> entry ids
    let mut map: AHashMap<(String, String, String, String), Vec<String>> = AHashMap::new();

    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        // missing required headers
        for rule in &config.required_headers {
            if glob_match(&rule.host, &e.host) {
                for h in &rule.headers {
                    let present = e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(h));
                    if !present {
                        let key = (
                            "missing-header".to_string(),
                            e.host.clone(),
                            e.norm_path.clone(),
                            format!("missing required header: {h}"),
                        );
                        map.entry(key).or_default().push(e.id.clone());
                    }
                }
            }
        }
        // content-type mismatches
        for detail in content_type_issues(e) {
            let key = ("content-type".to_string(), e.host.clone(), e.norm_path.clone(), detail);
            map.entry(key).or_default().push(e.id.clone());
        }
    }

    let mut findings: Vec<CheckFinding> = map
        .into_iter()
        .map(|((rule, host, norm_path, detail), entry_ids)| CheckFinding {
            rule,
            host,
            norm_path,
            detail,
            entry_ids,
        })
        .collect();
    findings.sort_by(|a, b| {
        b.entry_ids
            .len()
            .cmp(&a.entry_ids.len())
            .then(a.rule.cmp(&b.rule))
            .then(a.host.cmp(&b.host))
            .then(a.detail.cmp(&b.detail))
    });
    findings.truncate(top);
    ChecksResult { findings }
}

/// Render checks findings as deterministic terminal text.
pub fn render_checks_text(r: &ChecksResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail checks ==\n");
    for f in &r.findings {
        out.push_str(&format!(
            "\n[{}] {} {}\n  {} ({} entries)\n",
            f.rule,
            f.host,
            f.norm_path,
            f.detail,
            f.entry_ids.len()
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::checks 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/checks.rs
git commit -m "feat: checks command (required-header + content-type mismatch)"
```

---

### Task 5: Wire the two commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::handoff::...;` line, add:

```rust
use har::analysis::diff::{compute_diff, render_diff_text};
use har::analysis::checks::{compute_checks, render_checks_text};
```

- [ ] **Step 2: Add the subcommand variants.** Inside `enum Command { ... }`, after the `Handoff,` variant, add:

```rust
    /// What varies across repeated calls to the same endpoint.
    Diff,
    /// Built-in checks: required headers (config) + content-type mismatch.
    Checks,
```

- [ ] **Step 3: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::Handoff => { ... }` arm, add:

```rust
        Command::Diff => {
            let result = compute_diff(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = result.groups.iter().any(|g| {
                g.body_verdict == "meaningful"
                    || g.varying_header_names.iter().any(|n| n == "authorization")
            });
            emit(
                cli.json,
                "diff",
                &cap.meta,
                &result,
                &render_diff_text(&result),
                &["duplicates", "show-entry", "endpoints"],
            );
            exit(findings);
        }
        Command::Checks => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_checks(&cap, &filter, &config, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "checks",
                &cap.meta,
                &result,
                &render_checks_text(&result),
                &["errors", "show-entry", "endpoints"],
            );
            exit(findings);
        }
```

- [ ] **Step 4: Build**

Run: `cargo build 2>&1 | tail -8`
Expected: SUCCESS.

- [ ] **Step 5: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har diff
cargo run --quiet -- tests/fixtures/someapi123.har checks --json | head -6
```
Expected: `diff` prints its header (single-entry fixture → no groups); `checks --json`
prints an envelope with `"command": "checks"` and a `findings` array.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire diff/checks commands into CLI"
```

---

### Task 6: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_diff.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_diff.rs`:

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
fn diff_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "diff", "--json"]);
    assert!(stdout.contains("\"command\": \"diff\""));
    assert!(stdout.contains("\"groups\""));
}

#[test]
fn checks_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "checks", "--json"]);
    assert!(stdout.contains("\"command\": \"checks\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn checks_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "checks"]);
    assert!(stdout.contains("== wiretrail checks =="));
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_diff 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result: FAILED" || echo "all green"`
Expected: `all green`.

- [ ] **Step 4: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
HAR="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$HAR" diff 2>/dev/null | head -20
```
Expected: `diff` shows the repeated Supabase `sync_resolve_account_secret` POSTs as
`body: identical` (skipped — pure duplicates) or `volatile-only`, and surfaces any
endpoints where query/body genuinely varies across repeats. No secret values appear
(query samples redacted, header values shown as names only).

- [ ] **Step 5: Commit**

```bash
git add tests/cli_diff.rs
git commit -m "test: end-to-end tests for diff/checks"
```

---

## Self-review

**Spec coverage (Phase M3):**
- `diff` body (#31) — `identical`/`volatile-only`/`meaningful`/`none` verdict over request bodies → Task 3. ✓
- `diff` query (#32) — varying query keys with redacted sample values → Task 3. ✓
- `diff` header (#33) — varying request-header names (values not printed) → Task 3. ✓
- `checks` missing-header (#34) — config `required_headers` (host glob → header list) → Tasks 2, 4. ✓
- `checks` content-type mismatch (#35) — JSON body w/o json CT, JSON resp as html, empty body w/ json CT → Task 4. ✓
- All: `--json`, filter, `--top`, next_commands, findings exit codes; `diff` honors `--unsafe-include-secrets` (query samples); `checks` loads `--config` → Task 5. ✓
- No new dependencies. ✓

**Placeholder scan:** No TBD/TODO; every code step complete; every command step states expected output. ✓

**Type consistency:**
- `Config.required_headers: Vec<RequiredHeaderRule>` + `RequiredHeaderRule { host: String, headers: Vec<String> }` (Task 2) consumed in `checks` (Task 4). ✓
- `compute_diff(&Capture,&Filter,usize,bool) -> DiffResult`, `compute_checks(&Capture,&Filter,&Config,usize) -> ChecksResult`, and `render_diff_text`/`render_checks_text` — Task 5 dispatch passes matching args (diff takes `unsafe_include_secrets`; checks takes `&config`). ✓
- `DiffGroup` fields (`body_verdict`, `varying_header_names`) referenced in Task 5 findings. ✓
- `redact_query_value(&str,&str,bool)` (Plan 1) used in `diff` (Task 3); `glob_match(&str,&str)` (Plan 2) used in `checks` (Task 4). ✓
- Result structs derive `Serialize`; `emit`/`exit`/`Config::load`/`ExitCode` reused unchanged. ✓
- `Entry` fields used (`method`,`host`,`norm_path`,`query`,`req_headers`,`req_body`,`resp_body`,`content_type`,`status`,`id`,`index`) all exist. ✓
