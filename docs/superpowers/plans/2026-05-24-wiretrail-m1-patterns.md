# wiretrail M1 — Wasteful-Traffic Patterns (`storms`, `pagination`, `rate-limit`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three commands that name the capture's wasteful traffic patterns — request storms, pagination loops + N+1 fan-out, and rate-limit behavior.

**Architecture:** Three focused analysis modules following the established wiretrail pattern (`compute_* → serializable result + render_*_text`), wired through the existing `emit`/`exit`/filter machinery. A shared `densest_window` helper (added to `grouping`) backs both storm and N+1 detection. Subcommands carry their own threshold flags.

**Tech Stack:** Rust 2024, serde, ahash, clap — no new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-analysis-expansion-design.md`,
Phase M1. This is **Plan 1 of 3** for the expansion (M2 auth, M3 diff/checks follow).

**Deviation note:** `rate-limit` parses integer-seconds `Retry-After` only;
HTTP-date `Retry-After` is recorded raw but not used for cooldown math (deferred —
keeps M1 free of absolute-time reconstruction).

## Prerequisites (verified present)

- `model::{Capture, Entry}`, `Entry.started_offset_ms/index/host/norm_path/method/status/query/resp_headers`, cfg(test) `model::sample_entry`/`sample_capture`.
- `filter::Filter`, `render::human_ms`, `emit`/`exit` in `main.rs`.
- `grouping.rs` (will gain `densest_window`).
- `main.rs` `Command` enum ends with `Curl { id: Option<String> }` (variants confirmed at lines 57–86).

## File structure

```
src/grouping.rs            # Modify: add densest_window (shared by storms + N+1)
src/analysis/mod.rs        # Modify: declare storms, pagination, rate_limit
src/analysis/storms.rs     # NEW
src/analysis/pagination.rs # NEW (pagination loop + N+1)
src/analysis/rate_limit.rs # NEW
src/main.rs                # Modify: 3 subcommands + dispatch
tests/cli_patterns.rs      # NEW: integration tests
```

---

### Task 1: Scaffold modules

**Files:**
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Declare the three new modules.** Replace the entire contents of `src/analysis/mod.rs` with this exact list (alphabetical, adding `pagination`, `rate_limit`, `storms`):

```rust
pub mod curl;
pub mod duplicates;
pub mod endpoints;
pub mod errors;
pub mod hosts;
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

- [ ] **Step 2: Create empty module files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/analysis/storms.rs src/analysis/pagination.rs src/analysis/rate_limit.rs
cargo build 2>&1 | tail -4
```
Expected: build SUCCEEDS (empty modules are valid).

- [ ] **Step 3: Commit**

```bash
git add src/analysis/mod.rs src/analysis/storms.rs src/analysis/pagination.rs src/analysis/rate_limit.rs
git commit -m "chore: scaffold M1 pattern modules"
```

---

### Task 2: `densest_window` shared helper

**Files:**
- Modify: `src/grouping.rs`

- [ ] **Step 1: Add a failing test.** In `src/grouping.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add before its closing brace:

```rust
    #[test]
    fn densest_window_finds_burst() {
        // offsets 0,50,100,150,1000 with a 200ms window -> densest is the first 4
        let cap = sample_capture(vec![
            off(0, 0.0),
            off(1, 50.0),
            off(2, 100.0),
            off(3, 150.0),
            off(4, 1000.0),
        ]);
        let refs: Vec<&Entry> = cap.entries.iter().collect();
        let (count, l, r) = super::densest_window(&refs, 200.0);
        assert_eq!(count, 4);
        assert_eq!((l, r), (0, 3));
    }

    fn off(index: usize, offset_ms: f64) -> Entry {
        let mut e = sample_entry(index, "h", "GET", "/x", 200);
        e.started_offset_ms = offset_ms;
        e
    }
```

Add `Entry` to the test module's imports if not already present — the existing test `use` line is `use crate::model::{sample_capture, sample_entry, Entry};` (already imports `Entry`), so no change needed.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib grouping 2>&1 | tail -12`
Expected: FAIL with "cannot find function `densest_window`".

- [ ] **Step 3: Implement.** Add this function to `src/grouping.rs` directly above the `#[cfg(test)] mod tests` block:

```rust
/// Densest sliding window over entries pre-sorted by `started_offset_ms`.
/// Returns `(count, left_idx, right_idx)` (inclusive) of the most populous
/// window no wider than `window_ms`. Returns `(0, 0, 0)` for an empty slice.
pub fn densest_window(entries: &[&Entry], window_ms: f64) -> (usize, usize, usize) {
    if entries.is_empty() {
        return (0, 0, 0);
    }
    let mut best = (1usize, 0usize, 0usize);
    let mut l = 0usize;
    for r in 0..entries.len() {
        while entries[r].started_offset_ms - entries[l].started_offset_ms > window_ms {
            l += 1;
        }
        let count = r - l + 1;
        if count > best.0 {
            best = (count, l, r);
        }
    }
    best
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib grouping 2>&1 | tail -10`
Expected: PASS (4 tests: 3 original + 1 new).

- [ ] **Step 5: Commit**

```bash
git add src/grouping.rs
git commit -m "feat: densest_window sliding-window helper"
```

---

### Task 3: `storms` command

**Files:**
- Create: `src/analysis/storms.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/storms.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_storms;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Capture, Entry};

    fn at(index: usize, host: &str, path: &str, offset_ms: f64) -> Entry {
        let mut e = sample_entry(index, host, "GET", path, 200);
        e.started_offset_ms = offset_ms;
        e
    }

    fn burst() -> Capture {
        // 6 calls to same endpoint within 250ms
        let mut es = Vec::new();
        for i in 0..6 {
            es.push(at(i, "h", "/x", i as f64 * 50.0));
        }
        sample_capture(es)
    }

    #[test]
    fn detects_endpoint_and_host_burst() {
        let r = compute_storms(&burst(), &Filter::parse(&[]).unwrap(), 1000, 5, 10);
        assert!(r.storms.iter().any(|s| s.scope_kind == "endpoint" && s.peak_count == 6));
        assert!(r.storms.iter().any(|s| s.scope_kind == "host" && s.peak_count == 6));
    }

    #[test]
    fn no_storm_when_spread_out() {
        let mut es = Vec::new();
        for i in 0..6 {
            es.push(at(i, "h", "/x", i as f64 * 1000.0)); // 1s apart
        }
        let r = compute_storms(&sample_capture(es), &Filter::parse(&[]).unwrap(), 500, 5, 10);
        assert!(r.storms.is_empty());
    }

    #[test]
    fn min_count_gates() {
        let r = compute_storms(&burst(), &Filter::parse(&[]).unwrap(), 1000, 7, 10);
        assert!(r.storms.is_empty()); // only 6 calls, need 7
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib storms 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_storms`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/storms.rs`:

```rust
use crate::filter::Filter;
use crate::grouping::densest_window;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StormsResult {
    pub storms: Vec<Storm>,
}

#[derive(Debug, Serialize)]
pub struct Storm {
    pub scope_kind: String, // "host" | "endpoint"
    pub scope: String,
    pub peak_count: usize,
    pub window_ms: u64,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub calls_per_sec: f64,
    pub entry_ids: Vec<String>,
}

/// Detect bursts of many calls to the same host or endpoint within `window_ms`.
pub fn compute_storms(
    cap: &Capture,
    filter: &Filter,
    window_ms: u64,
    min_count: usize,
    top: usize,
) -> StormsResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_host: AHashMap<String, Vec<&Entry>> = AHashMap::new();
    let mut by_endpoint: AHashMap<(String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_host.entry(e.host.clone()).or_default().push(e);
        by_endpoint
            .entry((e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut storms = Vec::new();
    for (host, mut g) in by_host {
        sort_by_offset(&mut g);
        if let Some(s) = storm_for("host", host, &g, window_ms, min_count) {
            storms.push(s);
        }
    }
    for ((host, np), mut g) in by_endpoint {
        sort_by_offset(&mut g);
        if let Some(s) = storm_for("endpoint", format!("{host}{np}"), &g, window_ms, min_count) {
            storms.push(s);
        }
    }

    storms.sort_by(|a, b| {
        b.peak_count
            .cmp(&a.peak_count)
            .then(a.scope.cmp(&b.scope))
            .then(a.scope_kind.cmp(&b.scope_kind))
    });
    storms.truncate(top);
    StormsResult { storms }
}

fn sort_by_offset(g: &mut [&Entry]) {
    g.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });
}

