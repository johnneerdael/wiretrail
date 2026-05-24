# wiretrail Failures & Timing (`duplicates`, `retries`, `errors`, `redirects`, `slowest`, `transitions`, `timeline`, `show-entry`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the eight failure/timing/inspection commands to `wiretrail` — the heart of the dossier's duplicate index, error groups, redirect storms, status transitions, and per-entry inspection.

**Architecture:** Build on the Plans 1–2 foundation. Add shared helpers — fingerprint grouping + retry classification (`grouping`), timing-phase breakdown + bottleneck classifier (`timing`), JSON error-field parser (`errorbody`), and a body redactor (`redact::redact_body`) — then eight small analysis modules that each fold the filtered `Capture` into a serializable result with a deterministic terminal renderer. A new global `--unsafe-include-secrets` flag controls body/header redaction in `errors` and `show-entry`.

**Tech Stack:** Rust 2024, clap 4, serde/serde_json, ahash, plus the Plan 1–2 modules.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-har-analyzer-design.md` (the
`duplicates`/`retries`/`errors`/`redirects`/`slowest`/`transitions`/`timeline`/`show-entry`
rows, the duplicate-vs-retry detail, and the redaction section).

This is **Plan 3 of 4**. Plans 1–2 are complete and on `main`. Plan 4 (exports:
`report`, `curl`) follows and reuses everything here.

## Prerequisites (from Plans 1–2)

- `har::model::{Capture, Entry, Phases}`, `Entry::status_class()`, `Entry::is_error()`,
  `Entry::id`, and cfg(test) `model::sample_entry(index, host, method, path, status)` /
  `model::sample_capture(Vec<Entry>)`.
- `har::fingerprint::fingerprint(&Entry) -> String`.
- `har::filter::Filter`.
- `har::redact::{redact_header_value, redact_query_value, REDACTED}`.
- `har::render::{Envelope, ExitCode, human_bytes, human_ms}`.
- `main.rs` helpers `emit::<T: Serialize>(json, command, &CaptureMeta, &T, text, &[&str])` and `exit(findings) -> !`.

## File structure

```
src/lib.rs                       # Modify: add pub mod errorbody, grouping, timing
src/timing.rs                    # NEW: PhaseBreakdown + classify_bottleneck
src/grouping.rs                  # NEW: group_by_fingerprint, retry classification
src/errorbody.rs                 # NEW: JSON error-field extraction
src/redact.rs                    # Modify: add redact_body
src/analysis/mod.rs              # Modify: add the 8 new modules
src/analysis/duplicates.rs       # NEW
src/analysis/retries.rs          # NEW
src/analysis/errors.rs           # NEW
src/analysis/redirects.rs        # NEW
src/analysis/slowest.rs          # NEW
src/analysis/transitions.rs      # NEW
src/analysis/timeline.rs         # NEW
src/analysis/show_entry.rs       # NEW
src/main.rs                      # Modify: add 8 subcommands + --unsafe-include-secrets
tests/cli_failures.rs            # NEW: end-to-end binary tests
```

---

### Task 1: Timing breakdown + bottleneck classifier

**Files:**
- Create: `src/timing.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Declare the module.** In `src/lib.rs`, after `pub mod vendor;`, add:

```rust
pub mod errorbody;
pub mod grouping;
pub mod timing;
```

- [ ] **Step 2: Write the failing tests** at the top of `src/timing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{classify_bottleneck, PhaseBreakdown};
    use crate::model::Phases;

    #[test]
    fn picks_dominant_phase() {
        let p = Phases { wait: 500.0, receive: 10.0, send: 1.0, ..Phases::default() };
        assert_eq!(classify_bottleneck(&p), "server wait/TTFB");
    }

    #[test]
    fn dns_dominant() {
        let p = Phases { dns: Some(300.0), wait: 5.0, ..Phases::default() };
        assert_eq!(classify_bottleneck(&p), "DNS");
    }

    #[test]
    fn all_zero_is_unknown() {
        assert_eq!(classify_bottleneck(&Phases::default()), "unknown");
    }

    #[test]
    fn breakdown_copies_phases() {
        let p = Phases { dns: Some(3.0), wait: 9.0, ..Phases::default() };
        let b = PhaseBreakdown::from_phases(&p);
        assert_eq!(b.dns, Some(3.0));
        assert_eq!(b.wait, 9.0);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib timing 2>&1 | tail -12`
Expected: FAIL with "cannot find function `classify_bottleneck`".

- [ ] **Step 4: Implement** above the test module in `src/timing.rs`:

```rust
use crate::model::Phases;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PhaseBreakdown {
    pub blocked: Option<f64>,
    pub dns: Option<f64>,
    pub connect: Option<f64>,
    pub ssl: Option<f64>,
    pub send: f64,
    pub wait: f64,
    pub receive: f64,
}

impl PhaseBreakdown {
    pub fn from_phases(p: &Phases) -> Self {
        PhaseBreakdown {
            blocked: p.blocked,
            dns: p.dns,
            connect: p.connect,
            ssl: p.ssl,
            send: p.send,
            wait: p.wait,
            receive: p.receive,
        }
    }
}

/// Label the dominant timing phase, or "unknown" when nothing is positive.
pub fn classify_bottleneck(p: &Phases) -> &'static str {
    let candidates = [
        ("queueing/blocked", p.blocked.unwrap_or(0.0)),
        ("DNS", p.dns.unwrap_or(0.0)),
        ("TCP connect", p.connect.unwrap_or(0.0)),
        ("TLS handshake", p.ssl.unwrap_or(0.0)),
        ("request upload", p.send),
        ("server wait/TTFB", p.wait),
        ("download/receive", p.receive),
    ];
    let mut best: (&'static str, f64) = ("unknown", 0.0);
    for (label, v) in candidates {
        if v > best.1 {
            best = (label, v);
        }
    }
    best.0
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib timing 2>&1 | tail -8`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/timing.rs
git commit -m "feat: timing phase breakdown + bottleneck classifier"
```

---

### Task 2: Fingerprint grouping + retry classification

**Files:**
- Create: `src/grouping.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/grouping.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{group_by_fingerprint, group_has_retry, retry_entry_ids};
    use crate::model::{sample_capture, sample_entry, Entry};

    fn refs(cap: &crate::model::Capture) -> Vec<&Entry> {
        cap.entries.iter().collect()
    }

    #[test]
    fn groups_and_sorts_by_size() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/a", 200),
            sample_entry(2, "h", "GET", "/b", 200),
        ]);
        let groups = group_by_fingerprint(&refs(&cap));
        // /a group (2) sorts before /b group (1)
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn retries_need_a_prior_failure() {
        // three identical calls: 500, 500, 200 -> 2nd and 3rd are retries
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 500),
            sample_entry(1, "h", "POST", "/x", 500),
            sample_entry(2, "h", "POST", "/x", 200),
        ]);
        let ids = retry_entry_ids(&refs(&cap));
        assert!(ids.contains("e000001"));
        assert!(ids.contains("e000002"));
        assert!(!ids.contains("e000000"));
    }

    #[test]
    fn pure_duplicates_are_not_retries() {
        // all 200 -> wasteful duplicates, no retries
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 200),
            sample_entry(1, "h", "POST", "/x", 200),
        ]);
        assert!(retry_entry_ids(&refs(&cap)).is_empty());
        let groups = group_by_fingerprint(&refs(&cap));
        assert!(!group_has_retry(&groups[0].1));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib grouping 2>&1 | tail -12`
Expected: FAIL with "cannot find function `group_by_fingerprint`".

- [ ] **Step 3: Implement** above the test module in `src/grouping.rs`:

```rust
use crate::fingerprint::fingerprint;
use crate::model::Entry;
use ahash::{AHashMap, AHashSet};

