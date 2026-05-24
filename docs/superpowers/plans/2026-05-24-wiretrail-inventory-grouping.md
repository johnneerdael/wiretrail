# wiretrail Inventory & Grouping (`hosts`, `subsystems`, `endpoints`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `hosts`, `subsystems`, and `endpoints` commands to `wiretrail`, plus a config layer (built-in vendor heuristics + optional `wiretrail.yaml` ownership map) that powers the dossier-style subsystem category table.

**Architecture:** Build on the Plan 1 foundation (`har::model::{Capture, Entry}`, `har::filter::Filter`, `har::render::Envelope`, `har::loader`/`har::assemble`). Add three small pure analysis modules that fold the normalized `Capture` into per-host, per-subsystem, and per-endpoint aggregates, each with a serializable result + deterministic terminal renderer. Subsystem grouping resolves each entry through ownership rules (YAML) → built-in vendor heuristics → raw host. A shared glob module (extracted from the Plan 1 filter) and a percentile helper avoid duplication.

**Tech Stack:** Rust 2024, clap 4, serde/serde_json, yaml_serde (config), ahash, plus the Plan 1 modules.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-har-analyzer-design.md` (sections
on `hosts`, `subsystems`, `endpoints`, the YAML ownership map, and vendor heuristics).

This is **Plan 2 of 4**. Plan 1 (foundation + `summary`) is complete and on `main`.
Plan 3 (failures/timing) and Plan 4 (exports) follow and reuse everything here.

## Prerequisites (already in place from Plan 1)

- `har::model::{Capture, CaptureMeta, Entry, Phases, Sizes}`, `Entry::status_class()`, `Entry::is_error()`.
- `har::filter::Filter` with `parse(&[String])` / `matches(&Entry)`.
- `har::fingerprint::fingerprint(&Entry) -> String`.
- `har::render::{Envelope, ExitCode, human_bytes, human_ms}`.
- `har::loader::load` + `har::assemble::assemble`.
- The glob matcher currently lives **privately** in `src/filter.rs` (`glob_match`). Task 2 extracts it to a shared module.

## File structure

```
Cargo.toml                    # Modify: yaml_serde becomes non-optional; feature `yaml` = []
src/lib.rs                    # Modify: add pub mod config, glob, stats, vendor
src/glob.rs                   # NEW: shared glob_match (moved from filter.rs)
src/filter.rs                 # Modify: use crate::glob::glob_match, drop the private copy
src/stats.rs                  # NEW: percentile helper
src/vendor.rs                 # NEW: built-in host -> vendor name heuristics
src/config.rs                 # NEW: Config (YAML ownership map) + subsystem resolution
src/model.rs                  # Modify: add cfg(test) sample_entry / sample_capture helpers
src/analysis/mod.rs           # Modify: add pub mod hosts, subsystems, endpoints
src/analysis/hosts.rs         # NEW: per-host aggregate + renderer
src/analysis/subsystems.rs    # NEW: per-subsystem aggregate + renderer
src/analysis/endpoints.rs     # NEW: per-endpoint aggregate + renderer
src/main.rs                   # Modify: add Hosts/Subsystems/Endpoints subcommands + --config
tests/cli_inventory.rs        # NEW: end-to-end binary tests
```

---

### Task 1: Cargo + module scaffolding

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Make `yaml_serde` non-optional in `Cargo.toml`.** Replace the dependency line:

```toml
yaml_serde = { version = "0.10.4", optional = true }
```
with:
```toml
yaml_serde = "0.10.4"
```

- [ ] **Step 2: Update the `[features]` table in `Cargo.toml`** so the `yaml` feature no longer references the now-non-optional dep (it stays as a pure cfg gate for the library's `to_yaml`). Replace:

```toml
[features]
default = []
yaml = ["dep:yaml_serde"]
```
with:
```toml
[features]
default = []
yaml = []
```

- [ ] **Step 3: Declare the new top-level modules in `src/lib.rs`.** After the existing `pub mod render;` line (the last of the Plan 1 additions), add:

```rust
pub mod config;
pub mod glob;
pub mod stats;
pub mod vendor;
```

- [ ] **Step 4: Declare the new analysis modules.** `src/analysis/mod.rs` currently contains `pub mod summary;`. Replace its entire contents with:

```rust
pub mod endpoints;
pub mod hosts;
pub mod subsystems;
pub mod summary;
```

- [ ] **Step 5: Create empty module files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
for f in glob stats vendor config; do touch "src/$f.rs"; done
for f in hosts subsystems endpoints; do touch "src/analysis/$f.rs"; done
cargo build 2>&1 | tail -5
```
Expected: build SUCCEEDS (empty modules are valid). Warnings are fine.

- [ ] **Step 6: Confirm the yaml feature still compiles** (the regression test references `to_yaml` under `--features yaml`).

