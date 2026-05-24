# wiretrail

[![CI](https://github.com/johnneerdael/wiretrail/actions/workflows/ci.yml/badge.svg)](https://github.com/johnneerdael/wiretrail/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/wiretrail.svg)](https://crates.io/crates/wiretrail)
[![License](https://img.shields.io/crates/l/wiretrail.svg)](https://github.com/johnneerdael/wiretrail/blob/main/LICENSE)

`wiretrail` is a fast, deterministic, agent-friendly **HAR (HTTP Archive) analyzer**
for the command line. It answers narrow, repeatable questions about a network
capture in a single command — storms, duplicates, retries, errors, auth flows,
slow calls, what varies between repeated requests — with structured terminal
output and a stable `--json` schema, and **no GUI**.

It is **HeapTrail for network captures**. It reuses
[heaptrail](https://github.com/johnneerdael/heaptrail)'s design philosophy:
agentic/LLM-driven investigation, deterministic output that diffs cleanly,
JSON for machine consumers, and fixed single-command answers instead of an
interactive load-and-explore session.

Forked from [`mandrean/har-rs`](https://github.com/mandrean/har-rs), which
contributes the HAR 1.2/1.3 struct definitions; this fork replaces the parse path
with an mmap single-pass loader and adds the analysis CLI documented below.

## Motivation

Each `log.entries[]` item in a HAR already exposes the request, response, content,
timings, cache metadata, headers, cookies, URL, method, status, and bodies — more
than enough to reconstruct the kind of manual "what happened during this capture?"
analysis a debugger does by hand. wiretrail turns that into single-command
answers.

Sanitization is treated as core, not optional: HARs routinely contain cookies,
auth headers, tokens, request/response bodies, and user data. wiretrail **redacts
by default** across every output — including secret-bearing blobs hidden in URL
path segments — and only reveals raw values with an explicit
`--unsafe-include-secrets` flag.

### When to use `wiretrail`

- **Agentic / LLM-driven investigation.** Structured terminal output (with `--json`)
  lets an agent run a command, read the result, and decide the next probe. Every
  command prints a "next useful commands" footer.
- **Headless / CI.** Single static binary, deterministic output, defined exit codes
  (`0` clean, `1` findings, `2` invalid HAR).
- **Large captures.** mmap single-pass parse: a 143 MB capture (2237 entries) loads
  in ~0.5 s using ~2× the file size in RAM.
- **Safe to share.** Redact-by-default makes `report`, `curl`, and `show-entry`
  output safe to paste into a ticket; `--unsafe-include-secrets` when you actually
  need to replay a call.
- **Narrow, repeatable questions.** "What are the request storms?", "Which calls are
  wasteful duplicates vs retries?", "What's the auth/refresh story?", "What differs
  between these 27 identical-looking POSTs?" — one command each.

### When to use browser DevTools / a proxy GUI

- **Live, interactive inspection** while reproducing a flow — Chrome DevTools,
  Charles, Proxyman, HTTP Toolkit stay in their column.
- **Editing and re-sending** requests interactively. wiretrail emits sanitized
  `curl` for replay but isn't an interactive client.

The tools complement each other: capture in a proxy/DevTools, then run `wiretrail`
over the exported `.har` for fast, scriptable, agent-friendly post-mortem analysis.

## Install

```bash
cargo install wiretrail
```

Or build from git:

```bash
git clone https://github.com/johnneerdael/wiretrail
cd wiretrail
cargo build --release   # ./target/release/wiretrail
```

Pre-built binaries for Linux/macOS/Windows are attached to each
[GitHub release](https://github.com/johnneerdael/wiretrail/releases).

## Usage

```
wiretrail <FILE> [COMMAND] [OPTIONS]
```

`<FILE>` is a HAR (1.2 or 1.3) export. With no command, `summary` runs.

```bash
wiretrail capture.har                      # executive summary (default)
wiretrail capture.har duplicates           # repeated calls, grouped
wiretrail capture.har errors --json        # 4xx/5xx grouped, as JSON
wiretrail capture.har show-entry e000123   # full redacted detail for one entry
wiretrail capture.har curl e000123 --unsafe-include-secrets  # replayable cURL
```

### Global options

| Option | Effect |
|---|---|
| `--json` | Emit the stable JSON envelope instead of terminal text. |
| `--top N` | Bound list sizes (default 10). |
| `--filter "<expr>"` | Restrict to matching entries (repeatable). |
| `--config <path>` | Path to `wiretrail.yaml` (default: `./wiretrail.yaml` if present). |
| `--unsafe-include-secrets` | Show raw auth headers, tokens, bodies, and URL secrets instead of redacting. |

The filter language: `host:api.foo.com status:>=400 method:POST path:*login* time:>1000ms has:req.header.authorization`.

### Commands

**Overview & inventory**

| Command | Answers |
|---|---|
| `summary` *(default)* | Capture meta, time range, status/resource breakdown, top hosts, top duplicates, slowest, biggest payloads, root-cause hints. |
| `hosts` | Per-host: count, methods, status distribution, p50/p95/max latency, bytes, time window, duplicate count. |
| `subsystems` | Group hosts into named integrations (built-in vendor heuristics + `wiretrail.yaml` ownership map). |
| `endpoints` | Normalized endpoint inventory (host, method, `{id}` path, statuses, content types, sample query keys). |
| `timeline` | Chronological per-request view with DUP/RETRY markers. |

**Wasteful traffic**

| Command | Answers |
|---|---|
| `duplicates` | Repeated method + normalized-path + query fingerprint, grouped; marks retries. |
| `retries` | Repeats that follow a failed attempt (5xx/429/network), with backoff gaps. |
| `storms` | Bursts of many calls to one host/endpoint within a window (`--window-ms`, `--min-count`). |
| `pagination` | Pagination loops + N+1 fan-out (`--max-pages`, `--fanout-min`, `--window-ms`). |
| `rate-limit` | 429 events, `Retry-After`, `X-RateLimit-*`, cooldown violations. |

**Failures & timing**

| Command | Answers |
|---|---|
| `errors` | 4xx/5xx grouped by endpoint+status, with parsed message/code, correlation IDs, body snippet. |
| `redirects` | Redirect chains/storms, cross-host hops. |
| `transitions` | Status sequences: 401→200, 429→429, 5xx→2xx. |
| `slowest` | Top-N slow calls with timing-phase breakdown + bottleneck classifier. |

**Auth**

| Command | Answers |
|---|---|
| `jwt` | Find and decode JWTs (header/claims, `exp`/skew) — hashed `sub`, never the signature or raw token by default. |
| `auth` | 401/403 patterns, inconsistent auth, token rotation, and token-refresh flows (old-token-reuse, concurrent, failed). |
| `handoff` | Backend trace-handoff blocks for failed + slowest requests (template, correlation IDs, server IP, sanitized cURL). |

**Inspection, diff & export**

| Command | Answers |
|---|---|
| `show-entry <id>` | Full normalized request/response/timings for one entry, redacted. |
| `diff` | What varies across repeated calls to one endpoint (query/header/body verdict: identical / volatile-only / meaningful). |
| `checks` | Built-in checks: required-headers (config) + content-type mismatches. |
| `report` | A dossier-style markdown report composed from the analyses. |
| `curl [id]` | Sanitized, safety-labeled `curl` replay for one entry or all filtered entries. |

Run `wiretrail <file> <command> --help` for per-command options.

## Example

```text
$ wiretrail capture.har summary
== wiretrail summary ==
entries: 2237 total, 2237 after filter
duration (first start to last response): 1354.1s

status classes:
  2xx: 2040
  4xx: 57
  5xx: 2

top hosts (by request count):
    785  api.themoviedb.org
    131  yjyuomfgkqwmjvnoxurn.supabase.co

top duplicate calls:
    29x  POST youtubei.googleapis.com /youtubei/v1/visitor_id ...

hints:
  - 29x duplicate calls: POST youtubei.googleapis.com /youtubei/v1/visitor_id ...
  - 59 error responses (4xx/5xx/failed)

next useful commands: duplicates · errors · slowest
```

## Configuration

An optional `wiretrail.yaml` (in the working directory, or via `--config`) maps
hosts/paths to named subsystems and declares required-header rules:

```yaml
ownership:
  - name: Payments API
    host: "payments.*.foo.com"
    owner: payments-team
    criticality: high

required_headers:
  - host: "api.foo.com"
    headers: ["Authorization", "X-App-Version", "Accept"]
```

Without config, `subsystems` falls back to built-in vendor heuristics, then raw host.

## Redaction & safety

Every command redacts by default: auth/cookie headers, sensitive query params,
JSON body keys (`password`/`token`/`secret`/…), JWT signatures, and opaque
secret-bearing blobs embedded in URL path segments (collapsed to `{blob}` in
aggregate views, `<redacted>` in `show-entry`/`curl`). Pass
`--unsafe-include-secrets` to reveal raw values for replay. `curl` labels each
command SAFE/UNSAFE based on method and payment/order keywords.

## Performance

mmap + a single typed `serde_json::from_slice` over the mapped bytes — no
intermediate `serde_json::Value` DOM. Measured on a 143 MB capture (2237 entries):
parse + summary in ~0.5 s, ~314 MB peak RSS (~2.2× file size). Every command runs
in well under a second on that input. Release builds use `lto = "fat"`.

## Format support

- HAR 1.2 and 1.3 (parsed through a unified permissive model; unknown fields ignored).
- Reads JSON HAR exports from Chrome/Edge DevTools, Charles, Proxyman, HTTP Toolkit,
  mitmproxy, and others.

## Known limitations

HAR cannot prove low-level packet loss, full TLS certificate chains, client call
stacks, or proxy/service-worker internals unless captured in custom fields. JWT
analysis is structural only (no signature verification). N+1 detection is a
best-effort heuristic. See [USERGUIDE.md](USERGUIDE.md) for details and worked
examples.

## License

MIT. See [LICENSE](LICENSE).