/// Group entries by duplicate fingerprint. Each group's entries are sorted by
/// (started_offset_ms, index). Groups are returned sorted by descending size,
/// then fingerprint, for determinism.
pub fn group_by_fingerprint<'a>(entries: &[&'a Entry]) -> Vec<(String, Vec<&'a Entry>)> {
    let mut map: AHashMap<String, Vec<&'a Entry>> = AHashMap::new();
    for e in entries {
        map.entry(fingerprint(e)).or_default().push(e);
    }
    let mut groups: Vec<(String, Vec<&'a Entry>)> = map.into_iter().collect();
    for (_, g) in groups.iter_mut() {
        g.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    groups
}

/// A request whose status indicates a transient failure worth retrying.
pub fn is_retry_trigger(e: &Entry) -> bool {
    e.status == 0 || e.status == 429 || e.status_class() == 5
}

/// True if a time-ordered fingerprint group contains an attempt that follows a
/// failed earlier attempt (i.e. the group exhibits retry behavior).
pub fn group_has_retry(group: &[&Entry]) -> bool {
    let mut seen_failure = false;
    for e in group {
        if seen_failure {
            return true;
        }
        if is_retry_trigger(e) {
            seen_failure = true;
        }
    }
    false
}

/// IDs of entries classified as retries across all fingerprint groups.
pub fn retry_entry_ids(entries: &[&Entry]) -> AHashSet<String> {
    let mut out = AHashSet::new();
    for (_, group) in group_by_fingerprint(entries) {
        let mut seen_failure = false;
        for e in &group {
            if seen_failure {
                out.insert(e.id.clone());
            }
            if is_retry_trigger(e) {
                seen_failure = true;
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib grouping 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/grouping.rs
git commit -m "feat: fingerprint grouping + retry classification"
```

---

### Task 3: Error-body field parser

**Files:**
- Create: `src/errorbody.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/errorbody.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::parse_error_fields;

    #[test]
    fn extracts_message_and_code_flat() {
        let f = parse_error_fields(r#"{"message":"Not found","code":"NF404"}"#);
        assert_eq!(f.message.as_deref(), Some("Not found"));
        assert_eq!(f.code.as_deref(), Some("NF404"));
    }

    #[test]
    fn extracts_from_nested_error_object() {
        let f = parse_error_fields(r#"{"error":{"message":"bad token","code":"E1"}}"#);
        assert_eq!(f.message.as_deref(), Some("bad token"));
        assert_eq!(f.code.as_deref(), Some("E1"));
    }

    #[test]
    fn numeric_code_becomes_string() {
        let f = parse_error_fields(r#"{"error":"boom","status":500}"#);
        assert_eq!(f.message.as_deref(), Some("boom"));
        assert_eq!(f.code.as_deref(), Some("500"));
    }

    #[test]
    fn non_json_is_empty() {
        let f = parse_error_fields("<html>500</html>");
        assert!(f.message.is_none());
        assert!(f.code.is_none());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib errorbody 2>&1 | tail -12`
Expected: FAIL with "cannot find function `parse_error_fields`".

- [ ] **Step 3: Implement** above the test module in `src/errorbody.rs`:

```rust
use serde_json::Value;

#[derive(Debug, Default)]
pub struct ErrorFields {
    pub message: Option<String>,
    pub code: Option<String>,
}

const MESSAGE_KEYS: &[&str] = &["message", "error_description", "error", "reason", "detail", "details"];
const CODE_KEYS: &[&str] = &["code", "error_code", "status"];

/// Extract common error fields from a JSON response body. Returns empty fields
/// for non-JSON or when keys are absent.
pub fn parse_error_fields(body: &str) -> ErrorFields {
    let mut fields = ErrorFields::default();
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return fields;
    };
    fields.message = first_string(&v, MESSAGE_KEYS);
    fields.code = first_string(&v, CODE_KEYS);
    fields
}

fn first_string(v: &Value, keys: &[&str]) -> Option<String> {
    let obj = v.as_object()?;
    for k in keys {
        match obj.get(*k) {
            Some(Value::String(s)) => return Some(s.clone()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    // Fall back to a nested "error" object.
    if let Some(Value::Object(err)) = obj.get("error") {
        for k in keys {
            match err.get(*k) {
                Some(Value::String(s)) => return Some(s.clone()),
                Some(Value::Number(n)) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib errorbody 2>&1 | tail -8`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/errorbody.rs
git commit -m "feat: JSON error-body field parser"
```

---

### Task 4: Body redaction

**Files:**
- Modify: `src/redact.rs`

- [ ] **Step 1: Append the failing tests** to the existing `#[cfg(test)] mod tests` in `src/redact.rs` (add these functions inside the test module, before its closing brace):

```rust
    #[test]
    fn redacts_sensitive_json_keys() {
        let body = r#"{"user":"bob","access_token":"abc","nested":{"password":"x"}}"#;
        let out = super::redact_body(body, false, 1000);
        assert!(out.contains("bob"));
        assert!(!out.contains("abc"));
        assert!(!out.contains("\"x\""));
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn unsafe_body_passthrough() {
        let body = r#"{"access_token":"abc"}"#;
        let out = super::redact_body(body, true, 1000);
        assert!(out.contains("abc"));
    }

    #[test]
    fn truncates_long_body() {
        let body = "x".repeat(500);
        let out = super::redact_body(&body, false, 10);
        assert!(out.chars().count() <= 11); // 10 + ellipsis
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib redact 2>&1 | tail -12`
Expected: FAIL with "cannot find function `redact_body`".

- [ ] **Step 3: Implement.** Add the following to `src/redact.rs` directly above its `#[cfg(test)] mod tests` block:

```rust
const SENSITIVE_BODY_KEYS: &[&str] = &[
    "password",
    "token",
    "secret",
    "authorization",
    "access_token",
    "refresh_token",
    "id_token",
    "api_key",
    "apikey",
    "client_secret",
];

/// Redact and truncate a request/response body for safe display. JSON bodies
/// have sensitive keys recursively replaced; non-JSON bodies are truncated as-is.
/// `max` bounds the character count of the returned string.
pub fn redact_body(body: &str, unsafe_include: bool, max: usize) -> String {
    if unsafe_include {
        return truncate(body, max);
    }
    if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) {
        redact_json(&mut v);
        let s = serde_json::to_string(&v).unwrap_or_default();
        return truncate(&s, max);
    }
    truncate(body, max)
}

fn redact_json(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                let lk = k.to_ascii_lowercase();
                if SENSITIVE_BODY_KEYS.iter().any(|s| lk.contains(s)) {
                    *val = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redact_json(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for e in arr.iter_mut() {
                redact_json(e);
            }
        }
        _ => {}
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib redact 2>&1 | tail -10`
Expected: PASS (7 tests: 4 from Plan 1 + 3 new).

- [ ] **Step 5: Commit**

```bash
git add src/redact.rs
git commit -m "feat: body redaction (JSON key scrub + truncation)"
```

---

### Task 5: `duplicates` analysis

**Files:**
- Modify: `src/analysis/mod.rs`
- Create: `src/analysis/duplicates.rs`

- [ ] **Step 1: Register all eight new analysis modules.** Replace the entire contents of `src/analysis/mod.rs` with:

```rust
pub mod duplicates;
pub mod endpoints;
pub mod errors;
pub mod hosts;
pub mod redirects;
pub mod retries;
pub mod show_entry;
pub mod slowest;
pub mod subsystems;
pub mod summary;
pub mod timeline;
pub mod transitions;
```

- [ ] **Step 2: Create empty placeholder files** so the module declarations resolve while we build them one task at a time.

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
for f in duplicates errors redirects retries show_entry slowest timeline transitions; do touch "src/analysis/$f.rs"; done
```

- [ ] **Step 3: Write the failing tests** at the top of `src/analysis/duplicates.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_duplicates;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "h", "POST", "/resolve", 200),
            sample_entry(1, "h", "POST", "/resolve", 200),
            sample_entry(2, "h", "POST", "/resolve", 200),
            sample_entry(3, "h", "GET", "/once", 200), // unique -> not a duplicate
        ])
    }

    #[test]
    fn reports_only_groups_with_repeats() {
        let r = compute_duplicates(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.count, 3);
        assert_eq!(g.method, "POST");
        assert_eq!(g.norm_path, "/resolve");
        assert_eq!(g.entry_ids, vec!["e000000", "e000001", "e000002"]);
        assert!(!g.is_retry_pattern); // all 200
    }

    #[test]
    fn flags_retry_pattern() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 500),
            sample_entry(1, "h", "POST", "/x", 200),
        ]);
        let r = compute_duplicates(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups[0].is_retry_pattern);
    }
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test --lib duplicates 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_duplicates`".

- [ ] **Step 5: Implement** above the test module in `src/analysis/duplicates.rs`:

```rust
use crate::filter::Filter;
use crate::grouping::{group_by_fingerprint, group_has_retry};
use crate::model::Capture;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct DuplicatesResult {
    pub groups: Vec<DuplicateGroup>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub count: usize,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub statuses: BTreeMap<String, usize>,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub is_retry_pattern: bool,
}

/// Group repeated requests (count >= 2) by fingerprint. `top` bounds the list.
pub fn compute_duplicates(cap: &Capture, filter: &Filter, top: usize) -> DuplicatesResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut groups: Vec<DuplicateGroup> = group_by_fingerprint(&entries)
        .into_iter()
        .filter(|(_, g)| g.len() >= 2)
        .map(|(fp, g)| {
            let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
            for e in &g {
                *statuses.entry(e.status.to_string()).or_default() += 1;
            }
            DuplicateGroup {
                fingerprint: fp,
                count: g.len(),
                method: g[0].method.to_ascii_uppercase(),
                host: g[0].host.clone(),
                norm_path: g[0].norm_path.clone(),
                statuses,
                entry_ids: g.iter().map(|e| e.id.clone()).collect(),
                first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
                last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
                is_retry_pattern: group_has_retry(&g),
            }
        })
        .collect();

    groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.fingerprint.cmp(&b.fingerprint)));
    groups.truncate(top);
    DuplicatesResult { groups }
}

/// Render duplicates as deterministic terminal text.
pub fn render_duplicates_text(r: &DuplicatesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail duplicates ==\n");
    for g in &r.groups {
        let tag = if g.is_retry_pattern { " [retry pattern]" } else { "" };
        out.push_str(&format!(
            "\n{:>4}x{}  {} {}{}\n",
            g.count, tag, g.method, g.host, g.norm_path
        ));
        let statuses: Vec<String> = g.statuses.iter().map(|(s, c)| format!("{s}:{c}")).collect();
        out.push_str(&format!("  statuses: {}\n", statuses.join(" ")));
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib duplicates 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add src/analysis/mod.rs src/analysis/duplicates.rs
git commit -m "feat: duplicates command analysis + renderer"
```

---

### Task 6: `retries` analysis

**Files:**
- Create: `src/analysis/retries.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/retries.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_retries;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "h", "POST", "/bulk", 500),
            sample_entry(1, "h", "POST", "/bulk", 500),
            sample_entry(2, "h", "POST", "/bulk", 200),
            sample_entry(3, "h", "GET", "/clean", 200),
            sample_entry(4, "h", "GET", "/clean", 200), // pure duplicate, not a retry
        ];
        entries[1].started_offset_ms = 100.0;
        entries[2].started_offset_ms = 300.0;
        sample_capture(entries)
    }

    #[test]
    fn reports_only_retry_groups() {
        let r = compute_retries(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.attempts, 3);
        assert_eq!(g.retry_count, 2);
        assert_eq!(g.final_status, 200);
        assert!(g.trigger_statuses.contains(&500));
        // gaps between consecutive attempts: 100-0=100, 300-100=200
        assert_eq!(g.gaps_ms, vec![100.0, 200.0]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib retries 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_retries`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/retries.rs`:

```rust
use crate::filter::Filter;
use crate::grouping::{group_by_fingerprint, group_has_retry, is_retry_trigger};
use crate::model::Capture;
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RetriesResult {
    pub groups: Vec<RetryGroup>,
}

#[derive(Debug, Serialize)]
pub struct RetryGroup {
    pub fingerprint: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub attempts: usize,
    pub retry_count: usize,
    pub trigger_statuses: Vec<i64>,
    pub gaps_ms: Vec<f64>,
    pub entry_ids: Vec<String>,
    pub final_status: i64,
}

/// Report fingerprint groups that exhibit retry behavior (an attempt following
/// a failed earlier attempt). `top` bounds the list.
pub fn compute_retries(cap: &Capture, filter: &Filter, top: usize) -> RetriesResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut groups: Vec<RetryGroup> = group_by_fingerprint(&entries)
        .into_iter()
        .filter(|(_, g)| group_has_retry(g))
        .map(|(fp, g)| retry_group(fp, &g))
        .collect();

    groups.sort_by(|a, b| {
        b.retry_count
            .cmp(&a.retry_count)
            .then(a.fingerprint.cmp(&b.fingerprint))
    });
    groups.truncate(top);
    RetriesResult { groups }
}

fn retry_group(fingerprint: String, g: &[&crate::model::Entry]) -> RetryGroup {
    let mut retry_count = 0usize;
    let mut trigger_statuses: Vec<i64> = Vec::new();
    let mut seen_failure = false;
    for e in g {
        if seen_failure {
            retry_count += 1;
        }
        if is_retry_trigger(e) {
            seen_failure = true;
            if !trigger_statuses.contains(&e.status) {
                trigger_statuses.push(e.status);
            }
        }
    }
    let gaps_ms: Vec<f64> = g
        .windows(2)
        .map(|w| (w[1].started_offset_ms - w[0].started_offset_ms).max(0.0))
        .collect();

    RetryGroup {
        fingerprint,
        method: g[0].method.to_ascii_uppercase(),
        host: g[0].host.clone(),
        norm_path: g[0].norm_path.clone(),
        attempts: g.len(),
        retry_count,
        trigger_statuses,
        gaps_ms,
        entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        final_status: g.last().map(|e| e.status).unwrap_or(0),
    }
}

/// Render retries as deterministic terminal text.
pub fn render_retries_text(r: &RetriesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail retries ==\n");
    for g in &r.groups {
        let triggers: Vec<String> = g.trigger_statuses.iter().map(|s| s.to_string()).collect();
        out.push_str(&format!(
            "\n{} {}{}  ({} attempts, {} retries, final {})\n",
            g.method, g.host, g.norm_path, g.attempts, g.retry_count, g.final_status
        ));
        out.push_str(&format!("  triggered by: {}\n", triggers.join(", ")));
        let gaps: Vec<String> = g.gaps_ms.iter().map(|ms| human_ms(*ms)).collect();
        out.push_str(&format!("  backoff gaps: {}\n", gaps.join(", ")));
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib retries 2>&1 | tail -8`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/retries.rs
git commit -m "feat: retries command analysis + renderer"
```

---

### Task 7: `errors` analysis

**Files:**
- Create: `src/analysis/errors.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/errors.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_errors;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e0 = sample_entry(0, "api.x", "POST", "/bulk", 500);
        e0.resp_body = Some(r#"{"message":"boom","code":"E500"}"#.to_string());
        let mut e1 = sample_entry(1, "api.x", "POST", "/bulk", 500);
        e1.resp_body = Some(r#"{"message":"boom","code":"E500"}"#.to_string());
        let e2 = sample_entry(2, "api.x", "GET", "/ok", 200); // not an error
        let e3 = sample_entry(3, "api.x", "GET", "/missing", 404);
        sample_capture(vec![e0, e1, e2, e3])
    }

    #[test]
    fn groups_4xx_5xx_only() {
        let r = compute_errors(&cap(), &Filter::parse(&[]).unwrap(), 10, false);
        // /bulk 500 (x2) and /missing 404 (x1) -> 2 groups; /ok excluded
        assert_eq!(r.groups.len(), 2);
        let bulk = r.groups.iter().find(|g| g.norm_path == "/bulk").unwrap();
        assert_eq!(bulk.count, 2);
        assert_eq!(bulk.status, 500);
        assert_eq!(bulk.error_message.as_deref(), Some("boom"));
        assert_eq!(bulk.error_code.as_deref(), Some("E500"));
        assert_eq!(bulk.entry_ids, vec!["e000000", "e000001"]);
    }

    #[test]
    fn redacts_body_snippet_by_default() {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 401);
        e.resp_body = Some(r#"{"access_token":"leak","message":"no"}"#.to_string());
        let r = compute_errors(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), 10, false);
        let snip = r.groups[0].body_snippet.as_deref().unwrap();
        assert!(!snip.contains("leak"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib errors 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_errors`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/errors.rs`:

```rust
use crate::errorbody::parse_error_fields;
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::redact_body;
use ahash::AHashMap;
use serde::Serialize;

const SNIPPET_MAX: usize = 200;

#[derive(Debug, Serialize)]
pub struct ErrorsResult {
    pub groups: Vec<ErrorGroup>,
}

#[derive(Debug, Serialize)]
pub struct ErrorGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub body_snippet: Option<String>,
    pub correlation_ids: Vec<String>,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

/// Group 4xx/5xx/failed responses by (host, method, norm_path, status).
/// `unsafe_include` disables body redaction. `top` bounds the list.
pub fn compute_errors(
    cap: &Capture,
    filter: &Filter,
    top: usize,
    unsafe_include: bool,
) -> ErrorsResult {
    let mut by_key: AHashMap<(String, String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e) && e.is_error()) {
        let key = (e.host.clone(), e.method.to_ascii_uppercase(), e.norm_path.clone(), e.status);
        by_key.entry(key).or_default().push(e);
    }

    let mut groups: Vec<ErrorGroup> = by_key
        .into_iter()
        .map(|((host, method, norm_path, status), mut g)| {
            g.sort_by(|a, b| {
                a.started_offset_ms
                    .partial_cmp(&b.started_offset_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.index.cmp(&b.index))
            });
            error_group(host, method, norm_path, status, &g, unsafe_include)
        })
        .collect();

    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.status.cmp(&a.status))
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    ErrorsResult { groups }
}

fn error_group(
    host: String,
    method: String,
    norm_path: String,
    status: i64,
    g: &[&Entry],
    unsafe_include: bool,
) -> ErrorGroup {
    let sample = g[0];
    let fields = sample
        .resp_body
        .as_deref()
        .map(parse_error_fields)
        .unwrap_or_default();
    let body_snippet = sample
        .resp_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, SNIPPET_MAX));
    let correlation_ids: Vec<String> =
        sample.correlation.iter().map(|(_, v)| v.clone()).collect();

    ErrorGroup {
        host,
        method,
        norm_path,
        status,
        count: g.len(),
        error_message: fields.message,
        error_code: fields.code,
        body_snippet,
        correlation_ids,
        entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
        last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
    }
}

/// Render errors as deterministic terminal text.
pub fn render_errors_text(r: &ErrorsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail errors ==\n");
    for g in &r.groups {
        out.push_str(&format!(
            "\n{:>4}x  [{}] {} {}{}\n",
            g.count, g.status, g.method, g.host, g.norm_path
        ));
        if let Some(m) = &g.error_message {
            out.push_str(&format!("  message: {m}\n"));
        }
        if let Some(c) = &g.error_code {
            out.push_str(&format!("  code: {c}\n"));
        }
        if !g.correlation_ids.is_empty() {
            out.push_str(&format!("  correlation: {}\n", g.correlation_ids.join(", ")));
        }
        if let Some(s) = &g.body_snippet {
            out.push_str(&format!("  body: {s}\n"));
        }
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib errors 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/errors.rs
git commit -m "feat: errors command analysis + renderer"
```

---

### Task 8: `redirects` analysis

**Files:**
- Create: `src/analysis/redirects.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/redirects.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_redirects;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn redirect(index: usize, host: &str, path: &str, status: i64, target: &str) -> crate::model::Entry {
        let mut e = sample_entry(index, host, "GET", path, status);
        e.redirect_url = Some(target.to_string());
        e
    }

    fn cap() -> crate::model::Capture {
        let mut entries = Vec::new();
        // 6 x 308 to torii manifest -> storm
        for i in 0..6 {
            entries.push(redirect(i, "torii.app", "/manifest.json", 308, "https://torii.app/v2/manifest.json"));
        }
        // one cross-host 302
        entries.push(redirect(6, "a.com", "/go", 302, "https://b.com/landing"));
        // a normal 200 (ignored)
        entries.push(sample_entry(7, "a.com", "GET", "/ok", 200));
        sample_capture(entries)
    }

    #[test]
    fn groups_redirects_and_flags_storm() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let storm = r.groups.iter().find(|g| g.norm_path == "/manifest.json").unwrap();
        assert_eq!(storm.count, 6);
        assert_eq!(storm.status, 308);
        assert!(storm.is_storm);
        assert!(!storm.cross_host);
    }

    #[test]
    fn flags_cross_host() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let x = r.groups.iter().find(|g| g.norm_path == "/go").unwrap();
        assert!(x.cross_host);
        assert_eq!(x.target_host.as_deref(), Some("b.com"));
    }

    #[test]
    fn ignores_non_redirects() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups.iter().all(|g| g.norm_path != "/ok"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib redirects 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_redirects`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/redirects.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

const STORM_THRESHOLD: usize = 5;
const REDIRECT_STATUSES: &[i64] = &[301, 302, 303, 307, 308];

#[derive(Debug, Serialize)]
pub struct RedirectsResult {
    pub groups: Vec<RedirectGroup>,
}

#[derive(Debug, Serialize)]
pub struct RedirectGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub target_host: Option<String>,
    pub cross_host: bool,
    pub is_storm: bool,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

fn is_redirect(e: &Entry) -> bool {
    REDIRECT_STATUSES.contains(&e.status)
        || e.redirect_url.as_deref().is_some_and(|u| !u.is_empty())
}

fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_string()))
}

/// Group redirect responses by (host, method, norm_path, status); flag storms
/// (count >= 5) and cross-host hops. `top` bounds the list.
pub fn compute_redirects(cap: &Capture, filter: &Filter, top: usize) -> RedirectsResult {
    let mut by_key: AHashMap<(String, String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e) && is_redirect(e)) {
        let key = (e.host.clone(), e.method.to_ascii_uppercase(), e.norm_path.clone(), e.status);
        by_key.entry(key).or_default().push(e);
    }

    let mut groups: Vec<RedirectGroup> = by_key
        .into_iter()
        .map(|((host, method, norm_path, status), mut g)| {
            g.sort_by(|a, b| {
                a.started_offset_ms
                    .partial_cmp(&b.started_offset_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.index.cmp(&b.index))
            });
            let target_host = g
                .iter()
                .find_map(|e| e.redirect_url.as_deref())
                .and_then(host_of);
            let cross_host = target_host.as_deref().is_some_and(|t| !t.is_empty() && t != host);
            RedirectGroup {
                count: g.len(),
                is_storm: g.len() >= STORM_THRESHOLD,
                cross_host,
                target_host,
                entry_ids: g.iter().map(|e| e.id.clone()).collect(),
                first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
                last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
                host,
                method,
                norm_path,
                status,
            }
        })
        .collect();

    groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.host.cmp(&b.host)).then(a.norm_path.cmp(&b.norm_path)));
    groups.truncate(top);
    RedirectsResult { groups }
}

/// Render redirects as deterministic terminal text.
pub fn render_redirects_text(r: &RedirectsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail redirects ==\n");
    for g in &r.groups {
        let mut tags = Vec::new();
        if g.is_storm {
            tags.push("storm");
        }
        if g.cross_host {
            tags.push("cross-host");
        }
        let tagstr = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) };
        out.push_str(&format!(
            "\n{:>4}x  [{}] {} {}{}{}\n",
            g.count, g.status, g.method, g.host, g.norm_path, tagstr
        ));
        if let Some(t) = &g.target_host {
            out.push_str(&format!("  -> {t}\n"));
        }
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib redirects 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/redirects.rs
git commit -m "feat: redirects command analysis + renderer"
```

---

### Task 9: `slowest` analysis

**Files:**
- Create: `src/analysis/slowest.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/slowest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_slowest;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Phases};

    fn cap() -> crate::model::Capture {
        let mut fast = sample_entry(0, "h", "GET", "/fast", 200);
        fast.duration_ms = 5.0;
        let mut slow = sample_entry(1, "h", "GET", "/slow", 200);
        slow.duration_ms = 900.0;
        slow.timings = Phases { wait: 850.0, receive: 40.0, ..Phases::default() };
        sample_capture(vec![fast, slow])
    }

    #[test]
    fn orders_by_duration_desc_with_bottleneck() {
        let r = compute_slowest(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.entries[0].norm_path, "/slow");
        assert_eq!(r.entries[0].duration_ms, 900.0);
        assert_eq!(r.entries[0].bottleneck, "server wait/TTFB");
    }

    #[test]
    fn top_bounds_list() {
        let r = compute_slowest(&cap(), &Filter::parse(&[]).unwrap(), 1);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].norm_path, "/slow");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib slowest 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_slowest`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/slowest.rs`:

```rust
use crate::filter::Filter;
use crate::model::Capture;
use crate::render::human_ms;
use crate::timing::{classify_bottleneck, PhaseBreakdown};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SlowestResult {
    pub entries: Vec<SlowRow>,
}

#[derive(Debug, Serialize)]
pub struct SlowRow {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub duration_ms: f64,
    pub phases: PhaseBreakdown,
    pub bottleneck: String,
}

/// Top-N slowest requests globally, with timing breakdown and bottleneck label.
pub fn compute_slowest(cap: &Capture, filter: &Filter, top: usize) -> SlowestResult {
    let mut entries: Vec<SlowRow> = cap
        .entries
        .iter()
        .filter(|e| filter.matches(e))
        .map(|e| SlowRow {
            id: e.id.clone(),
            method: e.method.to_ascii_uppercase(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            duration_ms: e.duration_ms,
            phases: PhaseBreakdown::from_phases(&e.timings),
            bottleneck: classify_bottleneck(&e.timings).to_string(),
        })
        .collect();

    entries.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    entries.truncate(top);
    SlowestResult { entries }
}

/// Render slowest requests as deterministic terminal text.
pub fn render_slowest_text(r: &SlowestResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail slowest ==\n");
    for e in &r.entries {
        out.push_str(&format!(
            "\n{:>8}  {} {} {}{}  [{}]\n",
            human_ms(e.duration_ms),
            e.id,
            e.method,
            e.host,
            e.norm_path,
            e.status
        ));
        out.push_str(&format!("  bottleneck: {}\n", e.bottleneck));
        out.push_str(&format!(
            "  phases: wait {} / receive {} / send {} / connect {} / dns {} / ssl {} / blocked {}\n",
            human_ms(e.phases.wait),
            human_ms(e.phases.receive),
            human_ms(e.phases.send),
            human_ms(e.phases.connect.unwrap_or(0.0)),
            human_ms(e.phases.dns.unwrap_or(0.0)),
            human_ms(e.phases.ssl.unwrap_or(0.0)),
            human_ms(e.phases.blocked.unwrap_or(0.0)),
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib slowest 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/slowest.rs
git commit -m "feat: slowest command analysis + renderer"
```

---

### Task 10: `transitions` analysis

**Files:**
- Create: `src/analysis/transitions.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/transitions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_transitions;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn detects_auth_recovery() {
        // same endpoint: 401 then 200 -> auth-recovered
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/me", 401),
            sample_entry(1, "h", "GET", "/me", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.transitions.len(), 1);
        let t = &r.transitions[0];
        assert_eq!(t.from_status, 401);
        assert_eq!(t.to_status, 200);
        assert_eq!(t.label, "auth-recovered");
        assert_eq!(t.from_id, "e000000");
        assert_eq!(t.to_id, "e000001");
    }

    #[test]
    fn detects_rate_limit_persisted_and_recovered_5xx() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 429),
            sample_entry(1, "h", "GET", "/a", 429),
            sample_entry(2, "h", "POST", "/b", 500),
            sample_entry(3, "h", "POST", "/b", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.transitions.iter().any(|t| t.label == "rate-limit-persisted"));
        assert!(r.transitions.iter().any(|t| t.label == "recovered-5xx"));
    }

    #[test]
    fn no_transition_when_no_prior_error() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/a", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.transitions.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib transitions 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_transitions`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/transitions.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TransitionsResult {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Serialize)]
