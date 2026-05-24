# wiretrail M6 — Extraction & Data-out (`search`, `extract`, `export`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three data-out commands: `search` (redaction-safe body grep), `extract` (JSON-path field extraction), and `export` (flatten entries to NDJSON/CSV).

**Architecture:** Three focused analysis modules + a hand-rolled `jsonpath` support module, in the established `compute_* → result + render_*_text` pattern. `search` adds the `regex` crate; `extract` uses the hand-rolled evaluator (no dep). `export` is a serializer (NDJSON/CSV) over normalized metadata.

**Tech Stack:** Rust 2024, serde/serde_json, clap, **+ `regex`** (one new dependency).

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-m5-m7-expansion-design.md`,
Phase M6. **Plan 2 of 3** for expansion-2 (M5 shipped; M7 regression/rules follows).

## Prerequisites (verified present)

- `model::{Capture, Entry}`, `Entry.{id,index,method,host,norm_path,status,resp_body,req_body,started_offset_ms,duration_ms,content_type,resource_type,correlation,sizes}`, cfg(test) `sample_entry`/`sample_capture`.
- `filter::Filter`, `redact::{redact_value, REDACTED}` (verified: `redact_value(&str,bool)->String`, `REDACTED` const), `opaque::is_opaque`.
- `render` (not needed here), `emit`/`exit` in `main.rs`, global `--unsafe-include-secrets`.
- `regex` latest is `1.12.x`.

## File structure

```
Cargo.toml               # Modify: add regex = "1"
src/lib.rs               # Modify: pub mod jsonpath;
src/jsonpath.rs          # NEW: minimal JSON-path evaluator (support)
src/analysis/mod.rs      # Modify: declare export, extract, search
src/analysis/search.rs   # NEW
src/analysis/extract.rs  # NEW
src/analysis/export.rs   # NEW
src/main.rs              # Modify: 3 subcommands + Target/ExportFormat enums
tests/cli_extract.rs     # NEW: integration tests
```

---

### Task 1: Scaffold + add `regex`

**Files:**
- Modify: `Cargo.toml`, `src/lib.rs`, `src/analysis/mod.rs`

- [ ] **Step 1: Add the `regex` dependency.** In `Cargo.toml`, under `[dependencies]`, add:

```toml
regex = "1"
```

- [ ] **Step 2: Declare the support module.** In `src/lib.rs`, after `pub mod jwt;`, add:

```rust
pub mod jsonpath;
```

- [ ] **Step 3: Declare the analysis modules.** Add `pub mod export;`, `pub mod extract;`, `pub mod search;` to `src/analysis/mod.rs` in alphabetical position (it is a flat sorted list of `pub mod` lines).

- [ ] **Step 4: Create empty files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/jsonpath.rs src/analysis/export.rs src/analysis/extract.rs src/analysis/search.rs
cargo build 2>&1 | tail -4
```
Expected: build SUCCEEDS (downloads `regex`).

