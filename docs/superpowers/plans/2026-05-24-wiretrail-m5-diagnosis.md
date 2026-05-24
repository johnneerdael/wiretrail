# wiretrail M5 — Diagnosis & Startup (`diagnose`, `startup`, `cascade`, `validate`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add four commands that diagnose a capture: `diagnose` (ranked root-cause synthesis), `startup` (boot/concurrency profile), `cascade` (first-failure + downstream cascades), and `validate` (capture quality & sufficiency).

**Architecture:** Four focused analysis modules in the established `compute_* → result + render_*_text` pattern. `diagnose` is a **composition layer** that calls the existing `compute_*` functions and scores their output into ranked findings; the other three are direct passes over the normalized `Capture`. No new dependencies.

**Tech Stack:** Rust 2024, serde, ahash, clap — no new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-m5-m7-expansion-design.md`,
Phase M5. **Plan 1 of 3** for expansion-2 (M6 extraction, M7 regression/rules follow).

**Deviation note:** the spec listed `compute_diagnose(cap, filter, config, top)`, but
none of the analyses it composes take `config`, so the implemented signature is
`compute_diagnose(cap, filter, top)` (no `config` param).

## Prerequisites (verified present)

- `model::{Capture, Entry}`, `Entry.{id,index,started_offset_ms,duration_ms,method,host,norm_path,status,resp_body,req_body,req_headers,sizes,timings}`, `Entry::is_error()`, `CaptureMeta.{har_version,creator,entry_count}`, cfg(test) `sample_entry`/`sample_capture`.
- `filter::Filter`, `render::{human_ms, human_bytes}`, `emit`/`exit` in `main.rs`.
- Existing `compute_*` (verified signatures/fields):
  - `errors::compute_errors(&Capture,&Filter,usize,bool) -> ErrorsResult{groups:[ErrorGroup{host,method,norm_path,status:i64,count,error_message:Option<String>,entry_ids}]}`
  - `retries::compute_retries(&Capture,&Filter,usize) -> RetriesResult{groups:[RetryGroup{method,host,norm_path,retry_count,final_status:i64,entry_ids}]}`
  - `duplicates::compute_duplicates(&Capture,&Filter,usize) -> DuplicatesResult{groups:[DuplicateGroup{method,host,norm_path,count,is_retry_pattern,entry_ids}]}`
  - `storms::compute_storms(&Capture,&Filter,u64,usize,usize) -> StormsResult{storms:[Storm{scope_kind,scope,peak_count,entry_ids}]}`
  - `rate_limit::compute_rate_limit(&Capture,&Filter,usize) -> RateLimitResult{groups:[RateLimitGroup{host,norm_path,count_429,cooldown_violated,entry_ids}]}`
  - `auth::compute_auth(&Capture,&Filter,usize) -> AuthResult{failures:[AuthFailure{host,norm_path,status,count,entry_ids}], refreshes:[RefreshEvent{id,host,status,success,old_token_reused,reusing_ids}]}`
  - `redirects::compute_redirects(&Capture,&Filter,usize) -> RedirectsResult{groups:[RedirectGroup{host,method,norm_path,status,count,is_storm,entry_ids}]}`
  - `slowest::compute_slowest(&Capture,&Filter,usize) -> SlowestResult{entries:[SlowRow{id,method,host,norm_path,status,duration_ms,bottleneck:String}]}`

## File structure

```
src/analysis/mod.rs        # Modify: declare cascade, diagnose, startup, validate
src/analysis/validate.rs   # NEW
src/analysis/startup.rs    # NEW
src/analysis/cascade.rs    # NEW
src/analysis/diagnose.rs   # NEW (composition layer)
src/main.rs                # Modify: 4 subcommands + dispatch
tests/cli_diagnose.rs      # NEW: integration tests
```

---

### Task 1: Scaffold modules

**Files:**
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Declare the four new modules.** Add these lines to `src/analysis/mod.rs` in alphabetical position (the file is a flat list of `pub mod` lines): `pub mod cascade;`, `pub mod diagnose;`, `pub mod startup;`, `pub mod validate;`. After editing, the file's first lines should read:

```rust
pub mod auth;
pub mod cascade;
pub mod checks;
pub mod curl;
pub mod diagnose;
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
pub mod startup;
pub mod storms;
pub mod subsystems;
pub mod summary;
pub mod timeline;
pub mod transitions;
pub mod validate;
```

- [ ] **Step 2: Create empty files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/analysis/cascade.rs src/analysis/diagnose.rs src/analysis/startup.rs src/analysis/validate.rs
cargo build 2>&1 | tail -4
```
Expected: build SUCCEEDS.