pub struct Transition {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub from_status: i64,
    pub to_status: i64,
    pub from_id: String,
    pub to_id: String,
    pub gap_ms: f64,
    pub label: String,
}

/// Detect endpoint-local status transitions where a failed attempt is followed
/// by another attempt of the same (method, host, norm_path). `top` bounds the list.
pub fn compute_transitions(cap: &Capture, filter: &Filter, top: usize) -> TransitionsResult {
    // Group by endpoint, preserving time order.
    let mut by_key: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let key = (e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone());
        by_key.entry(key).or_default().push(e);
    }

    let mut transitions: Vec<Transition> = Vec::new();
    for (_, mut group) in by_key {
        group.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });
        for w in group.windows(2) {
            let (prev, curr) = (w[0], w[1]);
            if let Some(label) = label_for(prev.status, curr.status) {
                transitions.push(Transition {
                    host: prev.host.clone(),
                    method: prev.method.to_ascii_uppercase(),
                    norm_path: prev.norm_path.clone(),
                    from_status: prev.status,
                    to_status: curr.status,
                    from_id: prev.id.clone(),
                    to_id: curr.id.clone(),
                    gap_ms: (curr.started_offset_ms - prev.started_offset_ms).max(0.0),
                    label: label.to_string(),
                });
            }
        }
    }

    transitions.sort_by(|a, b| {
        a.from_id.cmp(&b.from_id).then(a.to_id.cmp(&b.to_id))
    });
    transitions.truncate(top);
    TransitionsResult { transitions }
}