Run: `cargo test --features yaml --test regression 2>&1 | tail -5`
Expected: PASS (7 tests).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/lib.rs src/analysis/mod.rs src/glob.rs src/stats.rs src/vendor.rs src/config.rs src/analysis/hosts.rs src/analysis/subsystems.rs src/analysis/endpoints.rs
git commit -m "chore: scaffold inventory/grouping modules; yaml_serde non-optional"
```

---

### Task 2: Extract shared glob matcher

**Files:**
- Create: `src/glob.rs`
- Modify: `src/filter.rs`

- [ ] **Step 1: Write `src/glob.rs`** with the matcher (moved verbatim from `filter.rs`) plus its own tests:

```rust
/// Minimal glob: `*` matches any run of characters. Case-insensitive.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let t = text.to_ascii_lowercase();
    if !p.contains('*') {
        return p == t;
    }
    let parts: Vec<&str> = p.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !t[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            return t[pos..].ends_with(part);
        } else if let Some(found) = t[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::glob_match;

    #[test]
    fn exact_match_no_star() {
        assert!(glob_match("api.foo.com", "API.FOO.COM"));
        assert!(!glob_match("api.foo.com", "api.bar.com"));
    }

    #[test]
    fn star_matches_substring() {
        assert!(glob_match("*login*", "/v1/login/start"));
        assert!(glob_match("api.*.com", "api.foo.com"));
        assert!(!glob_match("api.*.com", "cdn.foo.net"));
    }

    #[test]
    fn leading_and_trailing_star() {
        assert!(glob_match("*.nexioapp.org", "torii.nexioapp.org"));
        assert!(glob_match("torii.*", "torii.nexioapp.org"));
    }
}
```

- [ ] **Step 2: Update `src/filter.rs` to use the shared matcher.** Add this import directly below the existing `use crate::model::Entry;` line at the top of the file:

```rust
use crate::glob::glob_match;
```

- [ ] **Step 3: Delete the private `glob_match` from `src/filter.rs`.** Remove the entire function block that begins with the doc comment `/// Minimal glob: \`*\` matches any run of characters. Case-insensitive.` and its `fn glob_match(pattern: &str, text: &str) -> bool { ... }` body (the last function before the `#[cfg(test)]` module). Do not remove anything else.

- [ ] **Step 4: Run the glob + filter tests**

Run: `cargo test --lib glob 2>&1 | tail -8 && cargo test --lib filter 2>&1 | tail -8`
Expected: glob tests PASS (3), filter tests PASS (4) — filter now delegates to the shared matcher.

- [ ] **Step 5: Commit**

```bash
git add src/glob.rs src/filter.rs
git commit -m "refactor: extract shared glob matcher used by filter and config"
```

---

### Task 3: Percentile helper

**Files:**
- Create: `src/stats.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/stats.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{percentiles, Percentiles};

    #[test]
    fn empty_is_zero() {
        let p = percentiles(&[]);
        assert_eq!(p.p50, 0.0);
        assert_eq!(p.p95, 0.0);
        assert_eq!(p.max, 0.0);
    }

    #[test]
    fn single_value() {
        let p = percentiles(&[42.0]);
        assert_eq!(p.p50, 42.0);
        assert_eq!(p.p95, 42.0);
        assert_eq!(p.max, 42.0);
    }

    #[test]
    fn nearest_rank_five_values() {
        // sorted: 10,20,30,40,50 ; p50 -> rank ceil(2.5)=3 -> 30 ; p95 -> rank ceil(4.75)=5 -> 50
        let p = percentiles(&[50.0, 10.0, 40.0, 20.0, 30.0]);
        assert_eq!(p.p50, 30.0);
        assert_eq!(p.p95, 50.0);
        assert_eq!(p.max, 50.0);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib stats 2>&1 | tail -12`
Expected: FAIL with "cannot find function `percentiles`".

- [ ] **Step 3: Implement** above the test module in `src/stats.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

/// Nearest-rank percentiles over a set of values. Deterministic; does not mutate input.
pub fn percentiles(values: &[f64]) -> Percentiles {
    if values.is_empty() {
        return Percentiles { p50: 0.0, p95: 0.0, max: 0.0 };
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    let pick = |p: f64| -> f64 {
        let rank = ((p / 100.0) * n as f64).ceil() as usize;
        let idx = rank.saturating_sub(1).min(n - 1);
        v[idx]
    };
    Percentiles {
        p50: pick(50.0),
        p95: pick(95.0),
        max: *v.last().unwrap(),
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib stats 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/stats.rs
git commit -m "feat: nearest-rank percentile helper"
```

---

### Task 4: Built-in vendor heuristics

**Files:**
- Create: `src/vendor.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/vendor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::vendor_for;

    #[test]
    fn known_vendors() {
        assert_eq!(vendor_for("api.github.com"), Some("GitHub"));
        assert_eq!(vendor_for("raw.githubusercontent.com"), Some("GitHub"));
        assert_eq!(vendor_for("yjyuomfgkqwmjvnoxurn.supabase.co"), Some("Supabase"));
        assert_eq!(vendor_for("youtubei.googleapis.com"), Some("Google"));
        assert_eq!(vendor_for("api.themoviedb.org"), Some("TMDB"));
    }

    #[test]
    fn unknown_host_is_none() {
        assert_eq!(vendor_for("torii.nexioapp.org"), None);
        assert_eq!(vendor_for(""), None);
    }

    #[test]
    fn matches_on_suffix_not_substring() {
        // "notgithub.com" must NOT match "github.com"
        assert_eq!(vendor_for("notgithub.com"), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib vendor 2>&1 | tail -12`
Expected: FAIL with "cannot find function `vendor_for`".

