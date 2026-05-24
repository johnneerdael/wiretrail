# wiretrail — Expansion 2 (M5+M6+M7) Design

Date: 2026-05-24
Status: Approved (brainstorming complete; ready for implementation planning)
Author: John Neerdael

## Summary

wiretrail v0.1.0 ships 22 commands covering roughly features 1–35 + 44/48/64–67/82
of the ranked 120-feature wishlist. This spec defines the next 10 high-value
features as **9 new commands** across three coherent milestones, implemented as
three separate plans:

- **M5 — Diagnosis & startup:** `diagnose`, `startup`, `cascade`, `validate`.
- **M6 — Extraction & data-out:** `search`, `extract`, `export`.
- **M7 — Regression & rules:** `compare`, `rules` (regression scoring + CI gate fold
  into `compare`).

The diagnosis and regression commands are **composition layers** over the existing
`compute_*` analysis functions — they score and synthesize, they do not re-derive.

## Locked decisions (from brainstorming)

1. One spec, three phases (M5/M6/M7), three plans. Builds 9 commands → 31 total.
2. The 10 features as proposed (no security/perf swap-in).
3. `search` adds the **`regex`** crate; JSON-path in `extract` is hand-rolled;
   SQLite export is deferred (avoid the rusqlite C dependency — NDJSON/CSV cover
   jq/DuckDB/spreadsheet consumers).
4. `compare` takes the baseline HAR as a positional arg; `--fail-on <severity>`
   provides the CI gate.

## Architecture (unchanged)

Each command is one module under `src/analysis/` exposing
`compute_<cmd>(...) -> <Cmd>Result` (serde `Serialize`) + `render_<cmd>_text`.
`main.rs` adds a clap subcommand per command, dispatched through the existing
`emit`/`exit` helpers. All honor `--json`, the filter language, `--top`, and
redact-by-default; `search`/`extract`/`export` honor `--unsafe-include-secrets`.

Reused building blocks: `model::{Capture, Entry}`, `loader`/`assemble`,
`filter::Filter`, `redact::{redact_url, redact_value, redact_body, REDACTED}`,
`opaque::is_opaque`, `grouping`, `fingerprint`, `stats::percentiles`, `config::Config`,
`render::{Envelope, ExitCode, human_ms, human_bytes}`, and every existing
`analysis::*::compute_*` function.

## New files

```
src/analysis/diagnose.rs    src/analysis/startup.rs    src/analysis/cascade.rs
src/analysis/validate.rs    src/analysis/search.rs     src/analysis/extract.rs
src/analysis/export.rs      src/analysis/compare.rs     src/analysis/rules.rs
src/jsonpath.rs             # hand-rolled minimal JSON path evaluator (support)
```
Modified: `src/analysis/mod.rs`, `src/lib.rs` (`pub mod jsonpath`), `src/config.rs`
(`rules` field), `src/main.rs`, `Cargo.toml` (add `regex`).

---

## Phase M5 — Diagnosis & startup

### `diagnose` (#62 root-cause synthesizer)
Runs the existing analyses and emits ranked findings. Pure composition.

```rust
pub struct DiagnoseResult { pub findings: Vec<Diagnosis> }
pub struct Diagnosis {
    pub severity: String,       // "critical" | "high" | "medium" | "low"
    pub kind: String,           // e.g. "5xx-cluster", "token-refresh-race", "request-storm"
    pub title: String,          // one-line human summary
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub suggested_command: String,
}
pub fn compute_diagnose(cap: &Capture, filter: &Filter, config: &Config, top: usize) -> DiagnoseResult
```

Severity mapping (heuristic, deterministic):
- `compute_errors`: a ≥3-member 5xx group → `high` (`5xx-cluster`); 4xx group → `medium`.
- `compute_auth`: `old_token_reused` or failed refresh → `high` (`token-refresh-race`);
  401/403 group → `medium`.
