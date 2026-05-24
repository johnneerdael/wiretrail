# wiretrail — Analysis Expansion (M1+M2+M3) Design

Date: 2026-05-24
Status: Approved (brainstorming complete; ready for implementation planning)
Author: John Neerdael

## Summary

wiretrail v1 ships 14 commands covering ~24 of a ranked 120-feature wishlist
(items 1–15 plus 17, 22, 44, 48, 64, 66, 67, 82, 97, 111). This spec defines the
next expansion: three coherent milestones adding **8 commands** that turn the
tool from "describes the capture" into "names the wasteful/auth/diff problems in
it". The milestones are independent and implemented as **separate plans**, but
share one spec because they share the established analysis architecture and a few
support modules.

The driving use case remains agentic Android API debugging (the nexio dossier):
the capture's central phenomena were request storms / fan-out, a Supabase
token-refresh flow, and repeated near-identical calls. v1 reports duplicates,
errors, retries, and transitions; this expansion characterizes them as
first-class patterns (storm, N+1, pagination, rate-limit), explains the auth
story (JWT decode, auth-failure, token-refresh), and shows what actually differs
between repeated calls (diff), plus config-driven checks.

## Locked decisions (from brainstorming)

1. **Scope:** one spec, three phases (M1, M2, M3); M4 (multi-HAR regression)
   deferred. Each phase becomes its own implementation plan.
2. **Command shape:** hybrid — separate commands where the question is distinct,
   grouped where they share a lens. 8 new commands total.