- [ ] **Step 3: Implement** above the test module in `src/vendor.rs`:

```rust
/// (domain suffix, friendly vendor name). Matched as a dotted-label suffix so
/// `notgithub.com` does not match `github.com`.
const VENDORS: &[(&str, &str)] = &[
    ("github.com", "GitHub"),
    ("githubusercontent.com", "GitHub"),
    ("supabase.co", "Supabase"),
    ("googleapis.com", "Google"),
    ("youtube.com", "Google"),
    ("googlevideo.com", "Google"),
    ("google.com", "Google"),
    ("gstatic.com", "Google"),
    ("themoviedb.org", "TMDB"),
    ("tmdb.org", "TMDB"),
    ("cloudflare.com", "Cloudflare"),
    ("amazonaws.com", "AWS"),
    ("sentry.io", "Sentry"),
];

/// Map a host to a known vendor name, or None when unrecognized.
pub fn vendor_for(host: &str) -> Option<&'static str> {
    if host.is_empty() {
        return None;
    }
    for (suffix, name) in VENDORS {
        if host == *suffix || host.ends_with(&format!(".{suffix}")) {
            return Some(name);
        }
    }
    None
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib vendor 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/vendor.rs
git commit -m "feat: built-in vendor host heuristics"
```

---

### Task 5: Shared test entry builders

**Files:**
- Modify: `src/model.rs`

These `#[cfg(test)]` helpers are compiled into the whole `har` lib during `cargo test`, so every module's unit tests (Tasks 6–8) can build entries without repeating the 20-field literal.

- [ ] **Step 1: Append the helpers to the end of `src/model.rs`** (after the `impl Entry { ... }` block):

```rust
#[cfg(test)]
pub fn sample_entry(index: usize, host: &str, method: &str, path: &str, status: i64) -> Entry {
    Entry {
        id: format_entry_id(index),
        index,
        started_offset_ms: index as f64 * 10.0,
        duration_ms: 10.0,
        method: method.to_string(),
        url: format!("https://{host}{path}"),
        host: host.to_string(),
        path: path.to_string(),
        norm_path: path.to_string(),
        query: Vec::new(),
        status,
        status_text: String::new(),
        resource_type: ResourceType::Api,
        content_type: Some("application/json".to_string()),
        req_headers: Vec::new(),
        resp_headers: Vec::new(),
        req_body: None,
        resp_body: None,
        timings: Phases::default(),
        sizes: Sizes::default(),
        server_ip: None,
        http_version: "HTTP/2".to_string(),
        redirect_url: None,
        correlation: Vec::new(),
    }
}

#[cfg(test)]
pub fn sample_capture(entries: Vec<Entry>) -> Capture {
    let meta = CaptureMeta {
        har_version: "1.2".to_string(),
        creator: "test".to_string(),
        creator_version: "0".to_string(),
        browser: None,
        entry_count: entries.len(),
        start_ms: Some(0),
        end_ms: Some(0),
        duration_ms: 0.0,
    };
    Capture { meta, entries }
}
```

- [ ] **Step 2: Verify it compiles in test mode** (no behavior yet, just that the helpers typecheck):

