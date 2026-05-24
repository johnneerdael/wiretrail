# wiretrail M7 — Regression & Rules (`compare`, `rules`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `compare <baseline.har>` (multi-HAR regression diff + severity scoring + `--fail-on` CI gate) and `rules` (configurable rule engine + built-in rule packs).

**Architecture:** Two composition-layer analysis modules over the existing normalized model. `compare` builds per-`(method,host,norm_path)` aggregates of both captures and severity-scores the deltas. `rules` evaluates a `Config.rules` list plus code-defined built-in packs against each entry. Both follow the established `compute_* → result + render_*_text` pattern; `main.rs` adds two clap subcommands.

**Tech Stack:** Rust 2024, serde/serde_json, clap, ahash. No new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-m5-m7-expansion-design.md`,
Phase M7. **Plan 3 of 3** for expansion-2 (M5 + M6 shipped). Builds the final 2
commands → **31 total**.

## Prerequisites (verified present)

- `model::{Capture, Entry}`; `Entry.{id,method,host,path,norm_path,status,duration_ms,query,req_headers,req_body,sizes}`; `Entry::status_class()` (returns 2/3/4/5/0); cfg(test) `sample_entry(index,host,method,path,status)` (sets `duration_ms=10.0`, `norm_path=path`, empty `query`/headers, `Sizes::default()` all-zero) + `sample_capture`.
- `filter::Filter` with `parse(&[String])` + `matches(&Entry)`.
- `config::Config` — `#[derive(Debug, Default, Deserialize)]`, has `ownership`/`required_headers`, `from_yaml_str`, `load(Option<&Path>)`. Pattern for an added `#[serde(default)]` field + matcher struct: `RequiredHeaderRule` (config.rs:15) and `compute_checks` (checks.rs).
- `glob::glob_match(pattern, text) -> bool` (case-insensitive, `*` = any run; `"api.*.com"`, `"*charge*"`, `"*/graphql"` all work).
- `opaque::is_opaque(&str) -> bool`.
- `stats::percentiles(&[f64]) -> Percentiles { p50, p95, max }`.
- `loader::load(&Path) -> Result<RawDoc, _>` + `assemble::assemble(RawDoc) -> Capture` (already used in main.rs:167-174 for the primary file).
- `render::{Envelope, ExitCode, human_ms, human_bytes}`; `main.rs` `emit`/`exit` helpers; `ExitCode::{Clean=0, Findings=1, InvalidHar=2}`.
- Endpoint-aggregate pattern: `endpoints.rs` keys on `(method.to_ascii_uppercase(), host, norm_path)`.
- `diagnose.rs` precedent for a private `sev_rank(&str) -> u8` (critical=3, high=2, medium=1, _=0) and findings sort.

## File structure

```
src/config.rs            # Modify: add Rule struct + rules: Vec<Rule> field
src/analysis/mod.rs      # Modify: declare compare, rules
src/analysis/rules.rs    # NEW: rule engine + built-in packs
src/analysis/compare.rs  # NEW: regression diff + scoring
src/main.rs              # Modify: 2 subcommands + SeverityArg enum + baseline load
tests/cli_compare.rs     # NEW: integration tests
```

---

### Task 1: Scaffold — `Rule` struct, config field, module declarations

**Files:**
- Modify: `src/config.rs`, `src/analysis/mod.rs`

- [ ] **Step 1: Add the `Rule` struct.** In `src/config.rs`, immediately after the `RequiredHeaderRule` struct (ends at line 22, before `OwnershipRule`), add:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Rule {
    /// Human-readable rule name (shown in findings).
    pub name: String,
    /// Host glob the rule applies to (None = any host).
    #[serde(default)]
    pub host: Option<String>,
    /// Path glob the rule applies to (None = any path).
    #[serde(default)]
    pub path: Option<String>,
    /// HTTP method glob (None = any method).
    #[serde(default)]
    pub method: Option<String>,
    /// Status glob, matched against the stringified status (e.g. "2*", "404").
    #[serde(default)]
    pub status: Option<String>,
    /// Header names that must be present on matching requests.
    #[serde(default)]
    pub require_headers: Vec<String>,
    /// Maximum allowed request duration in milliseconds.
    #[serde(default)]
    pub max_latency_ms: Option<f64>,
    /// If true, any matching request is itself a violation.
    #[serde(default)]
    pub forbid: bool,
}
```

- [ ] **Step 2: Add the `rules` field to `Config`.** In `src/config.rs`, change the `Config` struct (lines 7-13) to:

```rust
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ownership: Vec<OwnershipRule>,
    #[serde(default)]
    pub required_headers: Vec<RequiredHeaderRule>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}