- [ ] **Step 3: Commit**

```bash
git add src/analysis/mod.rs src/analysis/cascade.rs src/analysis/diagnose.rs src/analysis/startup.rs src/analysis/validate.rs
git commit -m "chore: scaffold M5 diagnosis modules"
```

---

### Task 2: `validate` command

**Files:**
- Create: `src/analysis/validate.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/validate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_validate;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn flags_sanitized_capture_with_no_bodies_or_auth() {
        // sample_entry has no auth header, no resp_body, no cookies
        let cap = sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.x", "GET", "/b", 200),
        ]);
        let r = compute_validate(&cap);
        assert!(r.sanitized);
        assert!(!r.with_auth);
        assert_eq!(r.pct_with_resp_body, 0.0);
        assert!(r.sufficiency_notes.iter().any(|n| n.contains("response bodies")));
    }

    #[test]
    fn detects_status_zero_anomaly() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 0)]);
        let r = compute_validate(&cap);
        assert!(r.anomalies.iter().any(|a| a.kind == "status-0" && a.count == 1));
    }

    #[test]
    fn reports_body_and_auth_presence() {
        let mut e = sample_entry(0, "api.x", "POST", "/a", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        e.req_body = Some(r#"{"k":1}"#.into());
        e.resp_body = Some(r#"{"ok":true}"#.into());
        let r = compute_validate(&sample_capture(vec![e]));
        assert!(r.with_auth);
        assert_eq!(r.pct_with_resp_body, 1.0);
        assert_eq!(r.pct_post_with_req_body, 1.0);
        assert!(!r.sanitized);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::validate 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_validate`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/validate.rs`:

```rust
use crate::model::{Capture, Entry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ValidateResult {
    pub har_version: String,
    pub creator: String,
    pub entry_count: usize,
    pub pct_with_timings: f64,
    pub pct_with_resp_body: f64,
    pub pct_post_with_req_body: f64,
    pub with_auth: bool,
    pub with_cookies: bool,
    pub anomalies: Vec<Anomaly>,
    pub sanitized: bool,
    pub sufficiency_notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Anomaly {
    pub kind: String,
    pub count: usize,
}

fn has_header(e: &Entry, name: &str) -> bool {
    e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
}

fn has_body(b: &Option<String>) -> bool {
    b.as_deref().is_some_and(|s| !s.is_empty())
}

/// Assess HAR quality and analysis sufficiency.
pub fn compute_validate(cap: &Capture) -> ValidateResult {
    let n = cap.entries.len();
    let denom = n.max(1) as f64;

    let with_timings = cap
        .entries
        .iter()
        .filter(|e| e.timings.wait > 0.0 || e.timings.receive > 0.0 || e.timings.send > 0.0)
        .count();
    let with_resp_body = cap.entries.iter().filter(|e| has_body(&e.resp_body)).count();

    let posts: Vec<&Entry> = cap
        .entries
        .iter()
        .filter(|e| matches!(e.method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH"))
        .collect();
    let posts_with_body = posts.iter().filter(|e| has_body(&e.req_body)).count();

    let with_auth = cap.entries.iter().any(|e| has_header(e, "authorization"));
    let with_cookies = cap.entries.iter().any(|e| has_header(e, "cookie"));

    let mut count = |pred: &dyn Fn(&Entry) -> bool, kind: &str| -> Option<Anomaly> {
        let c = cap.entries.iter().filter(|e| pred(e)).count();
        if c > 0 {
            Some(Anomaly { kind: kind.to_string(), count: c })
        } else {
            None
        }
    };
    let mut anomalies = Vec::new();
    if let Some(a) = count(&|e| e.status == 0, "status-0") {
        anomalies.push(a);
    }
    if let Some(a) = count(&|e| e.method.is_empty(), "missing-method") {
        anomalies.push(a);
    }
    if let Some(a) = count(
        &|e| e.duration_ms == 0.0 && has_body(&e.resp_body),
        "zero-duration-with-body",
    ) {
        anomalies.push(a);
    }
    if let Some(a) = count(
        &|e| e.sizes.resp_body < -1 || e.sizes.req_body < -1 || e.sizes.resp_content < -1,
        "negative-size",
    ) {
        anomalies.push(a);
    }

    let pct_with_resp_body = with_resp_body as f64 / denom;
    let sanitized = !with_auth && !with_cookies && pct_with_resp_body < 0.10;

    let mut notes = Vec::new();
    if pct_with_resp_body < 0.10 {
        notes.push("few/no response bodies captured — `errors`/`search`/`extract` limited".to_string());
    }
    if !with_auth {
        notes.push("no Authorization headers — `auth`/`jwt` limited".to_string());
    }
    if !posts.is_empty() && posts_with_body == 0 {
        notes.push("no request bodies on POST/PUT/PATCH — `diff` body verdicts limited".to_string());
    }
    if with_timings == 0 {
        notes.push("no timing data — `slowest`/`startup` limited".to_string());
    }

    ValidateResult {
        har_version: cap.meta.har_version.clone(),
        creator: cap.meta.creator.clone(),
        entry_count: n,
        pct_with_timings: with_timings as f64 / denom,
        pct_with_resp_body,
        pct_post_with_req_body: if posts.is_empty() { 0.0 } else { posts_with_body as f64 / posts.len() as f64 },
        with_auth,
        with_cookies,
        anomalies,
        sanitized,
        sufficiency_notes: notes,
    }
}

fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// Render the validation report as deterministic terminal text.
pub fn render_validate_text(r: &ValidateResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail validate ==\n");
    out.push_str(&format!("HAR {} via {}  ({} entries)\n", r.har_version, r.creator, r.entry_count));
    out.push_str(&format!(
        "with timings: {} · response bodies: {} · POST req bodies: {}\n",
        pct(r.pct_with_timings),
        pct(r.pct_with_resp_body),
        pct(r.pct_post_with_req_body)
    ));
    out.push_str(&format!("auth headers: {} · cookies: {}\n", r.with_auth, r.with_cookies));
    if r.sanitized {
        out.push_str("sanitized: yes (no auth/cookies and few response bodies)\n");
    }
    if !r.anomalies.is_empty() {
        out.push_str("\nanomalies:\n");
        for a in &r.anomalies {
            out.push_str(&format!("  {}: {}\n", a.kind, a.count));
        }
    }
    if !r.sufficiency_notes.is_empty() {
        out.push_str("\nsufficiency:\n");
        for n in &r.sufficiency_notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::validate 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/validate.rs
git commit -m "feat: validate command (capture quality + sufficiency)"
```

---

### Task 3: `startup` command

**Files:**
- Create: `src/analysis/startup.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/startup.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_startup;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn at(index: usize, path: &str, offset: f64, dur: f64) -> Entry {
        let mut e = sample_entry(index, "h", "GET", path, 200);
        e.started_offset_ms = offset;
        e.duration_ms = dur;
        e
    }

    #[test]
    fn measures_max_concurrency() {
        // three calls overlapping at t=100..150
        let cap = sample_capture(vec![
            at(0, "/a", 0.0, 200.0),
            at(1, "/b", 100.0, 100.0),
            at(2, "/c", 120.0, 50.0),
        ]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 0, 10);
        assert_eq!(r.max_concurrency, 3);
        assert_eq!(r.requests_in_window, 3);
    }

    #[test]
    fn builds_sequential_critical_path() {
        // three strictly sequential calls -> chain length 3, sum 300ms
        let cap = sample_capture(vec![
            at(0, "/a", 0.0, 100.0),
            at(1, "/b", 100.0, 100.0),
            at(2, "/c", 200.0, 100.0),
        ]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 0, 10);
        assert_eq!(r.critical_path.len(), 3);
        assert_eq!(r.critical_path_ms, 300.0);
        assert_eq!(r.max_concurrency, 1);
    }

    #[test]
    fn window_bounds_entries() {
        let cap = sample_capture(vec![at(0, "/a", 0.0, 10.0), at(1, "/late", 60000.0, 10.0)]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 30000, 10);
        assert_eq!(r.requests_in_window, 1); // only /a within 30s
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::startup 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_startup`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/startup.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StartupResult {
    pub window_ms: f64,
    pub requests_in_window: usize,
    pub max_concurrency: usize,
    pub critical_path_ms: f64,
    pub critical_path: Vec<StartupCall>,
    pub slowest: Vec<StartupCall>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupCall {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub status: i64,
}

fn call_of(e: &Entry) -> StartupCall {
    StartupCall {
        id: e.id.clone(),
        method: e.method.to_ascii_uppercase(),
        host: e.host.clone(),
        norm_path: e.norm_path.clone(),
        offset_ms: e.started_offset_ms,
        duration_ms: e.duration_ms,
        status: e.status,
    }
}

/// Profile the boot window: concurrency, sequential critical path, slow deps.
/// `window_ms == 0` means "the whole capture".
pub fn compute_startup(cap: &Capture, filter: &Filter, window_ms: u64, top: usize) -> StartupResult {
    let mut entries: Vec<&Entry> = cap
        .entries
        .iter()
        .filter(|e| filter.matches(e) && (window_ms == 0 || e.started_offset_ms <= window_ms as f64))
        .collect();
    entries.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    // max concurrency via a sweep line over start/end events.
    let mut events: Vec<(f64, i32)> = Vec::with_capacity(entries.len() * 2);
    for e in &entries {
        events.push((e.started_offset_ms, 1));
        events.push((e.started_offset_ms + e.duration_ms.max(0.0), -1));
    }
    // ends before starts at the same instant, so a touch-point isn't double-counted.
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
    let mut cur = 0i32;
    let mut max_concurrency = 0i32;
    for (_, d) in &events {
        cur += d;
        max_concurrency = max_concurrency.max(cur);
    }

    // greedy sequential chain: each next call starts at/after the current one ends.
    let mut chain: Vec<StartupCall> = Vec::new();
    let mut chain_ms = 0.0;
    let mut end = f64::MIN;
    for e in &entries {
        if e.started_offset_ms >= end {
            chain.push(call_of(e));
            chain_ms += e.duration_ms.max(0.0);
            end = e.started_offset_ms + e.duration_ms.max(0.0);
        }
    }
    let critical_path: Vec<StartupCall> = chain.iter().take(top).cloned().collect();

    let mut slow: Vec<&Entry> = entries.clone();
    slow.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let slowest: Vec<StartupCall> = slow.iter().take(top).map(|e| call_of(e)).collect();

    let window_ms_out = if window_ms == 0 {
        entries.last().map(|e| e.started_offset_ms).unwrap_or(0.0)
    } else {
        window_ms as f64
    };

    StartupResult {
        window_ms: window_ms_out,
        requests_in_window: entries.len(),
        max_concurrency: max_concurrency.max(0) as usize,
        critical_path_ms: chain_ms,
        critical_path,
        slowest,
    }
}

/// Render the startup profile as deterministic terminal text.
pub fn render_startup_text(r: &StartupResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail startup ==\n");
    out.push_str(&format!(
        "{} requests in {} · max concurrency {} · critical path {}\n",
        r.requests_in_window,
        human_ms(r.window_ms),
        r.max_concurrency,
        human_ms(r.critical_path_ms)
    ));
    out.push_str("\ncritical path (sequential spine):\n");
    for c in &r.critical_path {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}  [{}]\n",
            human_ms(c.duration_ms),
            c.id,
            c.method,
            c.host,
            c.norm_path,
            c.status
        ));
    }
    out.push_str("\nslowest in window:\n");
    for c in &r.slowest {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}\n",
            human_ms(c.duration_ms),
            c.id,
            c.method,
            c.host,
            c.norm_path
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::startup 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/startup.rs
git commit -m "feat: startup command (boot profile: concurrency + critical path)"
```

---

### Task 4: `cascade` command

**Files:**
- Create: `src/analysis/cascade.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/cascade.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_cascade;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn at(index: usize, path: &str, status: i64, offset: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", path, status);
        e.started_offset_ms = offset;
        e
    }

    #[test]
    fn finds_first_failure_with_neighbors() {
        let cap = sample_capture(vec![
            at(0, "/ok1", 200, 0.0),
            at(1, "/boom", 500, 10.0),
            at(2, "/ok2", 200, 20.0),
        ]);
        let r = compute_cascade(&cap, &Filter::parse(&[]).unwrap(), 5000, 3, 10);
        let f = r.first_failure.unwrap();
        assert_eq!(f.id, "e000001");
        assert!(f.before_ids.contains(&"e000000".to_string()));
        assert!(f.after_ids.contains(&"e000002".to_string()));
    }

    #[test]
    fn detects_cascade_from_config_failure() {
        let mut es = vec![at(0, "/config", 500, 0.0)];
        for i in 1..=4 {
            es.push(at(i, "/data", 500, i as f64 * 100.0)); // 4 downstream failures within 5s
        }
        let r = compute_cascade(&sample_capture(es), &Filter::parse(&[]).unwrap(), 5000, 3, 10);
        let c = r.cascades.iter().find(|c| c.trigger_id == "e000000").unwrap();
        assert_eq!(c.trigger_kind, "config");
        assert!(c.downstream_failures >= 3);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::cascade 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_cascade`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/cascade.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CascadeResult {
    pub first_failure: Option<FailureContext>,
    pub cascades: Vec<Cascade>,
}

#[derive(Debug, Serialize)]
pub struct FailureContext {
    pub id: String,
    pub status: i64,
    pub host: String,
    pub norm_path: String,
    pub before_ids: Vec<String>,
    pub after_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Cascade {
    pub trigger_id: String,
    pub trigger_kind: String,
    pub downstream_failures: usize,
    pub downstream_ids: Vec<String>,
}

fn trigger_kind(np: &str) -> &'static str {
    let p = np.to_ascii_lowercase();
    if p.contains("/config") {
        "config"
    } else if p.contains("/auth") || p.contains("/token") || p.contains("/oauth") {
        "auth"
    } else if p.contains("bootstrap") || p.contains("/init") {
        "bootstrap"
    } else {
        "request"
    }
}

/// Find the earliest failure and downstream failure cascades.
pub fn compute_cascade(
    cap: &Capture,
    filter: &Filter,
    window_ms: u64,
    min_downstream: usize,
    top: usize,
) -> CascadeResult {
    let mut entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();
    entries.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    // first failure + 3 neighbors each side (in time order).
    let first_failure = entries.iter().position(|e| e.is_error()).map(|pos| {
        let e = entries[pos];
        let before_ids = entries[pos.saturating_sub(3)..pos]
            .iter()
            .map(|x| x.id.clone())
            .collect();
        let after_ids = entries[pos + 1..(pos + 4).min(entries.len())]
            .iter()
            .map(|x| x.id.clone())
            .collect();
        FailureContext {
            id: e.id.clone(),
            status: e.status,
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            before_ids,
            after_ids,
        }
    });

    // cascades: a failure followed by >= min_downstream failures within window_ms.
    let w = window_ms as f64;
    let mut cascades: Vec<Cascade> = Vec::new();
    for (i, trigger) in entries.iter().enumerate() {
        if !trigger.is_error() {
            continue;
        }
        let t = trigger.started_offset_ms;
        let downstream: Vec<String> = entries[i + 1..]
            .iter()
            .filter(|e| e.is_error() && e.started_offset_ms > t && e.started_offset_ms <= t + w)
            .map(|e| e.id.clone())
            .collect();
        if downstream.len() >= min_downstream {
            cascades.push(Cascade {
                trigger_id: trigger.id.clone(),
                trigger_kind: trigger_kind(&trigger.norm_path).to_string(),
                downstream_failures: downstream.len(),
                downstream_ids: downstream.into_iter().take(top).collect(),
            });
        }
    }
    cascades.sort_by(|a, b| b.downstream_failures.cmp(&a.downstream_failures).then(a.trigger_id.cmp(&b.trigger_id)));
    cascades.truncate(top);

    CascadeResult { first_failure, cascades }
}

/// Render cascades as deterministic terminal text.
pub fn render_cascade_text(r: &CascadeResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail cascade ==\n");
    if let Some(f) = &r.first_failure {
        out.push_str(&format!(
            "\nfirst failure: {} [{}] {}{}\n",
            f.id, f.status, f.host, f.norm_path
        ));
        out.push_str(&format!("  before: {}\n", f.before_ids.join(", ")));
        out.push_str(&format!("  after:  {}\n", f.after_ids.join(", ")));
    } else {
        out.push_str("\nno failures in capture\n");
    }
    if !r.cascades.is_empty() {
        out.push_str("\ncascades:\n");
        for c in &r.cascades {
            out.push_str(&format!(
                "  {} [{}] -> {} downstream failures\n",
                c.trigger_id, c.trigger_kind, c.downstream_failures
            ));
            out.push_str(&format!("    {}\n", c.downstream_ids.join(", ")));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::cascade 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/cascade.rs
git commit -m "feat: cascade command (first-failure + downstream cascades)"
```

---

### Task 5: `diagnose` command (root-cause synthesizer)

**Files:**
- Create: `src/analysis/diagnose.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/diagnose.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_diagnose;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn err(index: usize, path: &str, status: i64, off: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", path, status);
        e.started_offset_ms = off;
        e
    }

    #[test]
    fn surfaces_5xx_cluster_as_high() {
        // 3 identical 500s -> 5xx-cluster, high
        let cap = sample_capture(vec![
            err(0, "/bulk", 500, 0.0),
            err(1, "/bulk", 500, 10.0),
            err(2, "/bulk", 500, 20.0),
        ]);
        let r = compute_diagnose(&cap, &Filter::parse(&[]).unwrap(), 20);
        assert!(r.findings.iter().any(|f| f.kind == "5xx-cluster" && f.severity == "high"));
        // highest-severity finding sorts first
        assert_eq!(r.findings[0].severity, "high");
    }

    #[test]
    fn clean_capture_has_no_findings() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/ok", 200)]);
        let r = compute_diagnose(&cap, &Filter::parse(&[]).unwrap(), 20);
        assert!(r.findings.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::diagnose 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_diagnose`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/diagnose.rs`:

```rust
use crate::analysis::{auth, duplicates, errors, rate_limit, redirects, retries, slowest, storms};
use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DiagnoseResult {
    pub findings: Vec<Diagnosis>,
}

#[derive(Debug, Serialize)]
pub struct Diagnosis {
    pub severity: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub suggested_command: String,
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

/// Synthesize ranked root-cause findings by composing the existing analyses.
pub fn compute_diagnose(cap: &Capture, filter: &Filter, top: usize) -> DiagnoseResult {
    let mut f: Vec<Diagnosis> = Vec::new();

    // 5xx clusters / 4xx groups
    for g in errors::compute_errors(cap, filter, top, false).groups {
        let class = if (500..600).contains(&g.status) {
            5
        } else if (400..500).contains(&g.status) {
            4
        } else {
            0
        };
        if class == 5 && g.count >= 3 {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "5xx-cluster".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g.error_message.clone().unwrap_or_else(|| "server error cluster".into()),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: format!("errors --filter \"host:{}\"", g.host),
            });
        } else if class == 4 {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "4xx".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g.error_message.clone().unwrap_or_else(|| "client error".into()),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "errors".into(),
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
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "token-refresh-race".into(),
                title: format!("suspicious token refresh on {}", rf.host),
                detail: why.into(),
                evidence_ids: {
                    let mut v = vec![rf.id.clone()];
                    v.extend(rf.reusing_ids.clone());
                    v
                },
                suggested_command: "auth".into(),
            });
        }
    }
    if !a.failures.is_empty() {
        let total: usize = a.failures.iter().map(|x| x.count).sum();
        let ids: Vec<String> = a.failures.iter().flat_map(|x| x.entry_ids.clone()).collect();
        f.push(Diagnosis {
            severity: "medium".into(),
            kind: "auth-failures".into(),
            title: format!("{total} auth failures (401/403)"),
            detail: "requests rejected for authentication/authorization".into(),
            evidence_ids: ids,
            suggested_command: "auth".into(),
        });
    }

    // rate-limit without backoff
    for g in rate_limit::compute_rate_limit(cap, filter, top).groups {
        if g.cooldown_violated {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "rate-limit-no-backoff".into(),
                title: format!("calls during 429 cooldown on {} {}", g.host, g.norm_path),
                detail: format!("{} 429s, follow-ups before Retry-After elapsed", g.count_429),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "rate-limit".into(),
            });
        }
    }

    // retry exhaustion
    for g in retries::compute_retries(cap, filter, top).groups {
        if g.retry_count >= 3 && !(200..300).contains(&g.final_status) {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "retry-exhaustion".into(),
                title: format!("{} retries, final {} on {} {}", g.retry_count, g.final_status, g.method, g.norm_path),
                detail: "repeated retries did not recover".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "retries".into(),
            });
        }
    }

    // request storms
    for s in storms::compute_storms(cap, filter, 1000, 5, top).storms {
        if s.peak_count >= 10 {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "request-storm".into(),
                title: format!("{} {} calls/s burst to {}", s.peak_count, s.scope_kind, s.scope),
                detail: "burst of calls in a 1s window".into(),
                evidence_ids: s.entry_ids.clone(),
                suggested_command: "storms".into(),
            });
        }
    }

    // wasteful duplicates (not retries)
    for g in duplicates::compute_duplicates(cap, filter, top).groups {
        if g.count >= 10 && !g.is_retry_pattern {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "wasteful-duplicates".into(),
                title: format!("{}x identical {} {}", g.count, g.method, g.norm_path),
                detail: "repeated identical calls (not retries)".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "diff".into(),
            });
        }
    }

    // redirect storms
    for g in redirects::compute_redirects(cap, filter, top).groups {
        if g.is_storm {
            f.push(Diagnosis {
                severity: "low".into(),
                kind: "redirect-storm".into(),
                title: format!("{}x [{}] redirect on {} {}", g.count, g.status, g.host, g.norm_path),
                detail: "repeated redirects".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "redirects".into(),
            });
        }
    }

    // slow backend
    if let Some(s) = slowest::compute_slowest(cap, filter, top).entries.first() {
        if s.duration_ms > 1000.0 && s.bottleneck == "server wait/TTFB" {
            f.push(Diagnosis {
                severity: "low".into(),
                kind: "slow-backend".into(),
                title: format!("slowest call {}ms on {} {}", s.duration_ms as i64, s.host, s.norm_path),
                detail: "dominated by server wait (TTFB)".into(),
                evidence_ids: vec![s.id.clone()],
                suggested_command: "slowest".into(),
            });
        }
    }

    f.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.evidence_ids.len().cmp(&a.evidence_ids.len()))
            .then(a.kind.cmp(&b.kind))
    });
    f.truncate(top);
    DiagnoseResult { findings: f }
}

/// Render the diagnosis as deterministic terminal text.
pub fn render_diagnose_text(r: &DiagnoseResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail diagnose ==\n");
    for d in &r.findings {
        out.push_str(&format!("\n[{}] {} — {}\n", d.severity.to_ascii_uppercase(), d.kind, d.title));
        out.push_str(&format!("  {}\n", d.detail));
        out.push_str(&format!("  evidence: {}\n", d.evidence_ids.iter().take(8).cloned().collect::<Vec<_>>().join(", ")));
        out.push_str(&format!("  -> wiretrail <file> {}\n", d.suggested_command));
    }
    if r.findings.is_empty() {
        out.push_str("\nno notable findings\n");
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::diagnose 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/diagnose.rs
git commit -m "feat: diagnose command (ranked root-cause synthesizer)"
```

---

### Task 6: Wire the four commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::checks::...;` line (the last analysis import added in M3), add:

```rust
use har::analysis::diagnose::{compute_diagnose, render_diagnose_text};
use har::analysis::startup::{compute_startup, render_startup_text};
use har::analysis::cascade::{compute_cascade, render_cascade_text};
use har::analysis::validate::{compute_validate, render_validate_text};
```

- [ ] **Step 2: Add the subcommand variants.** Inside `enum Command { ... }`, after the `Checks,` variant, add:

```rust
    /// Ranked root-cause findings synthesized from all analyses.
    Diagnose,
    /// Boot/startup profile: concurrency, critical path, slow dependencies.
    Startup {
        /// Boot window in milliseconds (0 = whole capture).
        #[arg(long, default_value_t = 30000)]
        window_ms: u64,
    },
    /// Earliest failure and downstream failure cascades.
    Cascade {
        /// Window (ms) to attribute downstream failures to a trigger.
        #[arg(long, default_value_t = 5000)]
        window_ms: u64,
        /// Minimum downstream failures to report a cascade.
        #[arg(long = "min-downstream", default_value_t = 3)]
        min_downstream: usize,
    },
    /// Capture-quality and analysis-sufficiency report.
    Validate,
```

- [ ] **Step 3: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::Checks => { ... }` arm, add:

```rust
        Command::Diagnose => {
            let result = compute_diagnose(&cap, &filter, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "diagnose",
                &cap.meta,
                &result,
                &render_diagnose_text(&result),
                &["errors", "auth", "duplicates"],
            );
            exit(findings);
        }
        Command::Startup { window_ms } => {
            let result = compute_startup(&cap, &filter, window_ms, cli.top);
            emit(
                cli.json,
                "startup",
                &cap.meta,
                &result,
                &render_startup_text(&result),
                &["slowest", "timeline", "storms"],
            );
            exit(false);
        }
        Command::Cascade { window_ms, min_downstream } => {
            let result = compute_cascade(&cap, &filter, window_ms, min_downstream, cli.top);
            let findings = result.first_failure.is_some() || !result.cascades.is_empty();
            emit(
                cli.json,
                "cascade",
                &cap.meta,
                &result,
                &render_cascade_text(&result),
                &["errors", "transitions", "show-entry"],
            );
            exit(findings);
        }
        Command::Validate => {
            let result = compute_validate(&cap);
            let findings = !result.anomalies.is_empty();
            emit(
                cli.json,
                "validate",
                &cap.meta,
                &result,
                &render_validate_text(&result),
                &["summary", "diagnose", "errors"],
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
cargo run --quiet -- tests/fixtures/someapi123.har validate
cargo run --quiet -- tests/fixtures/someapi123.har diagnose --json | head -6
cargo run --quiet -- tests/fixtures/someapi123.har startup
cargo run --quiet -- tests/fixtures/someapi123.har cascade
```
Expected: `validate` prints the quality block (single-entry fixture → sanitized note);
`diagnose --json` prints an envelope with `"command": "diagnose"`; `startup` prints
the profile (1 request, concurrency 1); `cascade` prints "no failures in capture".

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire diagnose/startup/cascade/validate into CLI"
```

---

### Task 7: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_diagnose.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_diagnose.rs`:

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
fn diagnose_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "diagnose", "--json"]);
    assert!(stdout.contains("\"command\": \"diagnose\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn validate_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "validate", "--json"]);
    assert!(stdout.contains("\"command\": \"validate\""));
    assert!(stdout.contains("\"sanitized\""));
}

#[test]
fn startup_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "startup"]);
    assert!(stdout.contains("== wiretrail startup =="));
    assert!(stdout.contains("max concurrency"));
}

#[test]
fn cascade_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "cascade"]);
    assert!(stdout.contains("== wiretrail cascade =="));
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_diagnose 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result: FAILED" || echo "all green"`
Expected: `all green`.

- [ ] **Step 4: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
HAR="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$HAR" diagnose 2>/dev/null | head -24
./target/release/wiretrail "$HAR" validate 2>/dev/null | head -16
```
Expected: `diagnose` surfaces the ntsk.cloud 5xx-cluster and the Supabase
token-refresh/auth findings as `high`, ranked first; `validate` reports body/auth
coverage and any anomalies. No secret values appear.

- [ ] **Step 5: Commit**

```bash
git add tests/cli_diagnose.rs
git commit -m "test: end-to-end tests for diagnose/startup/cascade/validate"
```

---

## Self-review

**Spec coverage (Phase M5):**
- `diagnose` (#62) — composes errors/auth/rate-limit/retries/storms/duplicates/redirects/slowest into ranked severity findings → Task 5. ✓
- `startup` (#85 + #24) — boot window, max concurrency, sequential critical path, slow deps → Task 3. ✓
- `cascade` (#83 + #84) — first failure with neighbors + downstream cascades with trigger-kind → Task 4. ✓
- `validate` (#52/#54/#55) — version/creator, coverage %, anomalies, sanitized flag, sufficiency notes → Task 2. ✓
- All: `--json`, `--top`, next_commands, exit codes; `startup`/`cascade` flags → Task 6. ✓
- No new dependencies. ✓

**Placeholder scan:** No TBD/TODO; every code step complete; every command step states expected output. ✓

**Type consistency:**
- `compute_validate(&Capture) -> ValidateResult` (Task 2) — `Validate` arm passes `&cap` only (no filter), matching. ✓
- `compute_startup(&Capture,&Filter,u64,usize)`, `compute_cascade(&Capture,&Filter,u64,usize,usize)`, `compute_diagnose(&Capture,&Filter,usize)` and their `render_*_text` — Task 6 dispatch passes matching args. ✓
- `diagnose` reads exactly the verified fields: `ErrorGroup.{status,count,method,norm_path,host,error_message,entry_ids}`, `RefreshEvent.{old_token_reused,success,id,host,reusing_ids}`, `AuthFailure.{count,entry_ids}`, `RateLimitGroup.{cooldown_violated,count_429,host,norm_path,entry_ids}`, `RetryGroup.{retry_count,final_status,method,norm_path,entry_ids}`, `Storm.{peak_count,scope,scope_kind,entry_ids}`, `DuplicateGroup.{count,is_retry_pattern,method,norm_path,entry_ids}`, `RedirectGroup.{is_storm,count,status,host,norm_path,entry_ids}`, `SlowRow.{duration_ms,bottleneck,host,norm_path,id}`. ✓
- `compute_storms` called with `(cap, filter, 1000, 5, top)` matching its `(&Capture,&Filter,u64,usize,usize)` signature; `compute_errors` with `(cap, filter, top, false)`. ✓
- Result structs derive `Serialize`; `emit`/`exit` reused unchanged. ✓
- `Entry` fields used (`started_offset_ms`,`duration_ms`,`index`,`is_error()`,`status`,`method`,`host`,`norm_path`,`resp_body`,`req_body`,`req_headers`,`sizes.{req_body,resp_body,resp_content}`,`timings.{wait,receive,send}`,`id`) all exist. ✓
