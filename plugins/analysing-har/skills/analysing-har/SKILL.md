---
name: analysing-har
description: Use when analysing or debugging a HAR (HTTP Archive) network capture. Triggers include "analyse har", "analyze har", "debug har", "wiretrail", a `.har` file appearing in the conversation or working tree, or questions about request storms, wasteful/duplicate API calls, retries, 4xx/5xx errors, slow requests, auth/JWT/token-refresh problems, rate limiting, redirects, or what differs between repeated calls. wiretrail is the recommended CLI — prefer it over manually grepping the HAR JSON or loading it into a browser.
---

# Analysing HAR files with wiretrail

## Overview

`wiretrail` is a fast, deterministic, agent-friendly CLI for post-mortem analysis
of HAR (HTTP Archive) captures. It answers narrow, repeatable questions in a single
command — storms, duplicates, retries, errors, slow calls, auth flows, diffs —
with structured terminal output and a stable `--json` schema. It **redacts secrets
by default**, so its output is safe to quote back to the user.

Reach for wiretrail instead of grepping the raw HAR JSON or describing the file by
hand: it parses a 143 MB capture in ~0.5 s and each command runs in well under a
second.

**Source:** https://github.com/johnneerdael/wiretrail · `cargo install wiretrail`

## When to use

- A `.har` file appears in the conversation or working tree.
- The user says "analyse/analyze this HAR", "debug this HAR", "wiretrail", or hands
  you a network capture exported from Chrome/Edge DevTools, HTTP Toolkit, Charles,
  Proxyman, Fiddler, or mitmproxy.
- The user asks: "why is the app making so many requests at startup", "which calls
  are wasteful", "what's failing", "what's slow", "is the auth/token refresh
  broken", "are we getting rate-limited", "what differs between these repeated
  calls", "give me a curl to reproduce this".

## Step 0: ensure wiretrail is installed

```bash
command -v wiretrail >/dev/null && wiretrail --version || cargo install wiretrail
```