Run: `cargo test --lib model 2>&1 | tail -6`
Expected: builds; "0 tests" run in the `model` filter (there are no `model` tests, that's fine — the key is it compiles).

- [ ] **Step 3: Commit**

```bash
git add src/model.rs
git commit -m "test: shared sample_entry/sample_capture builders"
```

---

### Task 6: `hosts` analysis

**Files:**
- Create: `src/analysis/hosts.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/hosts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_hosts;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "api.foo.com", "GET", "/v1/a", 200),
            sample_entry(1, "api.foo.com", "GET", "/v1/a", 200), // duplicate of e0
            sample_entry(2, "api.foo.com", "POST", "/v1/b", 500),
            sample_entry(3, "cdn.bar.com", "GET", "/img", 200),
        ];
        entries[2].duration_ms = 100.0;
        sample_capture(entries)
    }

    #[test]
    fn groups_by_host_with_counts_and_errors() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let foo = r.hosts.iter().find(|h| h.host == "api.foo.com").unwrap();
        assert_eq!(foo.count, 3);
        assert_eq!(foo.error_count, 1);
        assert_eq!(foo.methods.get("GET"), Some(&2));
        assert_eq!(foo.methods.get("POST"), Some(&1));
        // e0 and e1 are identical -> 2 duplicate members
        assert_eq!(foo.duplicate_count, 2);
        assert_eq!(foo.max_ms, 100.0);
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.hosts[0].host, "api.foo.com"); // 3 > 1
    }

    #[test]
    fn top_bounds_list() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 1);
        assert_eq!(r.hosts.len(), 1);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib hosts 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_hosts`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/hosts.rs`:

```rust
use crate::fingerprint::fingerprint;
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::{human_bytes, human_ms};
use crate::stats::percentiles;
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct HostsResult {
    pub hosts: Vec<HostStat>,
}

#[derive(Debug, Serialize)]
pub struct HostStat {
    pub host: String,
    pub count: usize,
    pub methods: BTreeMap<String, usize>,
    pub status_classes: BTreeMap<String, usize>,
    pub error_count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub bytes_sent: i64,
    pub bytes_received: i64,
    pub duplicate_count: usize,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

/// Aggregate the filtered capture per host. `top` bounds the returned list.
pub fn compute_hosts(cap: &Capture, filter: &Filter, top: usize) -> HostsResult {
    let mut by_host: AHashMap<String, Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        by_host.entry(e.host.clone()).or_default().push(e);
    }

    let mut hosts: Vec<HostStat> = by_host
        .into_iter()
        .map(|(host, entries)| host_stat(host, &entries))
        .collect();

    hosts.sort_by(|a, b| b.count.cmp(&a.count).then(a.host.cmp(&b.host)));
    hosts.truncate(top);
    HostsResult { hosts }
}

fn host_stat(host: String, entries: &[&Entry]) -> HostStat {
    let mut methods: BTreeMap<String, usize> = BTreeMap::new();
    let mut status_classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    let mut durations: Vec<f64> = Vec::with_capacity(entries.len());
    let mut error_count = 0usize;
    let mut bytes_sent = 0i64;
    let mut bytes_received = 0i64;
    let mut first = f64::MAX;
    let mut last = f64::MIN;

    for e in entries {
        *methods.entry(e.method.to_ascii_uppercase()).or_default() += 1;
        *status_classes.entry(status_class_label(e.status_class())).or_default() += 1;
        if e.is_error() {
            error_count += 1;
        }
        durations.push(e.duration_ms);
        bytes_sent += e.sizes.req_body.max(0);
        bytes_received += e.sizes.resp_content.max(e.sizes.resp_body).max(0);
        first = first.min(e.started_offset_ms);
        last = last.max(e.started_offset_ms);
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }

    let duplicate_count: usize = fp_counts.values().filter(|c| **c > 1).sum();
    let p = percentiles(&durations);

    HostStat {
        host,
        count: entries.len(),
        methods,
        status_classes,
        error_count,
        p50_ms: p.p50,
        p95_ms: p.p95,
        max_ms: p.max,
        bytes_sent,
        bytes_received,
        duplicate_count,
        first_offset_ms: if first == f64::MAX { 0.0 } else { first },
        last_offset_ms: if last == f64::MIN { 0.0 } else { last },
    }
}

fn status_class_label(class: i64) -> String {
    match class {
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "other",
    }
    .to_string()
}

/// Render hosts as deterministic terminal text.
pub fn render_hosts_text(r: &HostsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail hosts ==\n");
    for h in &r.hosts {
        out.push_str(&format!(
            "\n{}  ({} req, {} err, {} dup)\n",
            h.host, h.count, h.error_count, h.duplicate_count
        ));
        out.push_str(&format!(
            "  latency p50/p95/max: {} / {} / {}\n",
            human_ms(h.p50_ms),
            human_ms(h.p95_ms),
            human_ms(h.max_ms)
        ));
        out.push_str(&format!(
            "  bytes sent/received: {} / {}\n",
            human_bytes(h.bytes_sent),
            human_bytes(h.bytes_received)
        ));
        let methods: Vec<String> = h.methods.iter().map(|(m, c)| format!("{m}:{c}")).collect();
        out.push_str(&format!("  methods: {}\n", methods.join(" ")));
        let statuses: Vec<String> =
            h.status_classes.iter().map(|(s, c)| format!("{s}:{c}")).collect();
        out.push_str(&format!("  status: {}\n", statuses.join(" ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib hosts 2>&1 | tail -8`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/hosts.rs
git commit -m "feat: hosts command analysis + renderer"
```

---

### Task 7: `subsystems` analysis + config

**Files:**
- Create: `src/config.rs`
- Create: `src/analysis/subsystems.rs`

- [ ] **Step 1: Write the failing config tests** at the top of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::Config;
    use crate::model::sample_entry;

    #[test]
    fn parses_ownership_rules_from_yaml() {
        let yaml = r#"
ownership:
  - name: Torii Addon
    host: "torii.*"
    owner: Addons
    criticality: high
  - name: GitHub Releases
    host: "api.github.com"
    path: "/repos/*"
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.ownership.len(), 2);
    }

    #[test]
    fn rule_match_wins_over_vendor() {
        let cfg = Config::from_yaml_str(
            "ownership:\n  - name: Torii Addon\n    host: \"torii.*\"\n",
        )
        .unwrap();
        let e = sample_entry(0, "torii.nexioapp.org", "GET", "/manifest.json", 308);
        let s = cfg.subsystem_for(&e);
        assert_eq!(s.name, "Torii Addon");
    }

    #[test]
    fn falls_back_to_vendor_then_host() {
        let cfg = Config::default();
        let gh = sample_entry(0, "api.github.com", "GET", "/x", 200);
        assert_eq!(cfg.subsystem_for(&gh).name, "GitHub");
        let unknown = sample_entry(1, "torii.nexioapp.org", "GET", "/x", 200);
        assert_eq!(cfg.subsystem_for(&unknown).name, "torii.nexioapp.org");
    }

    #[test]
    fn path_rule_requires_path_match() {
        let cfg = Config::from_yaml_str(
            "ownership:\n  - name: Repos\n    host: \"api.github.com\"\n    path: \"/repos/*\"\n",
        )
        .unwrap();
        let hit = sample_entry(0, "api.github.com", "GET", "/repos/foo/bar", 200);
        let miss = sample_entry(1, "api.github.com", "GET", "/users/foo", 200);
        assert_eq!(cfg.subsystem_for(&hit).name, "Repos");
        // miss does not match the rule -> vendor fallback
        assert_eq!(cfg.subsystem_for(&miss).name, "GitHub");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib config 2>&1 | tail -12`
Expected: FAIL with "cannot find type `Config`".

- [ ] **Step 3: Implement** above the test module in `src/config.rs`:

```rust
use crate::glob::glob_match;
use crate::model::Entry;
use crate::vendor::vendor_for;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ownership: Vec<OwnershipRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OwnershipRule {
    pub name: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub criticality: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Subsystem {
    pub name: String,
    pub owner: Option<String>,
    pub criticality: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file")]
    Io(#[source] std::io::Error),
    #[error("failed to parse config YAML")]
    Parse(#[source] yaml_serde::Error),
}

impl Config {
    /// Load config from an explicit path, or discover `wiretrail.yaml` in the
    /// current directory. A missing default file yields an empty config.
    pub fn load(explicit: Option<&Path>) -> Result<Config, ConfigError> {
        match explicit {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(ConfigError::Io)?;
                Config::from_yaml_str(&text)
            }
            None => {
                let default = Path::new("wiretrail.yaml");
                if default.is_file() {
                    let text = std::fs::read_to_string(default).map_err(ConfigError::Io)?;
                    Config::from_yaml_str(&text)
                } else {
                    Ok(Config::default())
                }
            }
        }
    }

    pub fn from_yaml_str(s: &str) -> Result<Config, ConfigError> {
        yaml_serde::from_str(s).map_err(ConfigError::Parse)
    }

    /// Resolve an entry's subsystem: first matching ownership rule, then a
    /// built-in vendor name, then the raw host.
    pub fn subsystem_for(&self, e: &Entry) -> Subsystem {
        for rule in &self.ownership {
            if rule_matches(rule, e) {
                return Subsystem {
                    name: rule.name.clone(),
                    owner: rule.owner.clone(),
                    criticality: rule.criticality.clone(),
                };
            }
        }
        if let Some(v) = vendor_for(&e.host) {
            return Subsystem { name: v.to_string(), owner: None, criticality: None };
        }
        let name = if e.host.is_empty() { "(unknown)".to_string() } else { e.host.clone() };
        Subsystem { name, owner: None, criticality: None }
    }
}

fn rule_matches(rule: &OwnershipRule, e: &Entry) -> bool {
    // A rule with neither host nor path never matches (avoids accidental catch-all).
    if rule.host.is_none() && rule.path.is_none() {
        return false;
    }
    if let Some(h) = &rule.host {
        if !glob_match(h, &e.host) {
            return false;
        }
    }
    if let Some(p) = &rule.path {
        if !glob_match(p, &e.path) {
            return false;
        }
    }
    true
}
```

- [ ] **Step 4: Run config tests to verify pass**

Run: `cargo test --lib config 2>&1 | tail -8`
Expected: PASS (4 tests).

- [ ] **Step 5: Write the failing subsystems tests** at the top of `src/analysis/subsystems.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_subsystems;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "api.github.com", "GET", "/repos/x", 200),
            sample_entry(1, "raw.githubusercontent.com", "GET", "/y", 404),
            sample_entry(2, "torii.nexioapp.org", "GET", "/manifest.json", 308),
        ])
    }

    #[test]
    fn groups_by_resolved_subsystem() {
        let r = compute_subsystems(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10);
        let gh = r.subsystems.iter().find(|s| s.name == "GitHub").unwrap();
        // both github hosts collapse into one subsystem
        assert_eq!(gh.count, 2);
        assert_eq!(gh.error_count, 1);
        assert_eq!(gh.hosts.len(), 2);
        // unknown host becomes its own subsystem named after the host
        assert!(r.subsystems.iter().any(|s| s.name == "torii.nexioapp.org"));
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_subsystems(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10);
        assert_eq!(r.subsystems[0].name, "GitHub");
    }
}
```

- [ ] **Step 6: Run to verify failure**

Run: `cargo test --lib subsystems 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_subsystems`".

- [ ] **Step 7: Implement** above the test module in `src/analysis/subsystems.rs`:

```rust
use crate::config::Config;
use crate::fingerprint::fingerprint;
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SubsystemsResult {
    pub subsystems: Vec<SubsystemStat>,
}

#[derive(Debug, Serialize)]
pub struct SubsystemStat {
    pub name: String,
    pub owner: Option<String>,
    pub criticality: Option<String>,
    pub count: usize,
    pub hosts: Vec<String>,
    pub error_count: usize,
    pub duplicate_count: usize,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

struct Acc<'a> {
    owner: Option<String>,
    criticality: Option<String>,
    entries: Vec<&'a Entry>,
    hosts: AHashSet<String>,
}

/// Aggregate the filtered capture per resolved subsystem. `top` bounds the list.
pub fn compute_subsystems(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    top: usize,
) -> SubsystemsResult {
    let mut by_name: AHashMap<String, Acc> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let sub = config.subsystem_for(e);
        let acc = by_name.entry(sub.name.clone()).or_insert_with(|| Acc {
            owner: sub.owner.clone(),
            criticality: sub.criticality.clone(),
            entries: Vec::new(),
            hosts: AHashSet::new(),
        });
        acc.entries.push(e);
        acc.hosts.insert(e.host.clone());
    }

    let mut subsystems: Vec<SubsystemStat> = by_name
        .into_iter()
        .map(|(name, acc)| subsystem_stat(name, acc))
        .collect();

    subsystems.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    subsystems.truncate(top);
    SubsystemsResult { subsystems }
}

fn subsystem_stat(name: String, acc: Acc) -> SubsystemStat {
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    let mut error_count = 0usize;
    let mut first = f64::MAX;
    let mut last = f64::MIN;
    for e in &acc.entries {
        if e.is_error() {
            error_count += 1;
        }
        first = first.min(e.started_offset_ms);
        last = last.max(e.started_offset_ms);
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }
    let duplicate_count: usize = fp_counts.values().filter(|c| **c > 1).sum();
    let mut hosts: Vec<String> = acc.hosts.into_iter().collect();
    hosts.sort();

    SubsystemStat {
        name,
        owner: acc.owner,
        criticality: acc.criticality,
        count: acc.entries.len(),
        hosts,
        error_count,
        duplicate_count,
        first_offset_ms: if first == f64::MAX { 0.0 } else { first },
        last_offset_ms: if last == f64::MIN { 0.0 } else { last },
    }
}

/// Render the dossier-style subsystem category table as terminal text.
pub fn render_subsystems_text(r: &SubsystemsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail subsystems ==\n");
    for s in &r.subsystems {
        let owner = s.owner.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "\n{}  [{}]  ({} req, {} err, {} dup)\n",
            s.name, owner, s.count, s.error_count, s.duplicate_count
        ));
        out.push_str(&format!(
            "  window: {} - {}\n",
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms)
        ));
        out.push_str(&format!("  hosts: {}\n", s.hosts.join(", ")));
    }
    out
}
```

- [ ] **Step 8: Run to verify pass**

Run: `cargo test --lib subsystems 2>&1 | tail -8 && cargo test --lib config 2>&1 | tail -4`
Expected: subsystems PASS (2), config PASS (4).

- [ ] **Step 9: Commit**

```bash
git add src/config.rs src/analysis/subsystems.rs
git commit -m "feat: config ownership map + subsystems command analysis"
```

---

### Task 8: `endpoints` analysis

**Files:**
- Create: `src/analysis/endpoints.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/endpoints.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_endpoints;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e0 = sample_entry(0, "api.foo.com", "GET", "/v1/users/{id}", 200);
        e0.query = vec![("page".into(), "1".into())];
        let mut e1 = sample_entry(1, "api.foo.com", "GET", "/v1/users/{id}", 404);
        e1.query = vec![("expand".into(), "true".into())];
        let e2 = sample_entry(2, "api.foo.com", "POST", "/v1/users/{id}", 200);
        sample_capture(vec![e0, e1, e2])
    }

    #[test]
    fn groups_by_method_host_normpath() {
        let r = compute_endpoints(&cap(), &Filter::parse(&[]).unwrap(), 10);
        // GET .../{id} and POST .../{id} are distinct endpoints
        let get = r
            .endpoints
            .iter()
            .find(|e| e.method == "GET" && e.norm_path == "/v1/users/{id}")
            .unwrap();
        assert_eq!(get.count, 2);
        assert_eq!(get.statuses.get("200"), Some(&1));
        assert_eq!(get.statuses.get("404"), Some(&1));
        assert_eq!(get.error_count, 1);
        // observed query keys are collected and sorted/deduped
        assert_eq!(get.sample_query_keys, vec!["expand".to_string(), "page".to_string()]);
        assert!(r.endpoints.iter().any(|e| e.method == "POST"));
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_endpoints(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.endpoints[0].count, 2);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib endpoints 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_endpoints`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/endpoints.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct EndpointsResult {
    pub endpoints: Vec<EndpointStat>,
}

#[derive(Debug, Serialize)]
pub struct EndpointStat {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub count: usize,
    pub statuses: BTreeMap<String, usize>,
    pub content_types: Vec<String>,
    pub sample_query_keys: Vec<String>,
    pub error_count: usize,
}

struct Acc<'a> {
    entries: Vec<&'a Entry>,
}

/// Build a normalized endpoint inventory, keyed by (method, host, norm_path).
pub fn compute_endpoints(cap: &Capture, filter: &Filter, top: usize) -> EndpointsResult {
    let mut by_key: AHashMap<(String, String, String), Acc> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let key = (e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone());
        by_key.entry(key).or_insert_with(|| Acc { entries: Vec::new() }).entries.push(e);
    }

    let mut endpoints: Vec<EndpointStat> = by_key
        .into_iter()
        .map(|((method, host, norm_path), acc)| endpoint_stat(method, host, norm_path, &acc.entries))
        .collect();

    endpoints.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
            .then(a.method.cmp(&b.method))
    });
    endpoints.truncate(top);
    EndpointsResult { endpoints }
}

fn endpoint_stat(method: String, host: String, norm_path: String, entries: &[&Entry]) -> EndpointStat {
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut content_types: AHashSet<String> = AHashSet::new();
    let mut query_keys: AHashSet<String> = AHashSet::new();
    let mut error_count = 0usize;

    for e in entries {
        *statuses.entry(e.status.to_string()).or_default() += 1;
        if let Some(ct) = &e.content_type {
            content_types.insert(ct.clone());
        }
        for (k, _) in &e.query {
            query_keys.insert(k.clone());
        }
        if e.is_error() {
            error_count += 1;
        }
    }

    let mut content_types: Vec<String> = content_types.into_iter().collect();
    content_types.sort();
    let mut sample_query_keys: Vec<String> = query_keys.into_iter().collect();
    sample_query_keys.sort();

    EndpointStat {
        host,
        method,
        norm_path,
        count: entries.len(),
        statuses,
        content_types,
        sample_query_keys,
        error_count,
    }
}

/// Render the endpoint inventory as deterministic terminal text.
pub fn render_endpoints_text(r: &EndpointsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail endpoints ==\n");
    for e in &r.endpoints {
        let statuses: Vec<String> = e.statuses.iter().map(|(s, c)| format!("{s}:{c}")).collect();
        out.push_str(&format!(
            "\n{:>4}  {} {}{}\n",
            e.count, e.method, e.host, e.norm_path
        ));
        out.push_str(&format!("  status: {}\n", statuses.join(" ")));
        if !e.content_types.is_empty() {
            out.push_str(&format!("  content-types: {}\n", e.content_types.join(", ")));
        }
        if !e.sample_query_keys.is_empty() {
            out.push_str(&format!("  query keys: {}\n", e.sample_query_keys.join(", ")));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib endpoints 2>&1 | tail -8`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/endpoints.rs
git commit -m "feat: endpoints command analysis + renderer"
```

---

### Task 9: Wire the three commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, replace the existing `use har::analysis::summary::{compute_summary, render_summary_text};` line with the full set of analysis imports plus config:

```rust
use har::analysis::endpoints::{compute_endpoints, render_endpoints_text};
use har::analysis::hosts::{compute_hosts, render_hosts_text};
use har::analysis::subsystems::{compute_subsystems, render_subsystems_text};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::config::Config;
```

- [ ] **Step 2: Add the `--config` global option.** In the `Cli` struct, directly after the `filter` field, add:

```rust
    /// Path to a wiretrail.yaml config (default: ./wiretrail.yaml if present).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
```

- [ ] **Step 3: Add the new subcommand variants.** Replace the entire `enum Command { ... }` block with:

```rust
#[derive(Subcommand, Debug)]
enum Command {
    /// Executive summary of the capture (default).
    Summary,
    /// Per-host request/latency/byte/error breakdown.
    Hosts,
    /// Group hosts into named subsystems (vendor heuristics + config).
    Subsystems,
    /// Normalized endpoint inventory.
    Endpoints,
}
```

- [ ] **Step 4: Replace the `match cli.command.unwrap_or(Command::Summary) { ... }` block** with the multi-command dispatch. The new block:

```rust
    match cli.command.unwrap_or(Command::Summary) {
        Command::Summary => {
            let result = compute_summary(&cap, &filter, cli.top);
            let findings = result.error_count > 0 || !result.top_duplicates.is_empty();
            emit(
                cli.json,
                "summary",
                &cap.meta,
                &result,
                &render_summary_text(&result),
                &["duplicates", "errors", "slowest"],
            );
            exit(findings);
        }
        Command::Hosts => {
            let result = compute_hosts(&cap, &filter, cli.top);
            let findings = result.hosts.iter().any(|h| h.error_count > 0 || h.duplicate_count > 0);
            emit(
                cli.json,
                "hosts",
                &cap.meta,
                &result,
                &render_hosts_text(&result),
                &["subsystems", "endpoints", "errors"],
            );
            exit(findings);
        }
        Command::Subsystems => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_subsystems(&cap, &filter, &config, cli.top);
            let findings = result.subsystems.iter().any(|s| s.error_count > 0 || s.duplicate_count > 0);
            emit(
                cli.json,
                "subsystems",
                &cap.meta,
                &result,
                &render_subsystems_text(&result),
                &["hosts", "endpoints", "duplicates"],
            );
            exit(findings);
        }
        Command::Endpoints => {
            let result = compute_endpoints(&cap, &filter, cli.top);
            let findings = result.endpoints.iter().any(|e| e.error_count > 0);
            emit(
                cli.json,
                "endpoints",
                &cap.meta,
                &result,
                &render_endpoints_text(&result),
                &["errors", "duplicates", "show-entry"],
            );
            exit(findings);
        }
    }
```

- [ ] **Step 5: Add the `emit` and `exit` helpers and import `CaptureMeta`.** Add this import near the other `use har::...` lines:

```rust
use har::model::CaptureMeta;
```

Then add these two functions to the bottom of `src/main.rs` (after `fn main`):

```rust
/// Print a result either as the stable JSON envelope or as terminal text.
fn emit<T: serde::Serialize>(
    json: bool,
    command: &'static str,
    meta: &CaptureMeta,
    result: &T,
    text: &str,
    next: &[&str],
) {
    if json {
        let env = Envelope::new(command, meta.clone(), result)
            .with_next_commands(next.iter().map(|s| s.to_string()).collect());
        println!("{}", env.to_json());
    } else {
        print!("{text}");
        println!("\nnext useful commands: {}", next.join(" · "));
    }
}

/// Exit 1 when findings exceed threshold, else 0.
fn exit(findings: bool) -> ! {
    std::process::exit(if findings {
        ExitCode::Findings as i32
    } else {
        ExitCode::Clean as i32
    });
}
```

- [ ] **Step 6: Remove the now-duplicated inline summary rendering.** The original `Command::Summary` arm built its own `Envelope` inline; Step 4 already replaced it with the `emit`/`exit` form, so no leftover `Envelope::new(...)` call should remain in the match. Verify by building.

Run: `cargo build 2>&1 | tail -8`
Expected: SUCCESS. (If the compiler reports `Envelope` or `compute_summary` unused, that indicates a leftover arm — remove it.)

- [ ] **Step 7: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har hosts
cargo run --quiet -- tests/fixtures/someapi123.har subsystems
cargo run --quiet -- tests/fixtures/someapi123.har endpoints --json | head -15
```
Expected: `hosts` prints the `== wiretrail hosts ==` block for `api.someapi123.io`; `subsystems` prints `== wiretrail subsystems ==` (host name as subsystem, since no config/vendor match); `endpoints --json` prints an envelope with `"command": "endpoints"`.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire hosts/subsystems/endpoints commands + --config into CLI"
```

---

### Task 10: End-to-end binary tests

**Files:**
- Create: `tests/cli_inventory.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_inventory.rs`:

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
fn hosts_text_has_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "hosts"]);
    assert!(stdout.contains("== wiretrail hosts =="));
    assert!(stdout.contains("api.someapi123.io"));
}

#[test]
fn subsystems_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "subsystems", "--json"]);
    assert!(stdout.contains("\"command\": \"subsystems\""));
    assert!(stdout.contains("\"subsystems\""));
}