```

- [ ] **Step 3: Add a config parse test.** In `src/config.rs`, inside `mod tests`, after `required_headers_defaults_empty` (ends line 200), add:

```rust
    #[test]
    fn parses_rules_from_yaml() {
        let yaml = r#"
rules:
  - name: "API needs auth"
    host: "api.*"
    require_headers: ["Authorization"]
    max_latency_ms: 2000
  - name: "no internal hosts"
    host: "*.internal"
    forbid: true
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.rules.len(), 2);
        assert_eq!(cfg.rules[0].require_headers, vec!["Authorization"]);
        assert_eq!(cfg.rules[0].max_latency_ms, Some(2000.0));
        assert!(cfg.rules[1].forbid);
    }
```

- [ ] **Step 4: Declare the analysis modules.** In `src/analysis/mod.rs`, add `pub mod compare;` between `pub mod checks;` and `pub mod curl;`, and `pub mod rules;` between `pub mod retries;` and `pub mod search;`.

- [ ] **Step 5: Create stub files and verify the config test.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/analysis/compare.rs src/analysis/rules.rs
CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo test --lib config::tests::parses_rules_from_yaml 2>&1 | tail -6
```
Expected: PASS (1 test). Empty `compare.rs`/`rules.rs` compile fine as empty modules.