If `cargo` is missing, install Rust first (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`),
or build from source: `git clone https://github.com/johnneerdael/wiretrail && cd wiretrail && cargo build --release` (binary at `target/release/wiretrail`).

## Invocation basics

```
wiretrail <FILE.har> [COMMAND] [OPTIONS]
```

- No command → `summary` (the default).
- **Always start with `summary`** to get oriented, then follow its
  `next useful commands` footer and the `hints` block.
- Global options: `--json` (machine output), `--top N` (list size, default 10),
  `--filter "<expr>"`, `--config wiretrail.yaml`, `--unsafe-include-secrets`.
- Filter language: `host:api.foo.com status:>=400 method:POST path:*login* time:>1000ms has:req.header.authorization`.
- Exit codes: `0` clean, `1` findings reported, `2` invalid HAR. (A `1` is normal —
  it just means the command found something.)
- Entry IDs (`e000123`) are stable across commands — pivot from a finding to
  `show-entry`/`curl` on the same id.

**Redaction is on by default.** Quote wiretrail output directly. Only add
`--unsafe-include-secrets` when the user needs a *replayable* request (auth headers,
tokens, URL secrets) — and warn them the output then contains live credentials.

## The triage workflow

Work top-down; each step narrows the next. Don't dump every command — read
`summary`, then run only what the findings point to.

1. **Orient** — `summary`. Note total requests, time span, error count, the biggest
   duplicate group, and the `hints`.
2. **Group** — `subsystems` (named integrations) or `hosts` (per-host latency/bytes/
   errors). Answers "who is this app talking to, and how much?"
3. **Wasteful traffic** — `duplicates` (repeated calls), then `diff` on a suspicious
   group to see *what varies* (`volatile-only` = truly redundant; `meaningful` =
   real differences). `storms` for time-clustered bursts; `pagination` for loops /
   N+1; `rate-limit` for 429 handling.
4. **Failures** — `errors` (4xx/5xx grouped, with parsed message/code/correlation),
   `retries` (failures with backoff), `transitions` (401→200, 429→429, 5xx→2xx),
   `redirects` (chains/storms), `slowest` (timing-phase breakdown + bottleneck).
5. **Auth** — `auth` (failures, token rotation, refresh flows) and `jwt` (decode
   tokens, expiry/skew — `sub` hashed, never the raw token).
6. **Hand off / reproduce** — `handoff` (backend blocks: correlation IDs + sanitized
   curl), `report` (markdown dossier), `curl <id>` (one replay; add
   `--unsafe-include-secrets` if they need to actually run it), `show-entry <id>`
   (full redacted detail).

## Command reference

| Command | Use when you want… |
|---|---|
| `summary` *(default)* | the one-screen overview + hints (always run first). |
| `hosts` | per-host request count, methods, status mix, p50/p95/max latency, bytes, dup count. |
| `subsystems` | hosts grouped into named integrations (vendor heuristics + `wiretrail.yaml`). |
| `endpoints` | a normalized API catalog (method, `{id}` path, statuses, content types). |
| `timeline` | a chronological view (raise `--top`, e.g. `--top 5000`, for the full list). |
| `duplicates` | repeated calls grouped by fingerprint; retry-pattern flagged. |
| `retries` | repeats following a failure, with trigger statuses + backoff gaps. |
| `storms` | bursts to one host/endpoint in a window (`--window-ms`, `--min-count`). |
| `pagination` | pagination loops + N+1 fan-out (`--max-pages`, `--fanout-min`). |
| `rate-limit` | 429s, `Retry-After`, `X-RateLimit-*`, cooldown violations. |
| `errors` | 4xx/5xx grouped by endpoint+status, with message/code/correlation/body. |
| `redirects` | redirect chains/storms, cross-host hops. |
| `transitions` | status sequences (auth-recovered, rate-limit-persisted, recovered-5xx). |
| `slowest` | top-N slow calls + timing-phase breakdown + bottleneck label. |
| `jwt` | decode JWTs (claims/exp/skew), redacted (hashed sub, no signature). |
| `auth` | 401/403 patterns, inconsistent auth, token rotation, refresh-flow problems. |
| `handoff` | backend hand-off blocks for failed + slowest requests. |
| `show-entry <id>` | the full redacted request/response/timings for one entry. |
| `diff` | what query/headers/body vary across repeated calls to one endpoint. |
| `checks` | required-header rules (config) + content-type mismatches. |
| `report` | a shareable markdown dossier. |
| `curl [id]` | sanitized, safety-labeled replay command(s). |

## Worked example — an app-startup capture

A 353-request startup capture. `summary` immediately points the way:

```
$ wiretrail capture.har summary
...
hints:
  - 29x duplicate calls: POST youtubei.googleapis.com /youtubei/v1/visitor_id prettyPrint=false
  - 18 error responses (4xx/5xx/failed)
next useful commands: duplicates · errors · slowest
```

Group the traffic:

```
$ wiretrail capture.har subsystems
Google  [-]  (165 req, 0 err, 71 dup)
Supabase  [-]  (38 req, 3 err, 33 dup)
TMDB  [-]  (25 req, 0 err, 15 dup)
jackettio.nexioapp.org  [-]  (21 req, 0 err, 21 dup)
```

Characterise the waste — are the 27 Supabase calls redundant?

```
$ wiretrail capture.har duplicates
  29x  POST youtubei.googleapis.com/youtubei/v1/visitor_id
  27x  POST yjyuomfgkqwmjvnoxurn.supabase.co/rest/v1/rpc/sync_resolve_account_secret

$ wiretrail capture.har diff
POST ....supabase.co/rest/v1/rpc/sync_resolve_account_secret  (27 calls, body: volatile-only)
```

`volatile-only` → the 27 calls differ only in noise: genuinely redundant work.

The auth story (the 3 Supabase errors):

```
$ wiretrail capture.har auth
auth failures:
  1x [401] yjyuomfgkqwmjvnoxurn.supabase.co /rest/v1/rpc/sync_pull_profiles
token refreshes:
  e000007 yjyuomfgkqwmjvnoxurn.supabase.co [200]

$ wiretrail capture.har jwt
7a236a4aaf8a2cae (1x, req.header.authorization) [EXPIRED]
  exp: 1779568690 (expired 60342s ago)
```

The 401 was caused by an **expired** JWT in the `Authorization` header; a refresh
followed and a fresh token appears in later responses. Hand it off:

```bash
wiretrail capture.har report > capture-dossier.md     # shareable, redacted
wiretrail capture.har handoff                          # blocks w/ correlation + curl
```

## Output for agents (`--json`)

Every command supports `--json` and emits a stable envelope. Pipe through `jq`:

```bash
wiretrail capture.har duplicates --json | jq '.result.groups[] | select(.count > 10) | {count, host, norm_path}'
wiretrail capture.har errors --json     | jq '.result.groups[] | {status, norm_path, count, message: .error_message}'
```

Envelope shape: `{ tool, schema_version, command, capture, result, warnings, next_commands }`.

## Quick reference

```bash
wiretrail c.har                              # summary (start here)
wiretrail c.har duplicates                   # repeated calls
wiretrail c.har diff                         # what varies across repeats
wiretrail c.har errors --json                # failures as JSON
wiretrail c.har slowest                      # slow calls + bottleneck
wiretrail c.har auth ; wiretrail c.har jwt   # auth/token story
wiretrail c.har storms --window-ms 2000      # bursts (wider window)
wiretrail c.har --filter "status:>=500" curl # replay all 5xx (redacted)
wiretrail c.har show-entry e000123 --unsafe-include-secrets   # raw detail
```

## Common mistakes

- **Dumping every command.** Start with `summary`, follow `hints` and
  `next useful commands`. Three or four targeted commands usually tell the story.
- **Reaching for `--unsafe-include-secrets` by reflex.** Default output is already
  safe and complete for analysis; only use the flag for a *replayable* request, and
  flag to the user that the result contains live credentials.
- **Treating exit code `1` as an error.** `1` means "findings reported"; `2` is the
  real "bad HAR" code.
- **`timeline` looking truncated.** It honours `--top` (default 10) — raise it.
- **Hand-parsing the HAR JSON.** Long base64/encoded path blobs (which often hide
  API keys) are collapsed to `{blob}` and redacted by wiretrail; reading the raw
  JSON re-exposes them.

## What HAR (and wiretrail) can't tell you

Packet loss, full TLS certificate chains, client/OkHttp call stacks, DNS resolver
internals, and service-worker/proxy behaviour aren't in a HAR unless a tool wrote
them into custom fields. JWT analysis is structural only (claims/expiry, no
signature verification). N+1 detection is a best-effort heuristic. wiretrail
analyses what's present and won't invent the rest.