> **Note:** if `cargo build` reports `Finished` in ~0s without `Compiling wiretrail`
> after a source edit (a stale-fingerprint quirk from the global cache), force it
> with `cargo clean -p wiretrail && cargo build`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/analysis/mod.rs src/jsonpath.rs src/analysis/export.rs src/analysis/extract.rs src/analysis/search.rs
git commit -m "chore: scaffold M6 extraction modules; add regex dep"
```

---

### Task 2: `jsonpath` evaluator

**Files:**
- Create: `src/jsonpath.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/jsonpath.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::eval;
    use serde_json::json;

    #[test]
    fn nested_key() {
        let v = json!({"a": {"b": 1}});
        assert_eq!(eval(&v, "$.a.b"), vec![json!(1)]);
    }

    #[test]
    fn array_index() {
        let v = json!({"errors": [{"code": "E1"}, {"code": "E2"}]});
        assert_eq!(eval(&v, "errors[0].code"), vec![json!("E1")]);
    }

    #[test]
    fn array_wildcard() {
        let v = json!({"errors": [{"code": "E1"}, {"code": "E2"}]});
        assert_eq!(eval(&v, "$.errors[*].code"), vec![json!("E1"), json!("E2")]);
    }

    #[test]
    fn missing_path_is_empty() {
        let v = json!({"a": 1});
        assert!(eval(&v, "$.nope.x").is_empty());
    }

    #[test]
    fn invalid_index_is_empty() {
        let v = json!({"a": [1]});
        assert!(eval(&v, "a[x]").is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib jsonpath 2>&1 | tail -12`
Expected: FAIL with "cannot find function `eval`".

- [ ] **Step 3: Implement** above the test module in `src/jsonpath.rs`:

```rust
use serde_json::Value;

enum Step {
    Key(String),
    Index(usize),
    Wildcard,
}

fn parse(path: &str) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let trimmed = path.trim().trim_start_matches('$');
    for raw in trimmed.split('.') {
        if raw.is_empty() {
            continue;
        }
        let mut rest = raw;
        // A segment may be "name", "name[0]", "name[*]", or "[0]".
        while let Some(lb) = rest.find('[') {
            let key = &rest[..lb];
            if !key.is_empty() {
                steps.push(Step::Key(key.to_string()));
            }
            let rb_rel = rest[lb..].find(']')?;
            let rb = lb + rb_rel;
            let inner = &rest[lb + 1..rb];
            if inner == "*" {
                steps.push(Step::Wildcard);
            } else {
                steps.push(Step::Index(inner.parse().ok()?));
            }
            rest = &rest[rb + 1..];
        }
        if !rest.is_empty() {
            steps.push(Step::Key(rest.to_string()));
        }
    }
    Some(steps)
}

/// Evaluate a minimal JSON path (`$.a.b`, `a[0].c`, `errors[*].code`) over a
/// JSON value, returning all matched values. Returns empty on a bad path.
pub fn eval(value: &Value, path: &str) -> Vec<Value> {
    let Some(steps) = parse(path) else {
        return Vec::new();
    };
    let mut current: Vec<&Value> = vec![value];
    for step in &steps {
        let mut next: Vec<&Value> = Vec::new();
        for v in &current {
            match step {
                Step::Key(k) => {
                    if let Some(child) = v.get(k) {
                        next.push(child);
                    }
                }
                Step::Index(i) => {
                    if let Some(child) = v.get(i) {
                        next.push(child);
                    }
                }
                Step::Wildcard => match v {
                    Value::Array(arr) => next.extend(arr.iter()),
                    Value::Object(map) => next.extend(map.values()),
                    _ => {}
                },
            }
        }
        current = next;
    }
    current.into_iter().cloned().collect()
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib jsonpath 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/jsonpath.rs
git commit -m "feat: minimal JSON-path evaluator"
```

---

### Task 3: `extract` command

**Files:**
- Create: `src/analysis/extract.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/extract.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{compute_extract, Target};
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn with_resp(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/a", 200);
        e.resp_body = Some(body.to_string());
        e
    }

    #[test]
    fn extracts_field_from_response_bodies() {
        let cap = sample_capture(vec![
            with_resp(0, r#"{"error":{"message":"boom"}}"#),
            with_resp(1, r#"{"error":{"message":"nope"}}"#),
        ]);
        let r = compute_extract(&cap, &Filter::parse(&[]).unwrap(), "$.error.message", Target::Resp, 10, false);
        let vals: Vec<&str> = r.values.iter().map(|v| v.value.as_str()).collect();
        assert!(vals.contains(&"boom"));
        assert!(vals.contains(&"nope"));
    }

    #[test]
    fn masks_opaque_value_by_default() {
        let cap = sample_capture(vec![with_resp(0, r#"{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"}"#)]);
        let r = compute_extract(&cap, &Filter::parse(&[]).unwrap(), "$.token", Target::Resp, 10, false);
        assert_eq!(r.values[0].value, "<redacted>");
        let r2 = compute_extract(&cap, &Filter::parse(&[]).unwrap(), "$.token", Target::Resp, 10, true);
        assert!(r2.values[0].value.starts_with("eyJ"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::extract 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_extract`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/extract.rs`:

```rust
use crate::filter::Filter;
use crate::jsonpath;
use crate::model::{Capture, Entry};
use crate::opaque::is_opaque;
use crate::redact::REDACTED;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Req,
    Resp,
}

#[derive(Debug, Serialize)]
pub struct ExtractResult {
    pub values: Vec<ExtractValue>,
}

#[derive(Debug, Serialize)]
pub struct ExtractValue {
    pub id: String,
    pub value: String,
}

fn body_of<'a>(e: &'a Entry, target: Target) -> Option<&'a str> {
    let b = match target {
        Target::Req => &e.req_body,
        Target::Resp => &e.resp_body,
    };
    b.as_deref().filter(|s| !s.is_empty())
}

fn stringify(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Extract a JSON-path value from request/response bodies across entries.
pub fn compute_extract(
    cap: &Capture,
    filter: &Filter,
    path: &str,
    target: Target,
    top: usize,
    unsafe_include: bool,
) -> ExtractResult {
    let mut values = Vec::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let Some(body) = body_of(e, target) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
            continue;
        };
        for v in jsonpath::eval(&json, path) {
            let s = stringify(&v);
            let shown = if !unsafe_include && is_opaque(&s) {
                REDACTED.to_string()
            } else {
                s
            };
            values.push(ExtractValue { id: e.id.clone(), value: shown });
            if values.len() >= top {
                return ExtractResult { values };
            }
        }
    }
    ExtractResult { values }
}

/// Render extracted values as deterministic terminal text.
pub fn render_extract_text(r: &ExtractResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail extract ==\n");
    for v in &r.values {
        out.push_str(&format!("{}  {}\n", v.id, v.value));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::extract 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/extract.rs
git commit -m "feat: extract command (JSON-path body extraction)"
```

---

### Task 4: `search` command

**Files:**
- Create: `src/analysis/search.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_search;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn with_resp(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/a", 200);
        e.resp_body = Some(body.to_string());
        e
    }

    #[test]
    fn substring_match_with_snippet() {
        let cap = sample_capture(vec![with_resp(0, r#"{"message":"internal error here"}"#)]);
        let r = compute_search(&cap, &Filter::parse(&[]).unwrap(), "internal error", false, false, 10, false).unwrap();
        assert_eq!(r.matches.len(), 1);
        assert_eq!(r.matches[0].location, "resp.body");
        assert!(r.matches[0].snippet.contains("internal error"));
    }

    #[test]
    fn ignore_case() {
        let cap = sample_capture(vec![with_resp(0, "Fatal Boom")]);
        let hit = compute_search(&cap, &Filter::parse(&[]).unwrap(), "fatal", false, true, 10, false).unwrap();
        assert_eq!(hit.matches.len(), 1);
        let miss = compute_search(&cap, &Filter::parse(&[]).unwrap(), "fatal", false, false, 10, false).unwrap();
        assert!(miss.matches.is_empty());
    }

    #[test]
    fn regex_match() {
        let cap = sample_capture(vec![with_resp(0, r#"{"code":"E1234"}"#)]);
        let r = compute_search(&cap, &Filter::parse(&[]).unwrap(), r"E\d{4}", true, false, 10, false).unwrap();
        assert_eq!(r.matches.len(), 1);
    }

    #[test]
    fn invalid_regex_errors() {
        let cap = sample_capture(vec![with_resp(0, "x")]);
        assert!(compute_search(&cap, &Filter::parse(&[]).unwrap(), "(", true, false, 10, false).is_err());
    }

    #[test]
    fn secret_in_snippet_is_redacted() {
        let cap = sample_capture(vec![with_resp(0, "token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ123 end")]);
        let r = compute_search(&cap, &Filter::parse(&[]).unwrap(), "token", false, false, 10, false).unwrap();
        assert!(!r.matches[0].snippet.contains("eyJhbGci"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::search 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_search`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/search.rs`:

```rust
use crate::filter::Filter;
use crate::model::Capture;
use crate::redact::redact_value;
use serde::Serialize;

const CONTEXT: usize = 40;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub id: String,
    pub location: String,
    pub snippet: String,
}

enum Matcher {
    Regex(regex::Regex),
    Substr { needle: String, ignore_case: bool },
}

impl Matcher {
    /// Byte offset of the first match, if any.
    fn find(&self, hay: &str) -> Option<usize> {
        match self {
            Matcher::Regex(re) => re.find(hay).map(|m| m.start()),
            Matcher::Substr { needle, ignore_case } => {
                if *ignore_case {
                    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
                } else {
                    hay.find(needle)
                }
            }
        }
    }
}

fn snippet(body: &str, at: usize, unsafe_include: bool) -> String {
    let start = nearest_boundary(body, at.saturating_sub(CONTEXT), false);
    let end = nearest_boundary(body, (at + CONTEXT).min(body.len()), true);
    redact_value(&body[start..end], unsafe_include)
}

fn nearest_boundary(s: &str, mut i: usize, forward: bool) -> usize {
    i = i.min(s.len());
    while i > 0 && i < s.len() && !s.is_char_boundary(i) {
        if forward {
            i += 1;
        } else {
            i -= 1;
        }
    }
    i
}

/// Search request/response bodies for a pattern; redaction-safe snippets.
pub fn compute_search(
    cap: &Capture,
    filter: &Filter,
    pattern: &str,
    regex: bool,
    ignore_case: bool,
    top: usize,
    unsafe_include: bool,
) -> Result<SearchResult, String> {
    let matcher = if regex {
        let re = regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|e| format!("invalid regex: {e}"))?;
        Matcher::Regex(re)
    } else {
        Matcher::Substr { needle: pattern.to_string(), ignore_case }
    };

    let mut matches = Vec::new();
    'outer: for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        for (loc, body) in [("req.body", &e.req_body), ("resp.body", &e.resp_body)] {
            if let Some(b) = body.as_deref().filter(|s| !s.is_empty()) {
                if let Some(at) = matcher.find(b) {
                    matches.push(SearchMatch {
                        id: e.id.clone(),
                        location: loc.to_string(),
                        snippet: snippet(b, at, unsafe_include),
                    });
                    if matches.len() >= top {
                        break 'outer;
                    }
                }
            }
        }
    }
    Ok(SearchResult { matches })
}

/// Render search matches as deterministic terminal text.
pub fn render_search_text(r: &SearchResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail search ==\n");
    for m in &r.matches {
        out.push_str(&format!("\n{} ({})\n  …{}…\n", m.id, m.location, m.snippet));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::search 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/search.rs
git commit -m "feat: search command (redaction-safe body grep, regex)"
```

---

### Task 5: `export` command

**Files:**
- Create: `src/analysis/export.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/export.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{export_records, render_csv, render_ndjson};
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.x", "POST", "/b", 500),
        ])
    }

    #[test]
    fn ndjson_one_line_per_entry() {
        let recs = export_records(&cap(), &Filter::parse(&[]).unwrap());
        let s = render_ndjson(&recs);
        assert_eq!(s.lines().count(), 2);
        assert!(s.lines().all(|l| l.starts_with('{') && l.contains("\"id\"")));
    }

    #[test]
    fn csv_has_header_and_rows() {
        let recs = export_records(&cap(), &Filter::parse(&[]).unwrap());
        let s = render_csv(&recs);
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[0].starts_with("id,offset_ms,"));
        assert_eq!(lines.len(), 3); // header + 2 rows
    }

    #[test]
    fn csv_quotes_fields_with_commas() {
        let mut e = sample_entry(0, "api.x", "GET", "/a,b", 200);
        e.content_type = Some("text/html; charset=utf-8".into());
        let recs = export_records(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap());
        let s = render_csv(&recs);
        assert!(s.contains("\"/a,b\""));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::export 2>&1 | tail -12`
Expected: FAIL with "cannot find function `export_records`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/export.rs`:

```rust
use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ExportRecord {
    pub id: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub bytes: i64,
    pub content_type: Option<String>,
    pub resource_type: String,
    pub correlation: Option<String>,
}

/// Flatten the filtered capture into one normalized record per entry (redacted
/// by construction — metadata only, no raw bodies/headers).
pub fn export_records(cap: &Capture, filter: &Filter) -> Vec<ExportRecord> {
    cap.entries
        .iter()
        .filter(|e| filter.matches(e))
        .map(|e| ExportRecord {
            id: e.id.clone(),
            offset_ms: e.started_offset_ms,
            duration_ms: e.duration_ms,
            method: e.method.to_ascii_uppercase(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            bytes: e.sizes.resp_content.max(e.sizes.resp_body).max(0),
            content_type: e.content_type.clone(),
            resource_type: format!("{:?}", e.resource_type).to_ascii_lowercase(),
            correlation: e.correlation.first().map(|(_, v)| v.clone()),
        })
        .collect()
}

/// One JSON object per line.
pub fn render_ndjson(records: &[ExportRecord]) -> String {
    records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// RFC4180-ish CSV: header + one row per record.
pub fn render_csv(records: &[ExportRecord]) -> String {
    let mut out = String::new();
    out.push_str("id,offset_ms,duration_ms,method,host,norm_path,status,bytes,content_type,resource_type,correlation\n");
    for r in records {
        let row = [
            r.id.clone(),
            (r.offset_ms as i64).to_string(),
            (r.duration_ms as i64).to_string(),
            r.method.clone(),
            r.host.clone(),
            r.norm_path.clone(),
            r.status.to_string(),
            r.bytes.to_string(),
            r.content_type.clone().unwrap_or_default(),
            r.resource_type.clone(),
            r.correlation.clone().unwrap_or_default(),
        ];
        out.push_str(&row.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::export 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/export.rs
git commit -m "feat: export command (NDJSON/CSV record flattening)"
```

---

### Task 6: Wire the three commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, below the existing analysis imports, add:

```rust
use har::analysis::search::{compute_search, render_search_text};
use har::analysis::extract::{compute_extract, render_extract_text, Target};
use har::analysis::export::{export_records, render_csv, render_ndjson};
```

Also extend the clap import to include `ValueEnum`:

```rust
use clap::{Parser, Subcommand, ValueEnum};
```

- [ ] **Step 2: Add the clap value enums.** In `src/main.rs`, directly above the `#[derive(Subcommand, Debug)] enum Command` declaration, add:

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
enum TargetArg {
    Req,
    Resp,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormatArg {
    Ndjson,
    Csv,
}
```

- [ ] **Step 3: Add the subcommand variants.** Inside `enum Command { ... }`, after the `Validate,` variant (added in M5), add:

```rust
    /// Search request/response bodies (redaction-safe).
    Search {
        /// Pattern to search for.
        pattern: String,
        /// Treat the pattern as a regular expression.
        #[arg(long)]
        regex: bool,
        /// Case-insensitive match.
        #[arg(long = "ignore-case")]
        ignore_case: bool,
    },
    /// Extract a JSON path from request/response bodies.
    Extract {
        /// JSON path, e.g. `$.errors[0].code`.
        path: String,
        /// Which body to query.
        #[arg(long, value_enum, default_value_t = TargetArg::Resp)]
        target: TargetArg,
    },
    /// Flatten entries to NDJSON or CSV.
    Export {
        /// Output format.
        #[arg(long, value_enum, default_value_t = ExportFormatArg::Ndjson)]
        format: ExportFormatArg,
    },
```

- [ ] **Step 4: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::Validate => { ... }` arm, add:

```rust
        Command::Search {
            pattern,
            regex,
            ignore_case,
        } => {
            match compute_search(&cap, &filter, &pattern, regex, ignore_case, cli.top, cli.unsafe_include_secrets) {
                Ok(result) => {
                    emit(
                        cli.json,
                        "search",
                        &cap.meta,
                        &result,
                        &render_search_text(&result),
                        &["show-entry", "extract", "errors"],
                    );
                    exit(false);
                }
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            }
        }
        Command::Extract { path, target } => {
            let target = match target {
                TargetArg::Req => Target::Req,
                TargetArg::Resp => Target::Resp,
            };
            let result = compute_extract(&cap, &filter, &path, target, cli.top, cli.unsafe_include_secrets);
            emit(
                cli.json,
                "extract",
                &cap.meta,
                &result,
                &render_extract_text(&result),
                &["search", "show-entry", "errors"],
            );
            exit(false);
        }
        Command::Export { format } => {
            let records = export_records(&cap, &filter);
            let out = match format {
                ExportFormatArg::Ndjson => render_ndjson(&records),
                ExportFormatArg::Csv => render_csv(&records),
            };
            println!("{out}");
            exit(false);
        }
```

- [ ] **Step 5: Build**

Run: `cargo build 2>&1 | tail -8` (if `Finished` in ~0s without `Compiling`, run `cargo clean -p wiretrail && cargo build`).
Expected: SUCCESS.

- [ ] **Step 6: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har search api --json | head -6
cargo run --quiet -- tests/fixtures/someapi123.har extract '$.version'
cargo run --quiet -- tests/fixtures/someapi123.har export --format csv | head -2
cargo run --quiet -- tests/fixtures/someapi123.har export | head -1
```
Expected: `search --json` prints an envelope with `"command": "search"`; `extract`
prints its header (likely no values for that path); `export --format csv` prints the
CSV header + a row; `export` prints one NDJSON object.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire search/extract/export commands into CLI"
```

---

### Task 7: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_extract.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_extract.rs`:

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
fn search_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "search", "api", "--json"]);
    assert!(stdout.contains("\"command\": \"search\""));
    assert!(stdout.contains("\"matches\""));
}

#[test]
fn extract_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "extract", "$.x", "--json"]);
    assert!(stdout.contains("\"command\": \"extract\""));
    assert!(stdout.contains("\"values\""));
}