> **Note:** if `cargo build`/`test` reports `Finished` in ~0s without `Compiling
> wiretrail` after a source edit (a stale-fingerprint quirk from the global cache),
> force it with `cargo clean -p wiretrail && cargo build`.

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/analysis/mod.rs src/analysis/compare.rs src/analysis/rules.rs
git commit -m "chore: scaffold M7 — Rule config struct + compare/rules module decls"
```

---

### Task 2: `rules` engine + built-in packs

**Files:**
- Create: `src/analysis/rules.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/rules.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_rules;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn no_filter() -> Filter {
        Filter::parse(&[]).unwrap()
    }

    #[test]
    fn config_rule_require_header_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: needs-auth\n    host: \"api.x\"\n    require_headers: [\"Authorization\"]\n",
        )
        .unwrap();
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(r.findings.iter().any(|f| f.rule == "needs-auth"
            && f.severity == "high"
            && f.detail.contains("Authorization")));
    }

    #[test]
    fn config_rule_max_latency_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: too-slow\n    host: \"api.x\"\n    max_latency_ms: 5\n",
        )
        .unwrap();
        // sample_entry sets duration_ms = 10.0 > 5
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(r.findings.iter().any(|f| f.rule == "too-slow" && f.severity == "medium"));
    }

    #[test]
    fn config_rule_forbid_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: no-staging\n    host: \"*.staging\"\n    forbid: true\n",
        )
        .unwrap();
        let cap = sample_capture(vec![sample_entry(0, "api.staging", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(r.findings.iter().any(|f| f.rule == "no-staging" && f.severity == "high"));
    }

    #[test]
    fn auth_pack_flags_missing_authorization() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &Config::default(), &["auth".to_string()], 50);
        assert!(r.findings.iter().any(|f| f.detail.contains("Authorization")));
    }

    #[test]
    fn security_pack_flags_opaque_query_secret() {
        let mut e: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        e.query = vec![(
            "token".into(),
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ".into(),
        )];
        let r = compute_rules(
            &sample_capture(vec![e]),
            &no_filter(),
            &Config::default(),
            &["security".to_string()],
            50,
        );
        assert!(r.findings.iter().any(|f| f.severity == "high" && f.detail.contains("token")));
    }

    #[test]
    fn present_header_not_flagged() {
        let mut e = sample_entry(0, "api.x", "GET", "/a", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        let r = compute_rules(
            &sample_capture(vec![e]),
            &no_filter(),
            &Config::default(),
            &["auth".to_string()],
            50,
        );
        assert!(r.findings.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::rules 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_rules`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/rules.rs`:

```rust
use crate::config::{Config, Rule};
use crate::filter::Filter;
use crate::glob::glob_match;
use crate::model::{Capture, Entry};
use crate::opaque::is_opaque;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RulesResult {
    pub findings: Vec<RuleFinding>,
}

#[derive(Debug, Serialize)]
pub struct RuleFinding {
    pub rule: String,
    pub severity: String,
    pub detail: String,
    pub entry_ids: Vec<String>,
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

fn matcher_opt(pat: &Option<String>, text: &str) -> bool {
    match pat {
        Some(p) => glob_match(p, text),
        None => true,
    }
}

fn rule_matches(rule: &Rule, e: &Entry) -> bool {
    matcher_opt(&rule.host, &e.host)
        && matcher_opt(&rule.path, &e.path)
        && matcher_opt(&rule.method, &e.method)
        && matcher_opt(&rule.status, &e.status.to_string())
}

fn has_header(e: &Entry, name: &str) -> bool {
    e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
}

/// Evaluate one generic rule against an entry: `(rule_name, severity, detail)` tuples.
fn eval_rule(rule: &Rule, e: &Entry) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if !rule_matches(rule, e) {
        return out;
    }
    if rule.forbid {
        out.push((rule.name.clone(), "high".into(), "matched a forbidden rule".into()));
        return out;
    }
    for h in &rule.require_headers {
        if !has_header(e, h) {
            out.push((
                rule.name.clone(),
                "high".into(),
                format!("missing required header: {h}"),
            ));
        }
    }
    if let Some(budget) = rule.max_latency_ms
        && e.duration_ms > budget
    {
        out.push((
            rule.name.clone(),
            "medium".into(),
            format!("latency {:.0}ms exceeds budget {budget:.0}ms", e.duration_ms),
        ));
    }
    out
}

/// Built-in rule packs expressible with generic `Rule` fields.
fn pack_rules(pack: &str) -> Vec<Rule> {
    match pack {
        "auth" => vec![Rule {
            name: "auth: Authorization required".into(),
            require_headers: vec!["Authorization".into()],
            ..Rule::default()
        }],
        "caching" => vec![Rule {
            name: "caching: GET 200 needs Cache-Control".into(),
            method: Some("GET".into()),
            status: Some("200".into()),
            require_headers: vec!["Cache-Control".into()],
            ..Rule::default()
        }],
        "payments" => vec![
            Rule {
                name: "payments: idempotency key on charges".into(),
                path: Some("*charge*".into()),
                require_headers: vec!["Idempotency-Key".into()],
                ..Rule::default()
            },
            Rule {
                name: "payments: idempotency key on payments".into(),
                path: Some("*payment*".into()),
                require_headers: vec!["Idempotency-Key".into()],
                ..Rule::default()
            },
        ],
        _ => vec![],
    }
}

fn is_special_pack(pack: &str) -> bool {
    matches!(pack, "security" | "rest" | "graphql")
}

/// Packs that need a custom predicate (not expressible via `Rule` fields).
fn eval_special(pack: &str, e: &Entry) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    match pack {
        "security" => {
            for (k, v) in &e.query {
                if is_opaque(v) {
                    out.push((
                        "security: no secrets in query".into(),
                        "high".into(),
                        format!("opaque secret in query param `{k}`"),
                    ));
                }
            }
        }
        "rest" => {
            if e.method.eq_ignore_ascii_case("GET")
                && e.req_body.as_deref().is_some_and(|b| !b.is_empty())
            {
                out.push((
                    "rest: no mutation over GET".into(),
                    "medium".into(),
                    "GET request carries a body".into(),
                ));
            }
        }
        "graphql" => {
            if e.method.eq_ignore_ascii_case("POST")
                && glob_match("*/graphql", &e.path)
                && !e.req_body.as_deref().unwrap_or("").contains("operationName")
            {
                out.push((
                    "graphql: operationName required".into(),
                    "low".into(),
                    "GraphQL POST without operationName".into(),
                ));
            }
        }
        _ => {}
    }
    out
}

/// Evaluate config rules + built-in packs against the filtered capture.
pub fn compute_rules(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    packs: &[String],
    top: usize,
) -> RulesResult {
    let mut rules: Vec<Rule> = config.rules.clone();
    for p in packs {
        rules.extend(pack_rules(p));
    }

    // key = (rule, severity, detail) -> entry ids
    let mut map: AHashMap<(String, String, String), Vec<String>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        for rule in &rules {
            for (name, sev, detail) in eval_rule(rule, e) {
                map.entry((name, sev, detail)).or_default().push(e.id.clone());
            }
        }
        for p in packs {
            if is_special_pack(p) {
                for (name, sev, detail) in eval_special(p, e) {
                    map.entry((name, sev, detail)).or_default().push(e.id.clone());
                }
            }
        }
    }

    let mut findings: Vec<RuleFinding> = map
        .into_iter()
        .map(|((rule, severity, detail), entry_ids)| RuleFinding {
            rule,
            severity,
            detail,
            entry_ids,
        })
        .collect();
    findings.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.entry_ids.len().cmp(&a.entry_ids.len()))
            .then(a.rule.cmp(&b.rule))
            .then(a.detail.cmp(&b.detail))
    });
    findings.truncate(top);
    RulesResult { findings }
}

/// Render rule findings as deterministic terminal text.
pub fn render_rules_text(r: &RulesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail rules ==\n");
    for f in &r.findings {
        out.push_str(&format!(
            "\n[{}] {}\n  {} ({} entries)\n",
            f.severity,
            f.rule,
            f.detail,
            f.entry_ids.len()
        ));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::rules 2>&1 | tail -10`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/rules.rs
git commit -m "feat: rules engine (config rules + auth/caching/payments/security/rest/graphql packs)"
```

---

### Task 3: `compare` regression diff + scoring

**Files:**
- Create: `src/analysis/compare.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/compare.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_compare;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn no_filter() -> Filter {
        Filter::parse(&[]).unwrap()
    }

    #[test]
    fn detects_new_host() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.y", "GET", "/b", 200),
        ]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert!(r.new_hosts.contains(&"api.y".to_string()));
        assert!(!r.new_hosts.contains(&"api.x".to_string()));
    }

    #[test]
    fn detects_new_5xx_as_high() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 500)]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert!(
            r.new_errors
                .iter()
                .any(|d| d.status == 500 && d.severity == "high")
        );
        assert_eq!(r.max_severity, "high");
    }

    #[test]
    fn detects_latency_regression() {
        let mut b = sample_entry(0, "api.x", "GET", "/a", 200);
        b.duration_ms = 100.0;
        let mut n = sample_entry(0, "api.x", "GET", "/a", 200);
        n.duration_ms = 900.0; // > 2x and > 200ms over baseline
        let r = compute_compare(
            &sample_capture(vec![n]),
            &sample_capture(vec![b]),
            &no_filter(),
            50,
        );
        assert_eq!(r.latency_regressions.len(), 1);
        assert_eq!(r.latency_regressions[0].severity, "medium");
    }

    #[test]
    fn detects_payload_growth() {
        let mut b: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        b.sizes.resp_content = 100;
        let mut n: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        n.sizes.resp_content = 500; // > 2x
        let r = compute_compare(
            &sample_capture(vec![n]),
            &sample_capture(vec![b]),
            &no_filter(),
            50,
        );
        assert_eq!(r.payload_growth.len(), 1);
        assert_eq!(r.payload_growth[0].severity, "low");
    }

    #[test]
    fn no_findings_is_none() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert_eq!(r.max_severity, "none");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::compare 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_compare`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/compare.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::stats::percentiles;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub new_hosts: Vec<String>,
    pub removed_hosts: Vec<String>,
    pub new_endpoints: Vec<String>,
    pub removed_endpoints: Vec<String>,
    pub new_errors: Vec<EndpointDelta>,
    pub latency_regressions: Vec<LatencyDelta>,
    pub payload_growth: Vec<SizeDelta>,
    pub max_severity: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointDelta {
    pub endpoint: String,
    pub status: i64,
    pub count: usize,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct LatencyDelta {
    pub endpoint: String,
    pub base_p50_ms: f64,
    pub new_p50_ms: f64,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct SizeDelta {
    pub endpoint: String,
    pub base_bytes: i64,
    pub new_bytes: i64,
    pub severity: String,
}

/// Severity ordering shared with the CLI `--fail-on` gate.
pub fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

#[derive(Default)]
struct Agg {
    durations: Vec<f64>,
    bytes: Vec<f64>,
    error_statuses: Vec<i64>,
}

fn endpoint_key(e: &Entry) -> String {
    format!("{} {}{}", e.method.to_ascii_uppercase(), e.host, e.norm_path)
}

fn aggregate(cap: &Capture, filter: &Filter) -> (AHashSet<String>, AHashMap<String, Agg>) {
    let mut hosts = AHashSet::new();
    let mut map: AHashMap<String, Agg> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        hosts.insert(e.host.clone());
        let a = map.entry(endpoint_key(e)).or_default();
        a.durations.push(e.duration_ms);
        a.bytes.push(e.sizes.resp_content.max(e.sizes.resp_body).max(0) as f64);
        let cls = e.status_class();
        if cls == 4 || cls == 5 {
            a.error_statuses.push(e.status);
        }
    }
    (hosts, map)
}

/// Diff a new capture against a baseline; severity-score the regressions.
pub fn compute_compare(
    new: &Capture,
    base: &Capture,
    filter: &Filter,
    top: usize,
) -> CompareResult {
    let (new_hosts_set, new_map) = aggregate(new, filter);
    let (base_hosts_set, base_map) = aggregate(base, filter);

    let mut new_hosts: Vec<String> = new_hosts_set.difference(&base_hosts_set).cloned().collect();
    let mut removed_hosts: Vec<String> =
        base_hosts_set.difference(&new_hosts_set).cloned().collect();

    let new_keys: AHashSet<&String> = new_map.keys().collect();
    let base_keys: AHashSet<&String> = base_map.keys().collect();
    let mut new_endpoints: Vec<String> =
        new_keys.difference(&base_keys).map(|s| (*s).clone()).collect();
    let mut removed_endpoints: Vec<String> =
        base_keys.difference(&new_keys).map(|s| (*s).clone()).collect();

    let mut new_errors = Vec::new();
    let mut latency_regressions = Vec::new();
    let mut payload_growth = Vec::new();

    for (ep, a) in &new_map {
        // new errors: 4xx/5xx present in new but not in baseline for this endpoint
        if !a.error_statuses.is_empty() {
            let base_had = base_map
                .get(ep)
                .map(|b| !b.error_statuses.is_empty())
                .unwrap_or(false);
            if !base_had {
                let worst = *a.error_statuses.iter().max().unwrap();
                let severity = if worst / 100 == 5 { "high" } else { "medium" };
                new_errors.push(EndpointDelta {
                    endpoint: ep.clone(),
                    status: worst,
                    count: a.error_statuses.len(),
                    severity: severity.into(),
                });
            }
        }

        if let Some(b) = base_map.get(ep) {
            let np = percentiles(&a.durations).p50;
            let bp = percentiles(&b.durations).p50;
            if bp > 0.0 && np > bp * 2.0 && (np - bp) > 200.0 {
                latency_regressions.push(LatencyDelta {
                    endpoint: ep.clone(),
                    base_p50_ms: bp,
                    new_p50_ms: np,
                    severity: "medium".into(),
                });
            }

            let nb = percentiles(&a.bytes).p50;
            let bb = percentiles(&b.bytes).p50;
            if bb > 0.0 && nb > bb * 2.0 {
                payload_growth.push(SizeDelta {
                    endpoint: ep.clone(),
                    base_bytes: bb as i64,
                    new_bytes: nb as i64,
                    severity: "low".into(),
                });
            }
        }
    }

    new_hosts.sort();
    removed_hosts.sort();
    new_endpoints.sort();
    removed_endpoints.sort();
    new_errors.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.count.cmp(&a.count))
            .then(a.endpoint.cmp(&b.endpoint))
    });
    latency_regressions.sort_by(|a, b| {
        (b.new_p50_ms - b.base_p50_ms)
            .partial_cmp(&(a.new_p50_ms - a.base_p50_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.endpoint.cmp(&b.endpoint))
    });
    payload_growth.sort_by(|a, b| {
        (b.new_bytes - b.base_bytes)
            .cmp(&(a.new_bytes - a.base_bytes))
            .then(a.endpoint.cmp(&b.endpoint))
    });

    new_hosts.truncate(top);
    removed_hosts.truncate(top);
    new_endpoints.truncate(top);
    removed_endpoints.truncate(top);
    new_errors.truncate(top);
    latency_regressions.truncate(top);
    payload_growth.truncate(top);

    let mut rank = 0u8;
    for s in new_errors
        .iter()
        .map(|d| d.severity.as_str())
        .chain(latency_regressions.iter().map(|d| d.severity.as_str()))
        .chain(payload_growth.iter().map(|d| d.severity.as_str()))
    {
        rank = rank.max(sev_rank(s));
    }
    let any = !new_errors.is_empty()
        || !latency_regressions.is_empty()
        || !payload_growth.is_empty();
    let max_severity = match rank {
        3 => "critical",
        2 => "high",
        1 => "medium",
        _ if any => "low",
        _ => "none",
    }
    .to_string();

    CompareResult {
        new_hosts,
        removed_hosts,
        new_endpoints,
        removed_endpoints,
        new_errors,
        latency_regressions,
        payload_growth,
        max_severity,
    }
}

/// Render the comparison as deterministic terminal text.
pub fn render_compare_text(r: &CompareResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail compare ==\n");
    out.push_str(&format!("max severity: {}\n", r.max_severity));
    if !r.new_hosts.is_empty() {
        out.push_str(&format!("new hosts: {}\n", r.new_hosts.join(", ")));
    }
    if !r.removed_hosts.is_empty() {
        out.push_str(&format!("removed hosts: {}\n", r.removed_hosts.join(", ")));
    }
    if !r.new_endpoints.is_empty() {
        out.push_str(&format!("new endpoints: {}\n", r.new_endpoints.len()));
    }
    if !r.removed_endpoints.is_empty() {
        out.push_str(&format!("removed endpoints: {}\n", r.removed_endpoints.len()));
    }
    if !r.new_errors.is_empty() {
        out.push_str("\nnew errors:\n");
        for d in &r.new_errors {
            out.push_str(&format!(
                "  [{}] {} -> {} ({}x)\n",
                d.severity, d.endpoint, d.status, d.count
            ));
        }
    }
    if !r.latency_regressions.is_empty() {
        out.push_str("\nlatency regressions:\n");
        for d in &r.latency_regressions {
            out.push_str(&format!(
                "  [{}] {} p50 {:.0}ms -> {:.0}ms\n",
                d.severity, d.endpoint, d.base_p50_ms, d.new_p50_ms
            ));
        }
    }
    if !r.payload_growth.is_empty() {
        out.push_str("\npayload growth:\n");
        for d in &r.payload_growth {
            out.push_str(&format!(
                "  [{}] {} {}B -> {}B\n",
                d.severity, d.endpoint, d.base_bytes, d.new_bytes
            ));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::compare 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/compare.rs
git commit -m "feat: compare command (multi-HAR regression diff + severity scoring)"
```