- `compute_rate_limit`: `cooldown_violated` → `high` (`rate-limit-no-backoff`).
- `compute_retries`: a group with retry_count ≥3 ending non-2xx → `high`.
- `compute_storms`: peak ≥ 2× `min_count` → `medium` (`request-storm`).
- `compute_duplicates`: a non-retry group with count ≥10 → `medium` (`wasteful-duplicates`).
- `compute_redirects`: a storm group → `low`. `compute_slowest`: top call dominated
  by `server wait/TTFB` and > 1s → `low` (`slow-backend`).

Sort by severity (critical>high>medium>low) then evidence count desc. Findings if any.

### `startup` (#85 boot profile, folds in #24 critical path)
Sweep-line over each entry's `[started_offset_ms, started_offset_ms + duration_ms]`.

```rust
pub struct StartupResult {
    pub window_ms: f64,            // first start -> last response within --window-ms boot window
    pub requests_in_window: usize,
    pub max_concurrency: usize,    // max overlapping intervals
    pub critical_path_ms: f64,     // sum of the longest non-overlapping sequential chain
    pub critical_path: Vec<StartupCall>,   // the chain (bounded by --top)
    pub slowest: Vec<StartupCall>,         // top-N slowest in the window
}
pub struct StartupCall { pub id, pub method, pub host, pub norm_path, pub offset_ms, pub duration_ms, pub status }
pub fn compute_startup(cap: &Capture, filter: &Filter, window_ms: u64, top: usize) -> StartupResult
```

Boot window default `--window-ms 30000` (first 30 s of the capture, relative to the
earliest entry); `0` means "whole capture". Max concurrency via sorted start/end
events. Critical path = greedy longest chain of calls where each starts at/after the
previous one ends (an approximation of the sequential blocking spine).

### `cascade` (#83 first-failure + #84 cascade)
```rust
pub struct CascadeResult {
    pub first_failure: Option<FailureContext>,   // earliest is_error + neighbors
    pub cascades: Vec<Cascade>,
}
pub struct FailureContext { pub id, pub status, pub host, pub norm_path, pub before_ids: Vec<String>, pub after_ids: Vec<String> }
pub struct Cascade { pub trigger_id, pub trigger_kind: String, pub downstream_failures: usize, pub downstream_ids: Vec<String> }
pub fn compute_cascade(cap: &Capture, filter: &Filter, window_ms: u64, min_downstream: usize, top: usize) -> CascadeResult
```

`first_failure`: earliest `is_error()` by offset, with the 3 entries before/after.
A `Cascade`: a failed entry followed by ≥`--min-downstream` (default 3) failures
within `--window-ms` (default 5000); `trigger_kind` flags `config`/`auth`/`bootstrap`/
`init`/`token` paths as likely root triggers. Findings if any cascade or first failure.

### `validate` (#52/#54/#55 capture quality + sufficiency)
```rust
pub struct ValidateResult {
    pub har_version: String, pub creator: String, pub entry_count: usize,
    pub pct_with_timings: f64, pub pct_with_resp_body: f64,
    pub pct_post_with_req_body: f64, pub with_auth: bool, pub with_cookies: bool,
    pub anomalies: Vec<Anomaly>,            // {kind, count}
    pub sanitized: bool,
    pub sufficiency_notes: Vec<String>,
}
pub fn compute_validate(cap: &Capture) -> ValidateResult
```

Anomalies: `status-0`, `negative-size`, `zero-duration-with-body`, `missing-method`.
`sanitized` = uses Authorization on no entries AND has no cookies AND <10% have
response bodies (heuristic). Sufficiency notes call out what's missing ("no response
bodies — `errors`/`search` limited", "no auth headers — `auth`/`jwt` limited").
(Read-only inspection; no filter needed but accepts one for consistency.)

---

## Phase M6 — Extraction & data-out