#[test]
fn export_ndjson_is_line_oriented() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "export"]);
    assert!(stdout.lines().next().unwrap().starts_with('{'));
    assert!(stdout.contains("\"id\":\"e000000\""));
}

#[test]
fn export_csv_has_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "export", "--format", "csv"]);
    assert!(stdout.starts_with("id,offset_ms,"));
}

#[test]
fn invalid_regex_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "search", "(", "--regex"]);
    assert_eq!(code, 2);
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_extract 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result: FAILED" || echo "all green"`
Expected: `all green`.

- [ ] **Step 4: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
HAR="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$HAR" search "internal server error" 2>/dev/null | head -8
./target/release/wiretrail "$HAR" extract '$.error.message' 2>/dev/null | head -6
./target/release/wiretrail "$HAR" export --format csv 2>/dev/null | head -3
```
Expected: `search` finds the ntsk.cloud 500 bodies; `extract '$.error.message'`
pulls error messages from JSON error responses; `export csv` prints the header + rows.
No secret values appear in snippets (opaque chunks redacted).

- [ ] **Step 5: Commit**

```bash
git add tests/cli_extract.rs
git commit -m "test: end-to-end tests for search/extract/export"
```

---

## Self-review

**Spec coverage (Phase M6):**
- `search` (#68) — substring + regex (`--regex`) + `--ignore-case`, redaction-safe snippet → Task 4. ✓
- `extract` (#69) — hand-rolled JSON path (`$.a.b[0].c`, `[*]`), `--target req|resp`, opaque masking → Tasks 2, 3. ✓
- `export` (#101/#102) — NDJSON + CSV (`--format`), one record per entry → Task 5. ✓ (SQLite deferred, per spec.)
- `regex` dependency added → Task 1. ✓
- All honor `--json` (search/extract via envelope; export emits the chosen format) + filter + `--top`; search/extract honor `--unsafe-include-secrets` → Task 6. ✓

**Placeholder scan:** No TBD/TODO; every code step complete; expected output stated.
Task 4 Step 3 includes a self-correcting note + an explicit fix step (Step 4) to drop
an unused `Entry` import — intentional, not a placeholder. ✓

**Type consistency:**
- `jsonpath::eval(&Value, &str) -> Vec<Value>` (Task 2) used by `extract` (Task 3). ✓
- `compute_extract(&Capture,&Filter,&str,Target,usize,bool)`, `compute_search(&Capture,&Filter,&str,bool,bool,usize,bool) -> Result<SearchResult,String>`, `export_records(&Capture,&Filter) -> Vec<ExportRecord>` + `render_ndjson`/`render_csv` — Task 6 dispatch passes matching args; `TargetArg`→`Target` and `ExportFormatArg` mapped in main. ✓
- `redact_value(&str,bool)` + `REDACTED` (Plan M3) used by search/extract. `opaque::is_opaque` (fix plan) used by extract. ✓
- Result structs derive `Serialize`; `emit`/`exit`/`ExitCode` reused; export prints directly (not via `emit`). ✓
- `Entry` fields used (`req_body`,`resp_body`,`id`,`method`,`host`,`norm_path`,`status`,`started_offset_ms`,`duration_ms`,`content_type`,`resource_type`,`correlation`,`sizes.{resp_content,resp_body}`) all exist. ✓