---

### Task 4: Wire `compare` and `rules` into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, add these `use` lines alongside the other `har::analysis` imports (keep alphabetical-ish; exact placement is not load-bearing):

```rust
use har::analysis::compare::{compute_compare, render_compare_text, sev_rank};
use har::analysis::rules::{compute_rules, render_rules_text};
```

(The clap import already became `use clap::{Parser, Subcommand, ValueEnum};` in M6 — no change needed.)

- [ ] **Step 2: Add the severity value enum.** In `src/main.rs`, next to the `TargetArg`/`ExportFormatArg` enums (added in M6, directly above `enum Command`), add:

```rust
#[derive(Debug, Clone, Copy, ValueEnum)]
enum SeverityArg {
    Critical,
    High,
    Medium,
    Low,
}

impl SeverityArg {
    fn as_str(self) -> &'static str {
        match self {
            SeverityArg::Critical => "critical",
            SeverityArg::High => "high",
            SeverityArg::Medium => "medium",
            SeverityArg::Low => "low",
        }
    }
}
```

- [ ] **Step 3: Add the subcommand variants.** Inside `enum Command { ... }`, after the `Export { ... }` variant (the last one, added in M6), add:

```rust
    /// Compare this capture against a baseline HAR (regression diff).
    Compare {
        /// Path to the baseline HAR to diff against.
        baseline: PathBuf,
        /// Exit non-zero only when max severity reaches this level (CI gate).
        #[arg(long = "fail-on", value_enum)]
        fail_on: Option<SeverityArg>,
    },
    /// Evaluate config rules and built-in rule packs against the capture.
    Rules {
        /// Built-in packs to apply, e.g. `--pack auth,security`.
        #[arg(long = "pack", value_delimiter = ',')]
        pack: Vec<String>,
    },
```