### `search <pattern>` (#68 body search)
```rust
pub struct SearchResult { pub matches: Vec<SearchMatch> }
pub struct SearchMatch { pub id, pub location: String, pub snippet: String }  // location: "req.body"|"resp.body"
pub fn compute_search(cap: &Capture, filter: &Filter, pattern: &str, regex: bool, ignore_case: bool, top: usize, unsafe_include: bool) -> Result<SearchResult, String>
```

Searches request/response bodies. Substring by default; `--regex` compiles the
pattern (via the new `regex` crate; return `Err` on invalid pattern → exit 2);
`--ignore-case`. Each match emits a context window (~80 chars around the first hit)
passed through `redact_body` so secrets never leak (unless `--unsafe-include-secrets`).

### `extract <path>` (#69 JSON-path extraction)
New `src/jsonpath.rs`: `pub fn eval(v: &serde_json::Value, path: &str) -> Vec<Value>`
supporting `$.a.b`, `a.b[0].c`, and `[*]` wildcard over arrays. Minimal, hand-rolled,
no dependency.

```rust
pub struct ExtractResult { pub values: Vec<ExtractValue> }
pub struct ExtractValue { pub id: String, pub value: String }
pub fn compute_extract(cap: &Capture, filter: &Filter, path: &str, target: Target, top: usize, unsafe_include: bool) -> ExtractResult
```

`--target resp|req` (default resp). Parses the body as JSON, evaluates the path,
stringifies each result; opaque values masked via `is_opaque` unless unsafe.

### `export` (#101/#102 NDJSON/CSV; #103 SQLite deferred)
```rust
pub struct ExportRecord {
    pub id: String, pub offset_ms: f64, pub duration_ms: f64, pub method: String,
    pub host: String, pub norm_path: String, pub status: i64, pub bytes: i64,
    pub content_type: Option<String>, pub resource_type: String,
    pub correlation: Option<String>,
}
pub fn export_records(cap: &Capture, filter: &Filter) -> Vec<ExportRecord>
pub fn render_ndjson(records: &[ExportRecord]) -> String   // one JSON object per line
pub fn render_csv(records: &[ExportRecord]) -> String      // header + rows, RFC4180 quoting
```

`--format ndjson|csv` (default ndjson). Redaction: records carry only normalized
metadata (no raw bodies/headers), so they are safe by construction; `correlation`
is a benign id. Printed to stdout. (`--json` is a no-op for `export`; the format
flag governs output.)

---

## Phase M7 — Regression & rules

### `compare <baseline.har>` (#57 multi-HAR diff + #59 scoring + #112 CI gate)
`wiretrail new.har compare baseline.har [--fail-on <severity>]`. Loads the baseline
via `loader`+`assemble`, builds per-`(method,host,norm_path)` aggregates for both,
and diffs.

```rust
pub struct CompareResult {
    pub new_hosts: Vec<String>, pub removed_hosts: Vec<String>,
    pub new_endpoints: Vec<String>, pub removed_endpoints: Vec<String>,
    pub new_errors: Vec<EndpointDelta>,           // endpoints with 5xx/4xx absent in baseline
    pub latency_regressions: Vec<LatencyDelta>,   // p50 delta beyond threshold
    pub payload_growth: Vec<SizeDelta>,
    pub max_severity: String,
}
pub fn compute_compare(new: &Capture, base: &Capture, filter: &Filter, top: usize) -> CompareResult
```

Each regression is severity-scored (new 5xx → high; new 4xx → medium; p50 regression
> 2× and > 200 ms → medium; payload > 2× → low). `main` exits `1` when
`max_severity` ≥ `--fail-on` (default off → exit 1 only if any finding, matching the
other commands; `--fail-on high` gates CI strictly).

### `rules` (#60 rule engine + #61 packs)
Extend `Config` with a `rules` list and accept built-in packs.