#[test]
fn endpoints_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "endpoints", "--json"]);
    assert!(stdout.contains("\"command\": \"endpoints\""));
    assert!(stdout.contains("\"endpoints\""));
}
```

- [ ] **Step 2: Run the new integration tests**

Run: `cargo test --test cli_inventory 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures (Plan 1's tests + the new unit and integration tests).

- [ ] **Step 4: Commit**

```bash
git add tests/cli_inventory.rs
git commit -m "test: end-to-end tests for hosts/subsystems/endpoints"
```

---

## Self-review

**Spec coverage (inventory & grouping slice):**
- `hosts` — count, methods, status distribution, p50/p95/max latency, bytes sent/received, time window, dup count → Task 6. ✓
- `subsystems` — vendor heuristics (Task 4) + YAML ownership map (Task 7 config) + raw-host fallback; counts, time windows, hosts, dup/error → Task 7. ✓
- `endpoints` — normalized path (reuses Plan 1 `norm_path`), observed statuses, content types, sample query keys → Task 8. ✓
- YAML config discovery (`wiretrail.yaml` / `--config`) → Task 7 (`Config::load`) + Task 9 (`--config` flag). ✓
- All three support `--json` (stable envelope), filter language, `--top`, `next_commands`, and findings-based exit codes → Task 9 (`emit`/`exit`). ✓
- Deferred to Plan 3/4: duplicates/retries/errors/redirects/slowest/transitions/timeline/show-entry, report, curl. Explicitly out of scope. ✓

**Placeholder scan:** No TBD/TODO; every code step contains complete code; every command step states expected output. ✓

**Type consistency:**
- `glob_match` (Task 2, `src/glob.rs`) consumed by `filter.rs` (Task 2) and `config.rs` (Task 7). Same signature `(&str, &str) -> bool`. ✓
- `percentiles(&[f64]) -> Percentiles` (Task 3) used in `hosts.rs` (Task 6). ✓
- `vendor_for(&str) -> Option<&'static str>` (Task 4) used in `config.rs` (Task 7). ✓
- `sample_entry(index, host, method, path, status)` / `sample_capture(Vec<Entry>)` (Task 5) used by Tasks 6, 7, 8 tests. Signatures match call sites. ✓
- `Config`, `Config::default()`, `Config::from_yaml_str`, `Config::load`, `Config::subsystem_for(&Entry) -> Subsystem` (Task 7) used by Task 7 subsystems + Task 9 main. ✓
- `compute_hosts(&Capture,&Filter,usize)`, `compute_subsystems(&Capture,&Filter,&Config,usize)`, `compute_endpoints(&Capture,&Filter,usize)` and their `render_*_text` fns (Tasks 6–8) called with matching arg order in Task 9. ✓
- `emit<T: Serialize>(bool, &'static str, &CaptureMeta, &T, &str, &[&str])` and `exit(bool) -> !` (Task 9) — `Envelope::new` accepts `&T` (T: Serialize), consistent with Plan 1 usage. ✓
- `Entry` fields referenced (`sizes.req_body`, `sizes.resp_content`, `sizes.resp_body`, `started_offset_ms`, `status`, `content_type`, `query`, `method`, `host`, `norm_path`, `path`) all exist in the Plan 1 `model.rs`. ✓
- `ResourceType`, `Phases`, `Sizes` used by `sample_entry` are imported via `model.rs`'s own `use crate::classify::ResourceType;` (already present) — `Phases`/`Sizes` are defined in `model.rs`. ✓