- [ ] **Step 4: Add the dispatch arms.** Inside the `match` block, after the `Command::Export { ... }` arm (the last one, added in M6), add:

```rust
        Command::Compare { baseline, fail_on } => {
            let base_doc = match load(&baseline) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("wiretrail: baseline: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let base = assemble(base_doc);
            let result = compute_compare(&cap, &base, &filter, cli.top);
            emit(
                cli.json,
                "compare",
                &cap.meta,
                &result,
                &render_compare_text(&result),
                &["diagnose", "errors", "slowest"],
            );
            let any = result.max_severity != "none";
            let findings = match fail_on {
                Some(t) => any && sev_rank(&result.max_severity) >= sev_rank(t.as_str()),
                None => any,
            };
            exit(findings);
        }
        Command::Rules { pack } => {
            let config = match Config::load(cli.config.as_deref()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("wiretrail: {e}");
                    std::process::exit(ExitCode::InvalidHar as i32);
                }
            };
            let result = compute_rules(&cap, &filter, &config, &pack, cli.top);
            let findings = !result.findings.is_empty();
            emit(
                cli.json,
                "rules",
                &cap.meta,
                &result,
                &render_rules_text(&result),
                &["checks", "errors", "diagnose"],
            );
            exit(findings);
        }
```

- [ ] **Step 5: Build**