fn storm_for(
    kind: &str,
    scope: String,
    g: &[&Entry],
    window_ms: u64,
    min_count: usize,
) -> Option<Storm> {
    let (count, l, r) = densest_window(g, window_ms as f64);
    if count < min_count {
        return None;
    }
    let win = &g[l..=r];
    Some(Storm {
        scope_kind: kind.to_string(),
        scope,
        peak_count: count,
        window_ms,
        first_offset_ms: win.first().unwrap().started_offset_ms,
        last_offset_ms: win.last().unwrap().started_offset_ms,
        calls_per_sec: count as f64 * 1000.0 / window_ms as f64,
        entry_ids: win.iter().map(|e| e.id.clone()).collect(),
    })
}

/// Render storms as deterministic terminal text.
pub fn render_storms_text(r: &StormsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail storms ==\n");
    for s in &r.storms {
        out.push_str(&format!(
            "\n{} {}  {} calls in {} ({:.1}/s)\n",
            s.scope_kind,
            s.scope,
            s.peak_count,
            human_ms(s.window_ms as f64),
            s.calls_per_sec
        ));
        out.push_str(&format!(
            "  window: {} - {}\n",
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms)
        ));
        out.push_str(&format!("  entries: {}\n", s.entry_ids.join(", ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib storms 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/storms.rs
git commit -m "feat: storms command (sliding-window burst detection)"
```

---

### Task 4: `pagination` command (pagination loop + N+1)

**Files:**
- Create: `src/analysis/pagination.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/pagination.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_pagination;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn page(index: usize, page: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/items", 200);
        e.query = vec![("page".to_string(), page.to_string())];
        e.started_offset_ms = index as f64 * 10.0;
        e
    }

    #[test]
    fn detects_pagination_sequence() {
        let cap = sample_capture(vec![page(0, "1"), page(1, "2"), page(2, "3")]);
        let r = compute_pagination(&cap, &Filter::parse(&[]).unwrap(), 20, 5, 2000, 10);
        assert_eq!(r.pages.len(), 1);
        let p = &r.pages[0];
        assert_eq!(p.page_count, 3);
        assert_eq!(p.param_keys, vec!["page".to_string()]);
        assert!(!p.repeated_cursor);
        assert!(!p.excessive);
    }

    #[test]
    fn flags_repeated_cursor() {
        let cap = sample_capture(vec![page(0, "abc"), page(1, "abc")]);
        let r = compute_pagination(&cap, &Filter::parse(&[]).unwrap(), 20, 5, 2000, 10);
        assert!(r.pages[0].repeated_cursor);
    }

    #[test]
    fn detects_nplus1_fanout() {
        // one list call, then 5 detail calls to an {id} endpoint within window
        let mut es = Vec::new();
        let mut list = sample_entry(0, "api.x", "GET", "/items", 200);
        list.started_offset_ms = 0.0;
        es.push(list);
        for i in 1..=5 {
            let mut e = sample_entry(i, "api.x", "GET", "/items/{id}", 200);
            e.started_offset_ms = i as f64 * 20.0;
            es.push(e);
        }
        let r = compute_pagination(&sample_capture(es), &Filter::parse(&[]).unwrap(), 20, 5, 2000, 10);
        assert_eq!(r.nplus1.len(), 1);
        let n = &r.nplus1[0];
        assert_eq!(n.fanout, 5);
        assert_eq!(n.norm_path, "/items/{id}");
        assert_eq!(n.parent_id.as_deref(), Some("e000000"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib pagination 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_pagination`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/pagination.rs`:

```rust
use crate::filter::Filter;
use crate::grouping::densest_window;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

const PAGE_KEYS: &[&str] = &[
    "page", "offset", "cursor", "page_token", "after", "before", "start",
    "limit", "p", "pagenumber", "page_number",
];

#[derive(Debug, Serialize)]
pub struct PaginationResult {
    pub pages: Vec<PageSeq>,
    pub nplus1: Vec<NPlusOne>,
}

#[derive(Debug, Serialize)]
pub struct PageSeq {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub param_keys: Vec<String>,
    pub page_count: usize,
    pub repeated_cursor: bool,
    pub excessive: bool,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct NPlusOne {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub fanout: usize,
    pub parent_id: Option<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub entry_ids: Vec<String>,
}

fn is_id_bearing(np: &str) -> bool {
    np.contains("{id}") || np.contains("{blob}")
}

/// Detect pagination sequences and N+1 fan-out clusters.
pub fn compute_pagination(
    cap: &Capture,
    filter: &Filter,
    max_pages: usize,
    fanout_min: usize,
    window_ms: u64,
    top: usize,
) -> PaginationResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_route: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut pages = Vec::new();
    let mut nplus1 = Vec::new();

    for ((method, host, np), mut group) in by_route {
        group.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });

        // --- pagination sequence ---
        if group.len() >= 2 {
            let varying = varying_query_keys(&group);
            if !varying.is_empty()
                && varying
                    .iter()
                    .all(|k| PAGE_KEYS.contains(&k.to_ascii_lowercase().as_str()))
            {
                let mut values: Vec<String> = Vec::new();
                for e in &group {
                    for (k, v) in &e.query {
                        if varying.iter().any(|vk| vk == k) {
                            values.push(v.clone());
                        }
                    }
                }
                let repeated_cursor = has_duplicate(&values);
                pages.push(PageSeq {
                    host: host.clone(),
                    method: method.clone(),
                    norm_path: np.clone(),
                    param_keys: varying,
                    page_count: group.len(),
                    repeated_cursor,
                    excessive: group.len() > max_pages,
                    entry_ids: group.iter().map(|e| e.id.clone()).collect(),
                });
            }
        }

        // --- N+1 fan-out ---
        if is_id_bearing(&np) && group.len() >= fanout_min {
            let (count, l, r) = densest_window(&group, window_ms as f64);
            if count >= fanout_min {
                let win = &group[l..=r];
                let first = win.first().unwrap().started_offset_ms;
                let parent_id = parent_list_call(&entries, &host, first);
                nplus1.push(NPlusOne {
                    host: host.clone(),
                    method: method.clone(),
                    norm_path: np.clone(),
                    fanout: count,
                    parent_id,
                    first_offset_ms: first,
                    last_offset_ms: win.last().unwrap().started_offset_ms,
                    entry_ids: win.iter().map(|e| e.id.clone()).collect(),
                });
            }
        }
    }

    pages.sort_by(|a, b| b.page_count.cmp(&a.page_count).then(a.norm_path.cmp(&b.norm_path)));
    nplus1.sort_by(|a, b| b.fanout.cmp(&a.fanout).then(a.norm_path.cmp(&b.norm_path)));
    pages.truncate(top);
    nplus1.truncate(top);
    PaginationResult { pages, nplus1 }
}

/// Query keys whose value differs across the group (missing counts as a value).
fn varying_query_keys(members: &[&Entry]) -> Vec<String> {
    let all_keys: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.query.iter().map(|(k, _)| k.clone()))
        .collect();
    let mut varying: Vec<String> = Vec::new();
    for k in all_keys {
        let mut vals: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .query
                .iter()
                .find(|(qk, _)| *qk == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            vals.insert(v);
        }
        if vals.len() > 1 {
            varying.push(k);
        }
    }
    varying.sort();
    varying
}

fn has_duplicate(values: &[String]) -> bool {
    let mut seen: AHashSet<&String> = AHashSet::new();
    for v in values {
        if !seen.insert(v) {
            return true;
        }
    }
    false
}

/// The most recent non-id-bearing call to the same host before `before_offset`.
fn parent_list_call(entries: &[&Entry], host: &str, before_offset: f64) -> Option<String> {
    entries
        .iter()
        .filter(|e| e.host == host && e.started_offset_ms < before_offset && !is_id_bearing(&e.norm_path))
        .max_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|e| e.id.clone())
}

/// Render pagination/N+1 as deterministic terminal text.
pub fn render_pagination_text(r: &PaginationResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail pagination ==\n");
    if !r.pages.is_empty() {
        out.push_str("\npagination sequences:\n");
        for p in &r.pages {
            let mut tags = Vec::new();
            if p.repeated_cursor {
                tags.push("repeated-cursor");
            }
            if p.excessive {
                tags.push("excessive");
            }
            let tagstr = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) };
            out.push_str(&format!(
                "  {} pages  {} {}{}  (by {}){}\n",
                p.page_count,
                p.method,
                p.host,
                p.norm_path,
                p.param_keys.join(","),
                tagstr
            ));
        }
    }
    if !r.nplus1.is_empty() {
        out.push_str("\nN+1 fan-out:\n");
        for n in &r.nplus1 {
            let parent = n.parent_id.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "  {}x  {} {}{}  (after {})\n",
                n.fanout, n.method, n.host, n.norm_path, parent
            ));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib pagination 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/pagination.rs
git commit -m "feat: pagination command (pagination loop + N+1 fan-out)"
```

---

### Task 5: `rate-limit` command

**Files:**
- Create: `src/analysis/rate_limit.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/rate_limit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_rate_limit;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn limited(index: usize, offset_ms: f64, retry_after: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/data", 429);
        e.started_offset_ms = offset_ms;
        e.resp_headers = vec![
            ("Retry-After".to_string(), retry_after.to_string()),
            ("X-RateLimit-Remaining".to_string(), "0".to_string()),
        ];
        e
    }

    fn ok(index: usize, offset_ms: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/data", 200);
        e.started_offset_ms = offset_ms;
        e
    }

    #[test]
    fn groups_429_and_parses_retry_after() {
        let cap = sample_capture(vec![limited(0, 0.0, "30")]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.count_429, 1);
        assert_eq!(g.retry_after_secs, vec![30.0]);
        assert_eq!(g.ratelimit_headers.get("x-ratelimit-remaining").map(String::as_str), Some("0"));
    }

    #[test]
    fn flags_cooldown_violation() {
        // 429 at t=0 with Retry-After 10s; a follow-up call at t=2s violates cooldown
        let cap = sample_capture(vec![limited(0, 0.0, "10"), ok(1, 2000.0)]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups[0].cooldown_violated);
        assert_eq!(r.groups[0].violating_ids, vec!["e000001"]);
    }

    #[test]
    fn respected_cooldown_not_flagged() {
        // follow-up at t=20s is after the 10s cooldown
        let cap = sample_capture(vec![limited(0, 0.0, "10"), ok(1, 20000.0)]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(!r.groups[0].cooldown_violated);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib rate_limit 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_rate_limit`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/rate_limit.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct RateLimitResult {
    pub groups: Vec<RateLimitGroup>,
}

#[derive(Debug, Serialize)]
pub struct RateLimitGroup {
    pub host: String,
    pub norm_path: String,
    pub count_429: usize,
    pub retry_after_secs: Vec<f64>,
    pub ratelimit_headers: BTreeMap<String, String>,
    pub cooldown_violated: bool,
    pub violating_ids: Vec<String>,
    pub entry_ids: Vec<String>,
}

fn header<'a>(e: &'a Entry, name: &str) -> Option<&'a str> {
    e.resp_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn is_limited(e: &Entry) -> bool {
    e.status == 429 || header(e, "x-ratelimit-remaining") == Some("0")
}

/// Detect rate-limit events and cooldown violations.
pub fn compute_rate_limit(cap: &Capture, filter: &Filter, top: usize) -> RateLimitResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    // Index entries by route for cooldown follow-up lookups.
    let mut by_route: AHashMap<(String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut groups: Vec<RateLimitGroup> = Vec::new();
    for ((host, np), members) in &by_route {
        let limited: Vec<&&Entry> = members.iter().filter(|e| is_limited(e)).collect();
        if limited.is_empty() {
            continue;
        }

        let count_429 = limited.iter().filter(|e| e.status == 429).count();
        let mut retry_after_secs: Vec<f64> = Vec::new();
        let mut ratelimit_headers: BTreeMap<String, String> = BTreeMap::new();
        let mut violating_ids: Vec<String> = Vec::new();

        for lim in &limited {
            if let Some(ra) = header(lim, "retry-after").and_then(|v| v.trim().parse::<f64>().ok()) {
                retry_after_secs.push(ra);
                let cooldown_end = lim.started_offset_ms + ra * 1000.0;
                for e in members.iter() {
                    if e.started_offset_ms > lim.started_offset_ms
                        && e.started_offset_ms < cooldown_end
                        && !violating_ids.contains(&e.id)
                    {
                        violating_ids.push(e.id.clone());
                    }
                }
            }
            for (n, v) in &lim.resp_headers {
                let ln = n.to_ascii_lowercase();
                if ln.starts_with("x-ratelimit") {
                    ratelimit_headers.entry(ln).or_insert_with(|| v.clone());
                }
            }
        }
        violating_ids.sort();

        groups.push(RateLimitGroup {
            host: host.clone(),
            norm_path: np.clone(),
            count_429,
            retry_after_secs,
            ratelimit_headers,
            cooldown_violated: !violating_ids.is_empty(),
            violating_ids,
            entry_ids: limited.iter().map(|e| e.id.clone()).collect(),
        });
    }

    groups.sort_by(|a, b| {
        b.count_429
            .cmp(&a.count_429)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    RateLimitResult { groups }
}

/// Render rate-limit findings as deterministic terminal text.
pub fn render_rate_limit_text(r: &RateLimitResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail rate-limit ==\n");
    for g in &r.groups {
        let tag = if g.cooldown_violated { " [cooldown violated]" } else { "" };
        out.push_str(&format!(
            "\n{} {}  ({}x 429){}\n",
            g.host, g.norm_path, g.count_429, tag
        ));
        if !g.retry_after_secs.is_empty() {
            let ras: Vec<String> = g.retry_after_secs.iter().map(|s| format!("{s}s")).collect();
            out.push_str(&format!("  retry-after: {}\n", ras.join(", ")));
        }
        for (k, v) in &g.ratelimit_headers {
            out.push_str(&format!("  {k}: {v}\n"));
        }
        if !g.violating_ids.is_empty() {
            out.push_str(&format!("  called during cooldown: {}\n", g.violating_ids.join(", ")));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib rate_limit 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/rate_limit.rs
git commit -m "feat: rate-limit command (429 + Retry-After + cooldown violation)"
```

---

### Task 6: Wire the three commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::transitions::...;` line, add:

```rust
use har::analysis::storms::{compute_storms, render_storms_text};
use har::analysis::pagination::{compute_pagination, render_pagination_text};
use har::analysis::rate_limit::{compute_rate_limit, render_rate_limit_text};
```

- [ ] **Step 2: Add the subcommand variants.** Inside `enum Command { ... }`, after the `Curl { id: Option<String> },` variant, add:

```rust
    /// Bursts of many calls to the same host or endpoint within a window.
    Storms {
        /// Window width in milliseconds.
        #[arg(long, default_value_t = 1000)]
        window_ms: u64,
        /// Minimum calls in the window to count as a storm.
        #[arg(long, default_value_t = 5)]
        min_count: usize,
    },
    /// Pagination loops and N+1 fan-out clusters.
    Pagination {
        /// Page count above which a sequence is flagged excessive.
        #[arg(long, default_value_t = 20)]
        max_pages: usize,
        /// Minimum fan-out to flag an N+1 cluster.
        #[arg(long = "fanout-min", default_value_t = 5)]
        fanout_min: usize,
        /// Window (ms) for N+1 clustering.
        #[arg(long, default_value_t = 2000)]
        window_ms: u64,
    },
    /// Rate-limit (429) events, Retry-After, and cooldown violations.
    RateLimit,
```

- [ ] **Step 3: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::Curl { id } => { ... }` arm, add:

```rust
        Command::Storms { window_ms, min_count } => {
            let result = compute_storms(&cap, &filter, window_ms, min_count, cli.top);
            let findings = !result.storms.is_empty();
            emit(
                cli.json,
                "storms",
                &cap.meta,
                &result,
                &render_storms_text(&result),
                &["pagination", "duplicates", "timeline"],
            );
            exit(findings);
        }
        Command::Pagination { max_pages, fanout_min, window_ms } => {
            let result = compute_pagination(&cap, &filter, max_pages, fanout_min, window_ms, cli.top);
            let findings = !result.pages.is_empty() || !result.nplus1.is_empty();
            emit(
                cli.json,
                "pagination",
                &cap.meta,
                &result,
                &render_pagination_text(&result),
                &["storms", "duplicates", "endpoints"],
            );
            exit(findings);
        }
        Command::RateLimit => {
            let result = compute_rate_limit(&cap, &filter, cli.top);
            let findings = !result.groups.is_empty();
            emit(
                cli.json,
                "rate-limit",
                &cap.meta,
                &result,
                &render_rate_limit_text(&result),
                &["errors", "retries", "transitions"],
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
cargo run --quiet -- tests/fixtures/someapi123.har storms
cargo run --quiet -- tests/fixtures/someapi123.har pagination
cargo run --quiet -- tests/fixtures/someapi123.har rate-limit --json | head -6
```
Expected: `storms`/`pagination` print their headers with no findings (single-entry
fixture); `rate-limit --json` prints an envelope with `"command": "rate-limit"`.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire storms/pagination/rate-limit commands into CLI"
```

---

### Task 7: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_patterns.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_patterns.rs`:

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
fn storms_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "storms"]);
    assert!(stdout.contains("== wiretrail storms =="));
}

#[test]
fn pagination_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "pagination", "--json"]);
    assert!(stdout.contains("\"command\": \"pagination\""));
    assert!(stdout.contains("\"pages\""));
    assert!(stdout.contains("\"nplus1\""));
}

#[test]
fn rate_limit_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "rate-limit", "--json"]);
    assert!(stdout.contains("\"command\": \"rate-limit\""));
    assert!(stdout.contains("\"groups\""));
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_patterns 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures.

- [ ] **Step 4: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
HAR="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$HAR" storms 2>/dev/null | head -16
./target/release/wiretrail "$HAR" pagination 2>/dev/null | head -16
```
Expected: `storms` surfaces the YouTube host fan-out and the 21× addon-manifest
endpoint bursts; `pagination` surfaces N+1 clusters on `{id}`/`{blob}` endpoints
(e.g. the Kitsu `/api/edge/anime/{id}` detail fan-out). Output uses `{blob}` for
the addon config segments (no secret leak).

- [ ] **Step 5: Commit**

```bash
git add tests/cli_patterns.rs
git commit -m "test: end-to-end tests for storms/pagination/rate-limit"
```

---

## Self-review

**Spec coverage (Phase M1):**
- `storms` (#25) — sliding-window burst, host + endpoint scope, `--window-ms`/`--min-count` → Task 3. ✓
- `pagination` loop (#27) — page-key variance, repeated-cursor, excessive → Task 4. ✓
- N+1 (#26) — id-bearing endpoint fan-out in window + parent list call → Task 4. ✓
- `rate-limit` (#28) — 429 grouping, Retry-After parse, X-RateLimit headers, cooldown violation → Task 5. ✓
- All: `--json`, filter, `--top`, next_commands, findings exit codes → Task 6. ✓
- `densest_window` shared by storms + N+1 → Task 2. ✓

**Placeholder scan:** No TBD/TODO in task steps; every code step is complete; every command step states expected output. ✓

**Type consistency:**
- `densest_window(&[&Entry], f64) -> (usize, usize, usize)` (Task 2) called identically in `storms` (Task 3) and `pagination` (Task 4). ✓
- `compute_storms(&Capture, &Filter, u64, usize, usize)`, `compute_pagination(&Capture, &Filter, usize, usize, u64, usize)`, `compute_rate_limit(&Capture, &Filter, usize)` — Task 6 dispatch arms pass args in matching order/type (storms: window_ms,min_count,top; pagination: max_pages,fanout_min,window_ms,top; rate-limit: top). ✓
- `render_storms_text`/`render_pagination_text`/`render_rate_limit_text` take `&<Cmd>Result` — used in Task 6. ✓
- Result structs derive `Serialize`; `emit`/`exit` reused unchanged. ✓
- `Entry` fields used (`started_offset_ms`, `index`, `host`, `norm_path`, `method`, `status`, `query`, `resp_headers`, `id`) all exist. ✓
- `RateLimit` variant has no fields → dispatch arm `Command::RateLimit =>` matches; `Storms`/`Pagination` destructure their flag fields. ✓