/// Classify a transition between two consecutive same-endpoint attempts. Returns
/// None when the prior attempt did not fail (no transition worth reporting).
fn label_for(prev: i64, curr: i64) -> Option<&'static str> {
    let prev_class = class_of(prev);
    match (prev, curr) {
        (401 | 403, c) if class_of(c) == 2 => Some("auth-recovered"),
        (429, 429) => Some("rate-limit-persisted"),
        (429, c) if class_of(c) == 2 => Some("rate-limit-recovered"),
        (p, c) if class_of(p) == 5 && class_of(c) == 2 => {
            let _ = p;
            Some("recovered-5xx")
        }
        (p, c) if is_failure(p) && c != p && is_failure(c) => {
            let _ = (prev_class, p, c);
            Some("error-changed")
        }
        _ => None,
    }
}

fn class_of(status: i64) -> i64 {
    if (100..600).contains(&status) {
        status / 100
    } else {
        0
    }
}

fn is_failure(status: i64) -> bool {
    status == 0 || class_of(status) == 4 || class_of(status) == 5
}

/// Render transitions as deterministic terminal text.
pub fn render_transitions_text(r: &TransitionsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail transitions ==\n");
    for t in &r.transitions {
        out.push_str(&format!(
            "\n{} -> {}  [{}]  {} {}{}\n",
            t.from_status, t.to_status, t.label, t.method, t.host, t.norm_path
        ));
        out.push_str(&format!(
            "  {} -> {}  (gap {})\n",
            t.from_id,
            t.to_id,
            human_ms(t.gap_ms)
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib transitions 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/transitions.rs
git commit -m "feat: transitions command analysis + renderer"
```

---

### Task 11: `timeline` analysis

**Files:**
- Create: `src/analysis/timeline.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/timeline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_timeline;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "h", "POST", "/x", 500), // retry trigger
            sample_entry(1, "h", "POST", "/x", 200), // retry
            sample_entry(2, "h", "GET", "/y", 200),  // unique
        ];
        entries[1].started_offset_ms = 50.0;
        entries[2].started_offset_ms = 20.0;
        sample_capture(entries)
    }

    #[test]
    fn ordered_by_offset_with_markers() {
        let r = compute_timeline(&cap(), &Filter::parse(&[]).unwrap(), 100);
        // offsets: e0=0, e2=20, e1=50 -> chronological order
        assert_eq!(r.rows[0].id, "e000000");
        assert_eq!(r.rows[1].id, "e000002");
        assert_eq!(r.rows[2].id, "e000001");
        // e0 and e1 are duplicates; e1 follows a 500 -> RETRY
        assert_eq!(r.rows[2].marker.as_deref(), Some("RETRY"));
        assert_eq!(r.rows[0].marker.as_deref(), Some("DUP"));
        assert!(r.rows[1].marker.is_none());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib timeline 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_timeline`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/timeline.rs`:

```rust
use crate::fingerprint::fingerprint;
use crate::filter::Filter;
use crate::grouping::retry_entry_ids;
use crate::model::Capture;
use crate::render::{human_bytes, human_ms};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TimelineResult {
    pub rows: Vec<TimelineRow>,
}

#[derive(Debug, Serialize)]
pub struct TimelineRow {
    pub id: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub bytes: i64,
    pub correlation_id: Option<String>,
    pub marker: Option<String>,
}

/// Chronological per-request timeline. `top` bounds the number of rows (earliest first).
pub fn compute_timeline(cap: &Capture, filter: &Filter, top: usize) -> TimelineResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let retries = retry_entry_ids(&entries);
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    for e in &entries {
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }

    let mut rows: Vec<TimelineRow> = entries
        .iter()
        .map(|e| {
            let is_dup = fp_counts.get(&fingerprint(e)).copied().unwrap_or(0) > 1;
            let marker = if retries.contains(&e.id) {
                Some("RETRY".to_string())
            } else if is_dup {
                Some("DUP".to_string())
            } else {
                None
            };
            TimelineRow {
                id: e.id.clone(),
                offset_ms: e.started_offset_ms,
                duration_ms: e.duration_ms,
                method: e.method.to_ascii_uppercase(),
                host: e.host.clone(),
                norm_path: e.norm_path.clone(),
                status: e.status,
                bytes: e.sizes.resp_content.max(e.sizes.resp_body).max(0),
                correlation_id: e.correlation.first().map(|(_, v)| v.clone()),
                marker,
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        a.offset_ms
            .partial_cmp(&b.offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    rows.truncate(top);
    TimelineResult { rows }
}

/// Render the timeline as deterministic terminal text.
pub fn render_timeline_text(r: &TimelineResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail timeline ==\n");
    for row in &r.rows {
        let marker = row.marker.as_deref().map(|m| format!(" {m}")).unwrap_or_default();
        out.push_str(&format!(
            "{:>8}  {:>7}  {} {} {}{}  [{}] {}{}\n",
            human_ms(row.offset_ms),
            human_ms(row.duration_ms),
            row.id,
            row.method,
            row.host,
            row.norm_path,
            row.status,
            human_bytes(row.bytes),
            marker,
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib timeline 2>&1 | tail -8`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/timeline.rs
git commit -m "feat: timeline command analysis + renderer"
```

---

### Task 12: `show-entry` inspection

**Files:**
- Create: `src/analysis/show_entry.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/show_entry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{entry_detail, find_entry};
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer secret".into())];
        e.query = vec![("access_token".into(), "leak".into()), ("page".into(), "2".into())];
        e.resp_body = Some(r#"{"token":"abc","ok":true}"#.to_string());
        let e1 = sample_entry(1, "api.x", "GET", "/other", 200);
        sample_capture(vec![e, e1])
    }

    #[test]
    fn finds_by_id_and_index() {
        let c = cap();
        assert_eq!(find_entry(&c, "e000001").unwrap().norm_path, "/other");
        assert_eq!(find_entry(&c, "1").unwrap().norm_path, "/other");
        assert!(find_entry(&c, "e999999").is_none());
    }

    #[test]
    fn redacts_headers_query_and_body_by_default() {
        let c = cap();
        let d = entry_detail(find_entry(&c, "e000000").unwrap(), false);
        // header value redacted
        let auth = d.req_headers.iter().find(|(n, _)| n == "Authorization").unwrap();
        assert_eq!(auth.1, "<redacted>");
        // sensitive query redacted, safe one kept
        let tok = d.query.iter().find(|(n, _)| n == "access_token").unwrap();
        assert_eq!(tok.1, "<redacted>");
        let page = d.query.iter().find(|(n, _)| n == "page").unwrap();
        assert_eq!(page.1, "2");
        // body token redacted
        assert!(!d.resp_body_snippet.as_deref().unwrap().contains("abc"));
    }

    #[test]
    fn unsafe_shows_raw() {
        let c = cap();
        let d = entry_detail(find_entry(&c, "e000000").unwrap(), true);
        let auth = d.req_headers.iter().find(|(n, _)| n == "Authorization").unwrap();
        assert_eq!(auth.1, "Bearer secret");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib show_entry 2>&1 | tail -12`
Expected: FAIL with "cannot find function `find_entry`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/show_entry.rs`:

```rust
use crate::model::{Capture, Entry};
use crate::redact::{redact_body, redact_header_value, redact_query_value};
use crate::timing::PhaseBreakdown;
use serde::Serialize;

const BODY_MAX: usize = 2000;

#[derive(Debug, Serialize)]
pub struct EntryDetail {
    pub id: String,
    pub index: usize,
    pub method: String,
    pub url: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub status_text: String,
    pub http_version: String,
    pub server_ip: Option<String>,
    pub resource_type: String,
    pub content_type: Option<String>,
    pub started_offset_ms: f64,
    pub duration_ms: f64,
    pub query: Vec<(String, String)>,
    pub req_headers: Vec<(String, String)>,
    pub resp_headers: Vec<(String, String)>,
    pub correlation: Vec<(String, String)>,
    pub timings: PhaseBreakdown,
    pub req_body_snippet: Option<String>,
    pub resp_body_snippet: Option<String>,
}

/// Find an entry by its `eNNNNNN` id, or by bare index (`123` or `e123`).
pub fn find_entry<'a>(cap: &'a Capture, id_arg: &str) -> Option<&'a Entry> {
    if let Some(e) = cap.entries.iter().find(|e| e.id == id_arg) {
        return Some(e);
    }
    let digits = id_arg.strip_prefix('e').unwrap_or(id_arg);
    let idx: usize = digits.parse().ok()?;
    cap.entries.iter().find(|e| e.index == idx)
}

/// Build a redacted, serializable detail view of one entry.
pub fn entry_detail(e: &Entry, unsafe_include: bool) -> EntryDetail {
    let query = e
        .query
        .iter()
        .map(|(k, v)| (k.clone(), redact_query_value(k, v, unsafe_include)))
        .collect();
    let req_headers = e
        .req_headers
        .iter()
        .map(|(k, v)| (k.clone(), redact_header_value(k, v, unsafe_include)))
        .collect();
    let resp_headers = e
        .resp_headers
        .iter()
        .map(|(k, v)| (k.clone(), redact_header_value(k, v, unsafe_include)))
        .collect();
    let req_body_snippet = e
        .req_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, BODY_MAX));
    let resp_body_snippet = e
        .resp_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, BODY_MAX));

    EntryDetail {
        id: e.id.clone(),
        index: e.index,
        method: e.method.to_ascii_uppercase(),
        url: e.url.clone(),
        host: e.host.clone(),
        norm_path: e.norm_path.clone(),
        status: e.status,
        status_text: e.status_text.clone(),
        http_version: e.http_version.clone(),
        server_ip: e.server_ip.clone(),
        resource_type: format!("{:?}", e.resource_type).to_ascii_lowercase(),
        content_type: e.content_type.clone(),
        started_offset_ms: e.started_offset_ms,
        duration_ms: e.duration_ms,
        query,
        req_headers,
        resp_headers,
        correlation: e.correlation.clone(),
        timings: PhaseBreakdown::from_phases(&e.timings),
        req_body_snippet,
        resp_body_snippet,
    }
}

/// Render an entry detail as deterministic terminal text.
pub fn render_entry_detail_text(d: &EntryDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("== wiretrail entry {} ==\n", d.id));
    out.push_str(&format!("{} {}  [{}] {}\n", d.method, d.url, d.status, d.status_text));
    out.push_str(&format!(
        "host: {}  http: {}  type: {}\n",
        d.host, d.http_version, d.resource_type
    ));
    if let Some(ip) = &d.server_ip {
        out.push_str(&format!("server ip: {ip}\n"));
    }
    out.push_str(&format!(
        "offset: {}ms  duration: {}ms\n",
        d.started_offset_ms as i64, d.duration_ms as i64
    ));
    if !d.query.is_empty() {
        out.push_str("query:\n");
        for (k, v) in &d.query {
            out.push_str(&format!("  {k} = {v}\n"));
        }
    }
    out.push_str("request headers:\n");
    for (k, v) in &d.req_headers {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push_str("response headers:\n");
    for (k, v) in &d.resp_headers {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    if let Some(b) = &d.req_body_snippet {
        out.push_str(&format!("request body: {b}\n"));
    }
    if let Some(b) = &d.resp_body_snippet {
        out.push_str(&format!("response body: {b}\n"));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib show_entry 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/show_entry.rs
git commit -m "feat: show-entry inspection (redacted detail view)"
```

---

### Task 13: Wire the eight commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::summary::...;` line, add:

```rust
use har::analysis::duplicates::{compute_duplicates, render_duplicates_text};
use har::analysis::errors::{compute_errors, render_errors_text};
use har::analysis::redirects::{compute_redirects, render_redirects_text};
use har::analysis::retries::{compute_retries, render_retries_text};
use har::analysis::show_entry::{entry_detail, find_entry, render_entry_detail_text};
use har::analysis::slowest::{compute_slowest, render_slowest_text};
use har::analysis::timeline::{compute_timeline, render_timeline_text};
use har::analysis::transitions::{compute_transitions, render_transitions_text};
```

- [ ] **Step 2: Add the `--unsafe-include-secrets` global flag.** In the `Cli` struct, directly after the `config` field, add:

```rust
    /// Show raw secret values (auth headers, tokens, bodies) instead of redacting.
    #[arg(long, global = true)]
    unsafe_include_secrets: bool,
```

- [ ] **Step 3: Add the eight subcommand variants.** Inside `enum Command { ... }`, after the `Endpoints,` variant, add:

```rust
    /// Repeated requests (method + path + query fingerprint).
    Duplicates,
    /// Repeated requests that follow a failed attempt.
    Retries,
    /// 4xx/5xx/failed responses grouped by endpoint.
    Errors,
    /// Redirect responses, chains, and storms.
    Redirects,
    /// Top-N slowest requests with timing breakdown.
    Slowest,
    /// Status-code transition sequences (401->200, 429->429, ...).
    Transitions,
    /// Chronological per-request timeline.
    Timeline,
    /// Full redacted detail for one entry (by id `e000123` or index).
    ShowEntry {
        /// Entry id (e000123) or bare index.
        id: String,
    },
```

- [ ] **Step 4: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::Endpoints => { ... }` arm, add:

```rust
        Command::Duplicates => {
            let result = compute_duplicates(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "duplicates",
                &cap.meta,
                &result,
                &render_duplicates_text(&result),
                &["retries", "errors", "show-entry"],
            );
            exit(findings);
        }
        Command::Retries => {
            let result = compute_retries(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "retries",
                &cap.meta,
                &result,
                &render_retries_text(&result),
                &["errors", "transitions", "show-entry"],
            );
            exit(findings);
        }
        Command::Errors => {
            let result = compute_errors(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "errors",
                &cap.meta,
                &result,
                &render_errors_text(&result),
                &["transitions", "redirects", "show-entry"],
            );
            exit(findings);
        }
        Command::Redirects => {
            let result = compute_redirects(&cap, &filter, cli.top);
            let findings = result.groups.iter().any(|g| g.is_storm);
            emit(
                cli.json,
                "redirects",
                &cap.meta,
                &result,
                &render_redirects_text(&result),
                &["timeline", "errors", "show-entry"],
            );
            exit(findings);
        }
        Command::Slowest => {
            let result = compute_slowest(&cap, &filter, cli.top);
            emit(
                cli.json,
                "slowest",
                &cap.meta,
                &result,
                &render_slowest_text(&result),
                &["timeline", "hosts", "show-entry"],
            );
            exit(false);
        }
        Command::Transitions => {
            let result = compute_transitions(&cap, &filter, cli.top);
            let findings = !result.transitions.is_empty();
            emit(
                cli.json,
                "transitions",
                &cap.meta,
                &result,
                &render_transitions_text(&result),
                &["errors", "retries", "show-entry"],
            );
            exit(findings);
        }
        Command::Timeline => {
            let result = compute_timeline(&cap, &filter, cli.top);
            emit(
                cli.json,
                "timeline",
                &cap.meta,
                &result,
                &render_timeline_text(&result),
                &["slowest", "duplicates", "show-entry"],
            );
            exit(false);
        }
        Command::ShowEntry { id } => {
            let Some(e) = find_entry(&cap, &id) else {
                eprintln!("wiretrail: no entry with id or index '{id}'");
                std::process::exit(ExitCode::InvalidHar as i32);
            };
            let detail = entry_detail(e, cli.unsafe_include_secrets);
            emit(
                cli.json,
                "show-entry",
                &cap.meta,
                &detail,
                &render_entry_detail_text(&detail),
                &["timeline", "duplicates", "errors"],
            );
            exit(false);
        }
```

- [ ] **Step 5: Build**

Run: `cargo build 2>&1 | tail -8`
Expected: SUCCESS.

- [ ] **Step 6: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har duplicates
cargo run --quiet -- tests/fixtures/someapi123.har errors
cargo run --quiet -- tests/fixtures/someapi123.har slowest --json | head -8
cargo run --quiet -- tests/fixtures/someapi123.har timeline
cargo run --quiet -- tests/fixtures/someapi123.har show-entry e000000
```
Expected: `duplicates`/`errors` print their headers with no groups (the single-entry fixture has neither); `slowest --json` prints an envelope with `"command": "slowest"`; `timeline` prints one row; `show-entry e000000` prints the entry detail block with redacted headers.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire failures/timing commands + --unsafe-include-secrets into CLI"
```

---

### Task 14: End-to-end binary tests

**Files:**
- Create: `tests/cli_failures.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_failures.rs`:

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
fn duplicates_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "duplicates", "--json"]);
    assert!(stdout.contains("\"command\": \"duplicates\""));
    assert!(stdout.contains("\"groups\""));
}

#[test]
fn errors_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "errors"]);
    assert!(stdout.contains("== wiretrail errors =="));
}

#[test]
fn slowest_json_has_bottleneck_field() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "slowest", "--json"]);
    assert!(stdout.contains("\"command\": \"slowest\""));
    assert!(stdout.contains("\"bottleneck\""));
}

#[test]
fn show_entry_prints_detail() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "show-entry", "e000000"]);
    assert!(stdout.contains("== wiretrail entry e000000 =="));
}

#[test]
fn show_entry_unknown_id_exits_2() {
    let (_stdout, code) = run(&[&fixture("someapi123.har"), "show-entry", "e999999"]);
    assert_eq!(code, 2);
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_failures 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_failures.rs
git commit -m "test: end-to-end tests for failures/timing commands"
```

---

## Self-review

**Spec coverage (failures & timing slice):**
- `duplicates` — fingerprint groups (count >= 2), statuses, entry ids, retry-pattern flag → Task 5. ✓
- `retries` — repeats following a failed attempt; trigger statuses, backoff gaps, final status → Task 6 (+ Task 2 classification). ✓
- `errors` — 4xx/5xx/failed grouped by endpoint+status; parsed message/code, redacted body snippet, correlation ids, first/last → Task 7 (+ Tasks 3, 4). ✓
- `redirects` — redirect responses grouped; storm flag (>=5), cross-host flag, target host → Task 8. ✓
- `slowest` — top-N by duration with phase breakdown + bottleneck classifier → Task 9 (+ Task 1). ✓
- `transitions` — endpoint-local 401->200 / 429->429 / 5xx->2xx sequences → Task 10. (Cross-endpoint token-refresh correlation is deferred to phase-2 auth-flow analysis, per spec.) ✓
- `timeline` — chronological rows with offset/duration/status/bytes/correlation + DUP/RETRY markers → Task 11. ✓
- `show-entry <id>` — redacted full detail; id-or-index lookup; `--unsafe-include-secrets` override → Task 12 (+ Task 13 flag). ✓
- Redaction safe-by-default for bodies/headers/query in `errors` and `show-entry`; `--unsafe-include-secrets` global → Tasks 4, 12, 13. ✓
- All commands: `--json` envelope, filter language, `--top`, next_commands, findings-based exit codes → Task 13. ✓
- Deferred to Plan 4: `report`, `curl`. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code; every command step states expected output. ✓

**Type consistency:**
- `group_by_fingerprint(&[&Entry]) -> Vec<(String, Vec<&Entry>)>`, `group_has_retry(&[&Entry]) -> bool`, `is_retry_trigger(&Entry) -> bool`, `retry_entry_ids(&[&Entry]) -> AHashSet<String>` (Task 2) used by duplicates (5), retries (6), timeline (11). ✓
- `classify_bottleneck(&Phases) -> &'static str`, `PhaseBreakdown::from_phases(&Phases)` (Task 1) used by slowest (9), show_entry (12). ✓
- `parse_error_fields(&str) -> ErrorFields { message, code }` (Task 3) used by errors (7). ✓
- `redact_body(&str, bool, usize) -> String` (Task 4) used by errors (7) and show_entry (12). `redact_header_value`/`redact_query_value` (Plan 1) used by show_entry (12). ✓
- Each `compute_*` signature matches its Task 13 call site; `compute_errors` and `entry_detail` take the extra `unsafe_include: bool` threaded from `cli.unsafe_include_secrets`. ✓
- `find_entry(&Capture, &str) -> Option<&Entry>` + `entry_detail(&Entry, bool) -> EntryDetail` + `render_entry_detail_text(&EntryDetail)` (Task 12) used in Task 13 `ShowEntry`. ✓
- `emit::<T: Serialize>(...)` / `exit(bool)` (Plan 2) reused unchanged; all result structs derive `Serialize`. ✓
- `Entry` fields referenced (`status`, `status_class()`, `is_error()`, `redirect_url`, `resp_body`, `req_body`, `correlation`, `timings`, `sizes.resp_content`, `sizes.resp_body`, `started_offset_ms`, `index`, `id`) all exist from Plan 1. ✓