```yaml
rules:
  - name: "API calls need auth"
    host: "api.foo.com"          # globs; optional method/path/status matchers
    require_headers: ["Authorization"]
    max_latency_ms: 2000
    forbid: false                 # if true, any match is a violation (forbidden host/endpoint)
```

```rust
pub struct Rule { name, host: Option<String>, path: Option<String>, method: Option<String>,
                  status: Option<String>, require_headers: Vec<String>,
                  max_latency_ms: Option<f64>, forbid: bool }   // in config.rs
pub struct RulesResult { pub findings: Vec<RuleFinding> }
pub struct RuleFinding { pub rule: String, pub severity: String, pub detail: String, pub entry_ids: Vec<String> }
pub fn compute_rules(cap: &Capture, filter: &Filter, config: &Config, packs: &[String], top: usize) -> RulesResult
```

Built-in packs (selected via `--pack auth,caching,...`) are `Rule` sets defined in
code: `auth` (Authorization on api hosts), `caching` (GET 200 without Cache-Control/
ETag), `rest` (no mutating verbs over GET), `payments` (idempotency-key on
create/charge paths), `security` (no secrets in query — reuses `is_opaque`),
`graphql` (POST to `/graphql` carries `operationName`). `checks` remains as the
built-in missing-header + content-type subset.

---

## Config extension

`Config` gains (additive, `#[serde(default)]`):

```rust
pub rules: Vec<Rule>,
```

## Cross-cutting requirements

- `--json` envelope on every command; filter language; `--top`; `next_commands`;
  findings-based exit codes (`compare` adds `--fail-on`).
- Redact-by-default; `search`/`extract` honor `--unsafe-include-secrets`; `export`
  emits only normalized metadata (no raw secrets).
- One new crate: `regex` (search). `jsonpath` hand-rolled. SQLite deferred.
- `diagnose`/`compare`/`rules` compose existing `compute_*` outputs (no re-derivation).

## Testing

- Per-module unit tests with `model::sample_entry`/`sample_capture`:
  - `diagnose`: a capture with a 5xx cluster + an old-token-reuse refresh yields
    `high` findings sorted first.
  - `startup`: overlapping intervals → correct `max_concurrency`; sequential chain →
    `critical_path`.
  - `cascade`: a 500 `/config` followed by N failures → one cascade with trigger_kind.
  - `validate`: a capture with no bodies/auth → `sanitized` + sufficiency notes;
    a status-0 entry → anomaly.
  - `search`: substring + regex + ignore-case; a secret in a body is redacted in the snippet.
  - `extract`/`jsonpath`: `$.errors[0].code` over a JSON body; `[*]` wildcard.
  - `export`: NDJSON line count == filtered entries; CSV header + quoting.
  - `compare`: two synthetic captures → new host, new 5xx, latency regression with
    correct severity; `--fail-on` exit code.
  - `rules`: a YAML rule (require_headers/max_latency/forbid) + a built-in pack fire
    correctly.
- Per-phase CLI integration tests (`tests/cli_diagnose.rs`, `cli_extract.rs`,
  `cli_compare.rs`): each command prints its header + a stable JSON envelope.
- Manual confidence check each phase against the two real HARs (15-24 and 19-24):
  `diagnose` should surface the Supabase token-refresh + ntsk 5xx cluster; `startup`
  the addon-manifest fan-out; `compare 15-24 vs 19-24` real host/endpoint deltas;
  `search "error"` and `extract '$.error.message'` over real error bodies.

## Non-goals / deferred

- SQLite/DuckDB export (#103), OpenAPI/mock/contract export (#104–106) — later.
- Golden-baseline file format (#58) beyond ad-hoc `compare` of two HARs.
- Full PII/secret scanner (#76/#77) as standalone commands (the `security` rule pack
  covers query-string opaque secrets; deep body PII scanning is a later milestone).
- Session/index cache (#98) — parse is fast enough (~0.5 s on 143 MB).
- The query-value-in-fingerprint residual leak remains open.