3. **`checks` ships now** with built-in missing-header (#34) and content-type
   mismatch (#35) checks; a future rule-engine milestone generalizes them.
4. **No new crate dependencies** — base64url decoding for JWT is hand-rolled.

## Architecture (unchanged from v1)

Each command is one focused module under `src/analysis/` exposing
`compute_<cmd>(cap, filter, …) -> <Cmd>Result` (serde `Serialize`) and
`render_<cmd>_text(&result) -> String`. `main.rs` adds a `clap` subcommand per
command and dispatches through the existing `emit`/`exit` helpers (stable JSON
envelope, terminal text + "next useful commands" footer, findings-based exit
codes). All commands honor the filter language, `--top`, and redact-by-default;
JWT/diff/handoff honor `--unsafe-include-secrets`.

Shared building blocks reused: `model::{Capture, Entry}`, `fingerprint`,
`grouping::{group_by_fingerprint, retry_entry_ids}`, `redact::{redact_url,
redact_value, redact_header_value, redact_body}`, `opaque`, `correlate`,
`config::Config`, `analysis::curl::entry_to_curl`, `render::{human_ms, human_bytes}`.

## New files

```
src/jwt.rs                       # base64url decode + JWT header/claims parse (support)
src/analysis/storms.rs           # M1
src/analysis/pagination.rs       # M1 (pagination loop + N+1)
src/analysis/rate_limit.rs       # M1
src/analysis/jwt.rs              # M2 (find + decode JWTs)
src/analysis/auth.rs             # M2 (auth failures + token refresh)
src/analysis/handoff.rs          # M2 (backend trace handoff)
src/analysis/diff.rs             # M3
src/analysis/checks.rs           # M3 (missing-header + content-type mismatch)
```
Modified: `src/analysis/mod.rs` (declare new modules), `src/lib.rs` (`pub mod jwt;`),
`src/config.rs` (`required_headers`), `src/main.rs` (8 subcommands).

---

## Phase M1 — Wasteful-traffic patterns

### `storms` (#25)
Sliding-window burst detection. Build two groupings: by `host`, and by
`(host, norm_path)`. For each group, sort members by `started_offset_ms` and
slide a window of `--window-ms` (default 1000); record the densest window whose
count ≥ `--min-count` (default 5).

Result: `StormsResult { storms: Vec<Storm> }`,
`Storm { scope_kind: "host"|"endpoint", scope: String, peak_count: usize,
window_ms: u64, first_offset_ms, last_offset_ms, calls_per_sec: f64,
entry_ids: Vec<String> }`. Sorted by `peak_count` desc. Findings if any storm.

### `pagination` (#26 N+1, #27 pagination loop)
Group by `(method, host, norm_path)`.
- **Pagination sequence:** members differ only in pagination query keys
  (`page`, `offset`, `cursor`, `page_token`, `after`, `before`, `start`). Emit a
  sequence with: param name(s), distinct page/cursor values, page count,
  `repeated_cursor` flag (same cursor seen ≥2×), `excessive` flag (count >
  `--max-pages`, default 20).
- **N+1 cluster:** an endpoint whose `norm_path` contains `{id}` (or `{blob}`)
  called ≥ `--fanout-min` times (default 5) within `--window-ms` (default 2000);
  record the preceding non-`{id}` sibling under the same host (the likely "list"
  call) when one exists just before the cluster.

Result: `PaginationResult { pages: Vec<PageSeq>, nplus1: Vec<NPlusOne> }`.
`PageSeq { host, method, norm_path, param_keys: Vec<String>, page_count,
repeated_cursor: bool, excessive: bool, entry_ids }`.
`NPlusOne { host, method, norm_path, fanout: usize, parent_id: Option<String>,
first_offset_ms, last_offset_ms, entry_ids }`. Findings if either non-empty.

### `rate-limit` (#28)
Select entries with `status == 429` or a response header
`x-ratelimit-remaining: 0`. For each, parse `Retry-After` (integer seconds or an
HTTP-date → seconds-from-entry) and any `x-ratelimit-*` headers. Detect follow-up
calls to the same `(host, norm_path)` whose `started_offset_ms` falls before
`limited_offset + retry_after`, flagging `cooldown_violated`.

Result: `RateLimitResult { groups: Vec<RateLimitGroup> }`,
`RateLimitGroup { host, norm_path, count_429: usize, retry_after_secs:
Vec<f64>, ratelimit_headers: BTreeMap<String,String>, cooldown_violated: bool,
violating_ids: Vec<String>, entry_ids: Vec<String> }`. Findings if any 429.

---

## Phase M2 — Auth & token

### `src/jwt.rs` (support module)
- `pub fn base64url_decode(s: &str) -> Option<Vec<u8>>` — standard base64url, no
  padding required; returns None on invalid input.
- `pub struct JwtClaims { alg, typ, iss, aud, sub_hash, iat, nbf, exp }` (all
  `Option`), `pub fn decode_jwt(token: &str) -> Option<JwtParts>` where
  `JwtParts { header: serde_json::Value, claims: serde_json::Value }` from parts
  0 and 1 only (signature ignored). A helper `summarize(parts, ref_epoch_ms) ->
  JwtSummary { alg, typ, iss, aud, sub_hash, iat, nbf, exp, expired: bool,
  seconds_to_expiry: Option<i64>, clock_skew_hint: Option<String> }`.
  `sub_hash` = short hash of `sub` (never the raw value). The raw token and
  signature are never returned in any summary.

### `jwt` command (#20)
Scan each entry for JWTs in: `Authorization: Bearer <jwt>`, `Cookie`/`Set-Cookie`
values, query values, and request/response bodies (regex `eyJ[A-Za-z0-9_-]+\.
eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+`). Decode + summarize against the entry's
`started_date_time`. Group by token hash.

Result: `JwtResult { tokens: Vec<JwtOccurrence> }`,
`JwtOccurrence { token_hash: String, source: String, summary: JwtSummary,
occurrences: usize, first_entry_id, last_entry_id }`. Findings if any token is
expired at its use time. Honors `--unsafe-include-secrets` (then includes the raw
token; default never does).

### `auth` command (#19 failures, #21 token refresh)
- **Failures:** group 401/403 by `(host, norm_path)`; flag hosts that use
  `Authorization` on some calls but omit it on others; count repeated failures;
  detect `Authorization` value changes across the capture per host (rotation).
- **Refresh:** classify a call as a refresh if URL path matches `/token`,
  `/oauth`, `/auth/refresh` or its body/query has `grant_type=refresh_token`.
  For each successful (2xx) refresh, compare the `Authorization` value used by
  calls before vs after it (same host): flag `old_token_reused` when later calls
  still send the pre-refresh token. Also flag `refresh_failed_then_storm`
  (a failed refresh followed by a burst — reuse storm logic over the next
  window) and `concurrent_refreshes` (≥2 refresh calls overlapping in time).

Result: `AuthResult { failures: Vec<AuthFailure>, missing_auth_hosts: Vec<String>,
token_changes: Vec<TokenChange>, refreshes: Vec<RefreshEvent> }`. Findings if any
failure, missing-auth host, or suspicious refresh flag.

### `handoff` command (#16)
For every failed (`is_error`) request and the top-N slowest, emit a handoff
block: method, normalized URL template (`norm_path`), status, absolute timestamp
(`started_date_time`) + offset, correlation IDs, `serverIPAddress`, and a
sanitized cURL (via `entry_to_curl`, honoring `--unsafe-include-secrets`).

Result: `HandoffResult { items: Vec<HandoffItem> }`,
`HandoffItem { id, method, host, norm_path, status, started_date_time,
offset_ms, correlation_ids: Vec<String>, server_ip: Option<String>,
curl: String }`. Findings if any item.

---

## Phase M3 — Diff & checks

### `diff` command (#31 body, #32 query, #33 header)
Group by `(method, host, norm_path)`; for each group with ≥2 members compute
variance:
- **Query:** keys whose values differ across members (with sample values,
  redacted via `redact_query_value`).
- **Headers:** request-header names whose values differ (values not printed
  unless `--unsafe-include-secrets`; redacted otherwise).
- **Body verdict:** `identical` (all bodies equal) / `volatile-only` (differ only
  in keys matching `timestamp|ts|nonce|date|_|cb|requestid`) / `meaningful`
  (real differences) / `none` (no bodies). For JSON bodies, compare parsed
  objects; for non-JSON, compare strings.

Result: `DiffResult { groups: Vec<DiffGroup> }`,
`DiffGroup { host, method, norm_path, count, varying_query_keys: Vec<String>,
varying_header_names: Vec<String>, body_verdict: String, entry_ids: Vec<String> }`.
Findings if any group has a `meaningful` body verdict or varying auth headers.

### `checks` command (#34 missing-header, #35 content-type mismatch)
- **Missing required headers:** extend `Config` with
  `required_headers: Vec<RequiredHeaderRule>` where
  `RequiredHeaderRule { host: String (glob), headers: Vec<String> }`. For each
  matching entry, report any listed header absent from the request.
- **Content-type mismatch (built-in heuristics):** request has a JSON-looking
  body but no `application/json` Content-Type; response body parses as JSON but
  `Content-Type` is `text/html`; response declares `Content-Encoding: gzip` but
  content is uncompressed-looking (size == compressed size, or text visible);
  empty body with a JSON Content-Type.

Result: `ChecksResult { findings: Vec<CheckFinding> }`,
`CheckFinding { rule: String, host, norm_path, detail: String,
entry_ids: Vec<String> }`. Findings if any.

---

## Config extension

`Config` gains an additive field (existing configs keep working):

```yaml
required_headers:
  - host: "api.company.com"
    headers: ["Authorization", "X-App-Version", "Accept"]
```

`pub required_headers: Vec<RequiredHeaderRule>` with `#[serde(default)]`.

## Cross-cutting requirements

- Every command: `--json` envelope (`tool`/`schema_version`/`command`/`capture`/
  `result`/`warnings`/`next_commands`), filter language, `--top`, deterministic
  ordering, findings-based exit code (`1` when findings, else `0`).
- Redact-by-default; `jwt`, `diff`, `handoff` honor `--unsafe-include-secrets`.
- No new crate dependencies.
- `main.rs` adds 8 subcommands with sensible `next_commands` cross-links.

## Testing

- Per-module unit tests using `model::sample_entry`/`sample_capture` synthetic
  captures: storms (burst vs spread), pagination (page sequence + N+1 fan-out),
  rate-limit (429 + Retry-After + cooldown violation), jwt (decode known token,
  expiry, redaction — never leak signature), auth (401 group, old-token-reuse
  after refresh), handoff (failed entry → block with curl + correlation), diff
  (identical / volatile-only / meaningful), checks (missing header from config,
  content-type mismatch).
- Per-phase CLI integration test (`tests/cli_patterns.rs`, `tests/cli_auth.rs`,
  `tests/cli_diff.rs`): each new command runs on a fixture, prints its header,
  and emits a stable JSON envelope; redaction asserted where relevant (jwt must
  not leak a signature; handoff curl must not leak `Authorization` by default).
- Manual confidence check each phase against the real nexio HAR
  (`~/Downloads/HTTPToolkit_2026-05-24_15-24.har`): it has genuine storms
  (165-req YouTube fan-out, 21× manifest), a Supabase 401→refresh→200 flow, and
  error traffic — so M1/M2 should surface real findings; M3 `diff` should show
  the 27× `sync_resolve_account_secret` calls as identical/volatile-only.

## Non-goals / judgment calls

- N+1 detection is heuristic (fan-out count + time window + sibling list call);
  best-effort, will miss some and is not authoritative.
- `handoff` emits terminal + JSON like other commands, not a separate markdown
  file (`report` already covers shareable markdown).
- `jwt`/`auth` are read-only: no signature verification (HAR cannot verify
  signatures; only structure/claims/expiry are analyzed).
- The full configurable rule engine, rule packs, severity model, and multi-HAR
  regression (features 57–63, 107–108) are a later milestone (M-Rules / M4).
- Residual from prior pass still open: duplicate *fingerprint* display includes
  raw query key=values (a secret in a query value can appear in
  summary/duplicates fingerprints). Not addressed here.