Run: `CARGO_CACHE_AUTO_CLEAN_FREQUENCY=never cargo build 2>&1 | tail -8` (if `Finished` in ~0s without `Compiling`, run `cargo clean -p wiretrail && cargo build`).
Expected: SUCCESS.

- [ ] **Step 6: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har rules --pack auth,security --json | head -6
cargo run --quiet -- tests/fixtures/someapi123.har compare tests/fixtures/someapi13.har --json | head -8
cargo run --quiet -- tests/fixtures/someapi123.har compare tests/fixtures/someapi123.har
echo "exit: $?"
```
Expected: `rules --json` prints an envelope with `"command": "rules"`; `compare --json`
prints `"command": "compare"` with a `max_severity`; comparing a capture against itself
prints `max severity: none` and exits `0`.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire compare (--fail-on CI gate) and rules (--pack) into CLI"
```

---

### Task 5: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_compare.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_compare.rs`:

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
fn rules_json_envelope() {
    let (stdout, _) = run(&[
        &fixture("someapi123.har"),
        "rules",
        "--pack",
        "auth",
        "--json",
    ]);
    assert!(stdout.contains("\"command\": \"rules\""));
    assert!(stdout.contains("\"findings\""));
}

#[test]
fn compare_json_envelope() {
    let (stdout, _) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi13.har"),
        "--json",
    ]);
    assert!(stdout.contains("\"command\": \"compare\""));
    assert!(stdout.contains("\"max_severity\""));
}

