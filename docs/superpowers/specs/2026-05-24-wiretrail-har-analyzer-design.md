# wiretrail — HAR Analyzer Design

Date: 2026-05-24
Status: Approved (brainstorming complete; ready for implementation planning)
Author: John Neerdael

## Summary

`wiretrail` is a Rust CLI for fast, deterministic, agent-driven analysis of HAR
(HTTP Archive) files — including large captures (200–300 MB) at speed. It is
**heaptrail for network captures**: single-command answers, structured terminal
output, stable `--json`, no GUI dependency.

It is built by forking `mandrean/har-rs` (this repository): the existing HAR
struct knowledge becomes the parsing foundation — mirroring how
[`heaptrail`](https://github.com/johnneerdael/heaptrail) was built on
`agourlay/hprof-slurp` — and the repository is re-identified as the `wiretrail`
binary crate. The README attributes the upstream `har` crate, as heaptrail
attributes hprof-slurp.

### Primary use case

Drive HAR analysis from an AI agent (for the `nexio` project) in a token-efficient
way, reproducing — from the HAR alone — the kind of manual analysis captured in
`~/Scripts/nexio/docs/superpowers/reports/2026-05-24-startup-network-har-review-dossier.md`.

That dossier is the canonical target output. Everything in it is reproducible
from the HAR **except** the source-code references (e.g.
`HomeViewModelCatalogPipeline.kt:1346`), which come from analyzing the nexio
repository, not the HAR. wiretrail produces the HAR-side evidence — category
table, per-host breakdown, duplicate index, error groups, redirect storms,
status transitions — that an agent then correlates with source.

## Design tenets

- **Agent-driven.** Run command → read structured result → decide the next probe.
  A GUI cannot sit inside that loop.
- **Evidence-first.** Every finding cites entry IDs, timestamps, host, endpoint,
  status, and short redacted snippets.
- **Redact-by-default.** Terminal, JSON, and exports redact sensitive values
  unless explicitly overridden.
- **Deterministic & diffable.** Stable ordering, stable IDs, `--no-color` CI mode,
  defined exit codes.

## Locked decisions (from brainstorming)

1. **Repo & parser strategy:** Fork & extend this repo; fix the parse path
   (single-pass typed deserialization). Session cache deferred to phase 2.
2. **Grouping:** Built-in vendor heuristics **plus** an optional per-project YAML
   ownership map; raw-host fallback when unmapped.
3. **v1 scope:** The 10 dossier-reproducing commands **plus** pulled-forward
   phase-2 items: status-code transitions, retry-vs-duplicate distinction,
   markdown report export, and cURL/repro export.
4. **CLI shape:** Subcommands (not heaptrail-style mode flags).
5. **Name:** `wiretrail`.

## Architecture & parsing

The performance fix is the core of v1. The upstream path
(`bytes → serde_json::Value → serde_json::from_value`) builds a full JSON DOM and
*then* clones into typed structs — roughly 1 GB+ on a 300 MB HAR. The
internally-tagged `Spec` enum (`#[serde(tag = "version")]`) additionally forces
serde to buffer content. v1 replaces both:

- **mmap the input** (`memmap2`) — avoids a 300 MB read copy.
- **One permissive raw model** covering HAR 1.2 *and* 1.3 (1.3 is largely a
  superset of 1.2), deserialized in a single `serde_json::from_slice` over the
  mapped bytes — direct into typed structs, with **no intermediate `Value`** and
  **no version-tagged enum**. A cheap byte-probe reads `log.version` for
  reporting and quirk handling only (not for type selection).
- Transform raw → **normalized analysis model** (below). Bodies are retained once
  (or sliced on demand); metadata commands never re-parse them.

**Targets:** 300 MB parsed in a few seconds; peak RAM under ~1.5 GB. Release
profile `lto = "fat"`, `codegen-units = 1` (matching heaptrail).

The on-disk **session cache** (parse once; repeated agent commands hit the cache)
is **phase 2** — single-pass parse is fast enough that v1 does not require it.

### Crate layout

```
src/
  main.rs, cli.rs              clap subcommands + dispatch
  har/        raw.rs, load.rs  forked structs + mmap loader (unified 1.2/1.3)
  model/      normalize.rs, classify.rs, correlate.rs, entry.rs
  filter/                      filter-language parser + matcher
  redact/                      redaction engine (output-boundary)
  analysis/   summary, hosts, subsystems, endpoints, duplicates,
              errors, redirects, slowest, timeline, transitions, retries
  render/                      terminal + json renderers, exit codes
  export/     report.rs (markdown dossier), curl.rs
  config.rs                    YAML ownership/config
```

The crate's product is the `wiretrail` binary. HAR parsing lives in an internal
module forked from the existing crate's structs; we drop the published-library
obligation of the `har` crate (new crate identity).

## Normalized analysis model

Computed once per entry and reused by every command:

- **EntryId** — deterministic `e000123`, assigned by capture order.
- **Route normalization** — collapse path segments matching pure-numeric, UUID
  (`8-4-4-4-12` hex), long-hex (≥16 hex chars), or base64-ish IDs into `{id}`.
  Deterministic; overridable via config endpoint aliases.
- **Resource classification** — `api | media | static | analytics | document |
  other`, from content-type + extension + known-vendor host. Lets `summary` make
  large request counts legible (API calls vs media probes vs images), as the
  dossier does.
- **Subsystem / owner** — vendor heuristics + YAML ownership map; raw-host
  fallback.
- **Correlation IDs** — `x-request-id`, `x-correlation-id`, `traceparent`,
  `x-amzn-trace-id`, `cf-ray`, `x-datadog-trace-id`, plus configurable headers.
- Plus: host, method, status, started-offset (ms from capture start), duration,
  timing phases (blocked/dns/connect/ssl/send/wait/receive), size with accuracy
  mode (decoded vs transferred), content-type, redirect target, body
  presence/kind.

## Redaction & safety

A redaction engine applied at the **output boundary**, so terminal and JSON are
both safe by construction. Redacts:

- Headers: `Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key`, bearer/JWT.
- Query params: `token`, `key`, `sig`, `password`, `access_token`,
  `refresh_token` (configurable).
- JSON body keys: `password`, `token`, `secret`, `authorization`, `access_token`,
  `refresh_token` (configurable).
- JWTs: decode header/claims **shape** only; never emit signature or raw token.

`--unsafe-include-secrets` opts out. An export that would leak secrets without
this flag fails with exit code `3`.

## Commands (v1)

Invocation shape: `wiretrail <file> <command> [filters] [--json] [--top N]`.
Every command supports `--json`, the filter language, `--no-color`, and prints a
**"next useful commands"** footer in terminal mode.

| Command | Output |
|---|---|
| `summary` *(default)* | capture meta, time range, total + by-resource-type counts, top hosts, top duplicates, error count, slowest, biggest payloads, root-cause hints |
| `subsystems` | dossier category table (heuristics + YAML grouping): counts, time windows, notes |
| `hosts` | per-host: count, methods, status distribution, p50/p95/max latency, bytes, time window, dup count |
| `endpoints` | normalized endpoint inventory: host, method, `{id}` path, observed statuses, content-types, sample params |
| `duplicates` | method + normalized-path + query/body fingerprint groups; marks which members are retries |
| `retries` | the subset of duplicates classified as retries (prior 5xx/429/status-0 + backoff gap) |
| `errors` | 4xx/5xx grouped by host/endpoint: status, body snippet, error code/message, correlation IDs, first/last occurrence |
| `redirects` | redirect chains + storms (the 21× 308), loops, cross-host, auth/param drops |
| `slowest` | top-N + timing phase breakdown + bottleneck classifier (DNS/connect/TLS/wait/receive) |
| `transitions` | sequences: 401→refresh→200, 302→login→200, 429→retry→429, 500→retry→success |
| `timeline` | chronological compact columns: offset, duration, method, host, endpoint, status, bytes, corr-id, dup/retry marker |
| `show-entry <id>` | full normalized request/response/timings for one entry, redacted |
| `report` | emit a dossier-style **markdown** document (summary + category table + duplicate index + errors + evidence) |
| `curl <selector>` | sanitized cURL replay for an entry / endpoint group / all failures, with safe/unsafe labels |

### Duplicates vs retries (detail)

- **Fingerprint** = method + normalized-path + sorted query (excluding known
  cache-buster/nonce params) + body fingerprint (hash, with timestamp/nonce
  normalization).
- **Duplicates** = same fingerprint occurring ≥2 times.
- **Retry** = a duplicate whose prior attempt failed (5xx, 429, status 0, or a
  network error) with a timing gap consistent with backoff. `duplicates` reports
  all repetition and flags retries; `retries` reports only the retry subset.

## Filter language

Shared by all commands via one parser + matcher:

```
host:api.foo.com status:>=400 method:POST path:*login* time:>1000ms has:req.header.authorization
```

Supports comparators (`>= <= > < =`), globs (`*`), and `has:` existence checks
over request/response headers, query params, and body presence.

## JSON schema & exit codes

Stable envelope on every command (`--json`):

```json
{
  "tool": "wiretrail",
  "schema_version": 1,
  "command": "duplicates",
  "capture": { "...meta..." : null },
  "result": { "...command specific..." : null },
  "warnings": [],
  "next_commands": []
}
```

Stable IDs for entries, hosts, endpoints, and groups. Evidence snippets carry
entry IDs. Exit codes:

- `0` — no findings (or below threshold)
- `1` — findings over threshold
- `2` — invalid / unparseable HAR
- `3` — unsafe output blocked (would leak secrets without `--unsafe-include-secrets`)

## Configuration

Optional `wiretrail.yaml` (discovered in cwd, or via `--config`):

- Ownership map: host/path glob → `{ name, owner, criticality }`.
- Host aliases and endpoint aliases.
- Additional redaction keys (headers, query params, JSON keys).
- Correlation header names.
- Noise suppression rules (still counted in raw stats).

Absent config → built-in vendor heuristics + raw-host fallback.

## Phasing

- **v1 (this spec):** the 14 commands above + parsing rewrite + normalized model
  + filter language + redaction engine + YAML config + JSON/terminal renderers +
  exit codes.
- **Phase 2:** on-disk session cache; N+1 detector; request-storm detector;
  rate-limit analysis; JWT-claims / auth-flow / token-refresh deep analysis;
  body/query/header diffing; payload-growth suspects; GraphQL operation
  extraction + error clustering; schema inference; multi-HAR diff + golden
  baseline + regression scoring; rule engine + rule packs; NDJSON/CSV/SQLite
  export.
- **Phase 3:** OpenAPI skeleton export; mock-server / contract-test generation;
  signed-URL expiry; clock-skew detector; environment-mismatch detection; the
  long tail of the original feature list.

## Testing strategy

- Existing fixtures `tests/fixtures/someapi123.har` and `someapi13.har` retained.
- A synthetic large-HAR generator for performance verification on 200–300 MB
  inputs.
- **Golden snapshot tests** for terminal and JSON output, per command.
- Unit tests for route normalization, resource classification, the filter
  language, and the redaction engine.
- A **sanitized synthetic HAR modeled on the nexio dossier** committed as the
  end-to-end integration fixture (the real nexio HAR is sensitive and stays a
  local-only smoke test).

## Judgment calls (open to override)

- v1 stays single-pass; session cache deferred to phase 2.
- `report` emits markdown only (not HTML).
- `summary` is the default no-arg command.

## Non-goals (what HAR cannot prove)

Low-level packet loss, full TLS certificate chains, Android call stacks, OkHttp
interceptor state, DNS resolver internals, and service-worker/proxy behavior —
unless captured in custom HAR fields. A `limitations` note in relevant output
states this explicitly.