#[test]
fn compare_against_self_is_clean() {
    let (stdout, code) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi123.har"),
    ]);
    assert!(stdout.contains("max severity: none"));
    assert_eq!(code, 0);
}

#[test]
fn fail_on_high_gates_exit_code() {
    // Comparing a capture to itself yields no findings, so even --fail-on low exits 0.
    let (_stdout, code) = run(&[
        &fixture("someapi123.har"),
        "compare",
        &fixture("someapi123.har"),
        "--fail-on",
        "low",
    ]);
    assert_eq!(code, 0);
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_compare 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result: FAILED" || echo "all green"`
Expected: `all green`.

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
B="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_19-24.har"
./target/release/wiretrail "$A" rules --pack auth,security,caching 2>/dev/null | head -12
echo "=== compare 15-24 (new) vs 19-24 (baseline) ==="
./target/release/wiretrail "$A" compare "$B" 2>/dev/null | head -20
echo "=== fail-on high exit code ==="
./target/release/wiretrail "$A" compare "$B" --fail-on high >/dev/null 2>&1; echo "exit: $?"
```
Expected: `rules` surfaces real findings (missing-auth / opaque-query-secret / missing
Cache-Control) without printing the actual secret values (detail names the param/header,
not the value); `compare` prints real host/endpoint deltas + a `max severity`; the
`--fail-on high` invocation exits `1` only if a high-severity regression exists, else `0`.

- [ ] **Step 6: Commit**

```bash
git add tests/cli_compare.rs
git commit -m "test: end-to-end tests for compare/rules"
```

---

## Self-review

**Spec coverage (Phase M7):**
- `compare <baseline.har>` (#57 multi-HAR diff + #59 scoring + #112 CI gate) — positional baseline, `CompareResult` with new/removed hosts+endpoints, new_errors, latency_regressions, payload_growth, max_severity; severity scoring (new 5xx→high, 4xx→medium, p50 >2× and >200ms→medium, payload >2×→low); `--fail-on` gate → Tasks 3, 4. ✓
- `rules` (#60 engine + #61 packs) — `Config.rules` list (Task 1) + `compute_rules` with matchers/require_headers/max_latency/forbid + built-in packs `auth`/`caching`/`payments` (generic) and `security`/`rest`/`graphql` (special predicates) via `--pack` → Tasks 2, 4. ✓
- Config extension `pub rules: Vec<Rule>` (additive `#[serde(default)]`) → Task 1. ✓
- Cross-cutting: `--json` envelope, filter, `--top`, `next_commands` on both; `rules` uses findings-based exit; `compare` adds `--fail-on`; redact-by-default (rules name params/headers not values; compare emits only aggregates) → Task 4. ✓
- No new crate (uses existing ahash/stats/glob/opaque) — consistent with spec ("one new crate: regex" was M6). ✓
- `compare` composes the existing model + `stats::percentiles` (no re-derivation of analyses); `rules` is a fresh per-entry evaluator mirroring `checks`. ✓

**Placeholder scan:** No TBD/TODO. Every code step is complete with full bodies; every command has its expected output stated. All six packs named in the spec are implemented (three generic in `pack_rules`, three predicate-based in `eval_special`). ✓

**Type consistency:**
- `Rule { name, host, path, method, status, require_headers, max_latency_ms, forbid }` defined in config.rs (Task 1), consumed by `rules.rs` `eval_rule`/`pack_rules`/`rule_matches` and cloned from `config.rules` in `compute_rules` (Task 2). `Rule` derives `Clone + Default + Deserialize` — required by `config.rules.clone()` and `..Rule::default()`. ✓
- `compute_rules(&Capture,&Filter,&Config,&[String],usize) -> RulesResult` + `render_rules_text` (Task 2) match the Task 4 dispatch call (`&pack` is `&Vec<String>` coercing to `&[String]`). ✓
- `compute_compare(&Capture,&Capture,&Filter,usize) -> CompareResult` + `render_compare_text` + `pub fn sev_rank` (Task 3) match the Task 4 dispatch + `--fail-on` comparison. ✓
- `RulesResult.findings` / `CompareResult.max_severity` field names used identically in tests, render fns, and main dispatch. ✓
- Reused signatures verified against source: `Filter::parse(&[])`/`matches`, `glob_match(&str,&str)`, `is_opaque(&str)`, `percentiles(&[f64]).p50`, `Entry::status_class()`, `load(&Path)`+`assemble(RawDoc)`, `Config::load(Option<&Path>)`, `emit`/`exit`/`ExitCode`. ✓
- `sev_rank` intentionally duplicated as a private fn in `rules.rs` and a `pub fn` in `compare.rs` (compare's is reused by main for `--fail-on`), mirroring `diagnose.rs`'s existing private `sev_rank` — consistent with the established codebase pattern, not a naming bug. ✓
