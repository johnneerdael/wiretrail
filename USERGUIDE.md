# wiretrail User Guide

A practical guide to analyzing HAR captures with `wiretrail`. Examples use output
shaped like a real ~144 MB mobile-app capture (2237 requests across ~22 minutes).

> **Redaction is on by default.** Everything below is safe to paste into a ticket.
> Add `--unsafe-include-secrets` only when you need raw values (e.g. to replay a call).

---

## 1. Capturing a HAR

wiretrail reads any HAR 1.2 / 1.3 JSON export. Common sources:

- **Chrome / Edge DevTools** → Network tab → right-click → *Save all as HAR with content*.
  (Enable "Preserve log" first to keep entries across navigations.)
- **HTTP Toolkit / Charles / Proxyman / Fiddler** → export session as `.har`.
- **mitmproxy** → `mitmdump -w flow` then convert, or export from mitmweb.
- **Android apps** → proxy device traffic through HTTP Toolkit / Charles / mitmproxy
  and export. (Bodies require the proxy's CA installed on the device.)

A "sanitized" HAR (some tools strip auth/cookies/bodies on export) still works —
wiretrail analyzes whatever is present and the redaction is a no-op on absent data.

---

## 2. Cheat sheet

```bash
wiretrail capture.har                       # executive summary (default command)
wiretrail capture.har hosts                 # per-host latency/bytes/errors
wiretrail capture.har subsystems            # named integrations (config + heuristics)
wiretrail capture.har duplicates            # wasteful repeated calls
wiretrail capture.har retries               # repeats after a failure (with backoff)
wiretrail capture.har storms                # call bursts in a time window
wiretrail capture.har pagination            # pagination loops + N+1 fan-out
wiretrail capture.har rate-limit            # 429s, Retry-After, cooldown violations
wiretrail capture.har errors                # 4xx/5xx grouped, parsed messages
wiretrail capture.har redirects             # redirect chains/storms
wiretrail capture.har transitions           # 401→200, 429→429, 5xx→2xx
wiretrail capture.har slowest               # slow calls + timing phase breakdown
wiretrail capture.har jwt                   # decode JWTs (redacted)
wiretrail capture.har auth                  # auth failures + token-refresh story
wiretrail capture.har handoff               # backend hand-off blocks (failed+slow)
wiretrail capture.har diff                  # what varies across repeated calls
wiretrail capture.har checks                # required-header + content-type checks
wiretrail capture.har timeline              # chronological view
wiretrail capture.har show-entry e000123    # one entry, full + redacted
wiretrail capture.har report                # markdown dossier
wiretrail capture.har curl e000123          # sanitized replay command

# Modifiers (global)
--json                     # machine-readable envelope
--top N                    # bound list sizes (default 10; raise for timeline)
--filter "host:api.x status:>=400 method:POST path:*login* time:>1000ms"
--config wiretrail.yaml    # subsystem ownership + required-header rules
--unsafe-include-secrets   # reveal raw secrets (for replay)
```

Every command supports `--json` and the filter language, and prints a
"next useful commands" footer in terminal mode.

---

## 3. `summary` — what happened in this capture?

```text
$ wiretrail capture.har
== wiretrail summary ==
entries: 2237 total, 2237 after filter
duration (first start to last response): 1354.1s

status classes:
  2xx: 2040
  3xx: 136
  4xx: 57
  5xx: 2
  other: 2

resource types:
  api: 1113
  media: 954
  document: 99
  other: 69
  static: 2

top hosts (by request count):
    785  api.themoviedb.org
    744  images.metahub.space
    131  yjyuomfgkqwmjvnoxurn.supabase.co
     62  api.trakt.tv

top duplicate calls:
    29x  POST youtubei.googleapis.com /youtubei/v1/visitor_id ...

hints:
  - 29x duplicate calls: POST youtubei.googleapis.com /youtubei/v1/visitor_id ...
  - 59 error responses (4xx/5xx/failed)

next useful commands: duplicates · errors · slowest
```

The `hints` block is the fastest "where do I look?" — it surfaces the largest
duplicate group and the error count. `resource types` separates business API
traffic from media/static noise so big counts stay legible.

---

## 4. Wasteful traffic

### `duplicates` — repeated calls

Groups entries by `method + host + normalized-path + query fingerprint`
(cache-buster/nonce params excluded). The `[retry pattern]` tag marks groups where
a repeat followed a failure.

```text
$ wiretrail capture.har duplicates
== wiretrail duplicates ==

  27x  POST yjyuomfgkqwmjvnoxurn.supabase.co /rest/v1/rpc/sync_resolve_account_secret
  statuses: 200:27
  entries: e000085, e000090, ...

   9x [retry pattern]  POST api.ntsk.cloud /v1/ratings/bulk
  statuses: 500:8 0:1
  entries: e000112, e000138, ...
```

### `retries` vs `duplicates`

`retries` is the subset that follows a failure — the actionable kind. It shows
trigger statuses and the backoff gaps between attempts:

```text
$ wiretrail capture.har retries
POST api.ntsk.cloud/v1/ratings/bulk  (9 attempts, 8 retries, final 500)
  triggered by: 500, 0
  backoff gaps: 3.7s, 8.4s, 1.2s, 727ms, 744ms, 1.5s, 8.5s, 17.7s
```

### `storms` — bursts in a window

```text
$ wiretrail capture.har storms
endpoint torii.nexioapp.org/{blob}/manifest.json  14 calls in 1.0s (14.0/s)
  window: 15.8s - 16.7s
```

Tune the burst definition with `--window-ms` (default 1000) and `--min-count`
(default 5). `host`-scope storms catch fan-out across many endpoints; `endpoint`
storms catch one endpoint being hammered.

### `pagination` — loops + N+1

```text
$ wiretrail capture.har pagination
pagination sequences:
  2 pages  GET api.themoviedb.org/3/movie/popular  (by page) [repeated-cursor]

N+1 fan-out:
  13x  GET kitsu.io/api/edge/anime/{id}  (after e000140)
```

`repeated-cursor` = the same page/cursor requested twice (a loop). N+1 flags an
`{id}`-bearing endpoint hit many times in a window, with the preceding list call.

### `rate-limit`

Surfaces `429`s (and `X-RateLimit-Remaining: 0`), parses `Retry-After` and
`X-RateLimit-*`, and flags follow-up calls made *before* the cooldown elapsed
(`[cooldown violated]`).

---

## 5. Failures & timing

### `errors` — grouped, with parsed messages

```text
$ wiretrail capture.har errors
   8x  [500] POST api.ntsk.cloud/v1/ratings/bulk
  message: internal server error
  code: internal_error
  correlation: a00c9e346f71ef9c-AMS
  body: {"error":{"code":"internal_error","message":"internal server error"}}
  entries: e000112, e000138, ...

   1x  [401] POST yjyuomfgkqwmjvnoxurn.supabase.co/rest/v1/rpc/sync_pull_profiles
  message: JWT expired
  code: PGRST303
```

Body snippets are redacted (sensitive JSON keys scrubbed) and collapsed to one line.

### `slowest` — with bottleneck classification

```text
$ wiretrail capture.har slowest
   2.2s  e000210 POST openrouter.ai/api/v1/chat/completions  [200]
  bottleneck: server wait/TTFB
  phases: wait 2.1s / receive 40ms / send 1ms / connect 0ms / ...
```

The classifier labels the dominant timing phase: DNS, TCP connect, TLS handshake,
request upload, server wait/TTFB, download/receive, queueing/blocked, or unknown.

### `transitions`

```text
$ wiretrail capture.har transitions
401 -> 200  [auth-recovered]  POST .../rest/v1/rpc/sync_pull_profiles
  e000006 -> e000014  (gap 729ms)
```

---

## 6. Auth

### `jwt` — decode, never leak

Finds JWTs in headers, cookies, query, and bodies; decodes header + claims;
**hashes `sub`** and never prints the signature or raw token (unless
`--unsafe-include-secrets`):

```text
$ wiretrail capture.har jwt
7a236a4aaf8a2cae (1x, req.header.authorization) [EXPIRED]
  iss: https://....supabase.co/auth/v1
  aud: authenticated
  sub (hashed): 2115b3e5941ae067
  exp: 1779568690 (expired 60342s ago)

a766b5c02d06f209 (33x, resp.body)
  exp: 1779632633 (3600s left)
```

This is the whole expired-token story at a glance: the stale token in the
`Authorization` header (which triggered the 401) and the fresh one returned in
later response bodies.

### `auth` — failures + refresh

```text
$ wiretrail capture.har auth
auth failures:
  1x [401] ....supabase.co /rest/v1/rpc/sync_pull_profiles

hosts with inconsistent Authorization: ....supabase.co

token rotation:
  ....supabase.co (2 distinct tokens)

token refreshes:
  e000007 ....supabase.co [200]
```

Suspicious refresh patterns are tagged: `old-token-reused` (refresh succeeded but
later calls still send the pre-refresh token), `concurrent`, `failed`.

### `handoff` — give the backend team everything

For every failed and top-N slowest request: method, normalized URL template,
status, timestamp, correlation IDs, server IP, and a sanitized `curl`.

---

## 7. Inspection, diff & export

### `show-entry <id>`

```text
$ wiretrail capture.har show-entry e000009
== wiretrail entry e000009 ==
GET https://torii.nexioapp.org/<redacted>/manifest.json  [308] Permanent Redirect
host: torii.nexioapp.org  http: HTTP/1.1  type: api
request headers:
  Authorization: <redacted>
  ...
```

The base64 config blob in the path (which embeds API keys) shows as `<redacted>`.
Add `--unsafe-include-secrets` to get the replayable URL.

### `diff` — what actually changed?

```text
$ wiretrail capture.har diff
POST youtubei.googleapis.com/youtubei/v1/reel/reel_item_watch  (28 calls, body: meaningful)
  query id varies: yeetI2DfbaE, -4ZVFspRn3M, gMC8kkwbIQQ

POST ....supabase.co/rest/v1/rpc/sync_resolve_account_secret  (27 calls, body: volatile-only)
  headers vary: content-length
```

`volatile-only` means the 27 calls differ only in noise (timestamps/nonces) — they
are genuinely redundant. `meaningful` means real payload differences.

### `checks`

With a `wiretrail.yaml` declaring `required_headers`, flags requests missing them,
plus built-in content-type mismatches (JSON body sent as `text/plain`, JSON
response served as `text/html`, empty body with a JSON content-type).

### `report` — shareable markdown

```bash
wiretrail capture.har report > capture-dossier.md
```

Composes summary + subsystem table + duplicate index + errors + redirect storms +
slowest into one redacted markdown document.

### `curl` — sanitized replay

```bash
wiretrail capture.har curl e000123                     # one entry (redacted)
wiretrail capture.har curl --filter "status:>=500"     # all 5xx, each labeled
wiretrail capture.har curl e000123 --unsafe-include-secrets   # replayable
```

Each command is labeled `SAFE`/`UNSAFE` based on method (mutating?) and
payment/order keywords in the path.

---

## 8. `--json` — for scripts and agents

Every command emits a stable envelope:

```bash
wiretrail capture.har duplicates --json | jq '.result.groups[] | select(.count > 10)'
```

```json
{
  "tool": "wiretrail",
  "schema_version": 1,
  "command": "duplicates",
  "capture": { "entry_count": 2237, "duration_ms": 1354100.0, ... },
  "result": { "groups": [ ... ] },
  "warnings": [],
  "next_commands": ["retries", "errors", "show-entry"]
}
```

Entry IDs (`e000123`) are stable across commands, so an agent can pivot from a
`summary` finding to `show-entry` to `curl` on the same entry.

---

## 9. End-to-end: an Android startup investigation

```bash
# 1. What dominates the capture?
wiretrail capture.har summary
#    -> 2237 reqs, top dup 29x visitor_id, 59 errors

# 2. Characterize the wasteful traffic
wiretrail capture.har storms          # addon-manifest bursts (14/s)
wiretrail capture.har duplicates      # 27x sync_resolve_account_secret
wiretrail capture.har diff            # ...which are body: volatile-only (truly redundant)

# 3. The auth story
wiretrail capture.har auth            # 401 -> refresh -> 200; token rotation
wiretrail capture.har jwt             # the 401 token is EXPIRED; fresh token follows

# 4. The errors
wiretrail capture.har errors          # 8x 500 from /v1/ratings/bulk (+ retries)
wiretrail capture.har retries         # confirms backoff between the 8 attempts

# 5. Hand off / reproduce
wiretrail capture.har handoff         # blocks with correlation IDs + sanitized curl
wiretrail capture.har report > dossier.md
```

---

## 10. Performance and limits

- **Throughput:** mmap + one typed `from_slice` (no `serde_json::Value` DOM). 143 MB
  / 2237 entries parses + summarizes in ~0.5 s.
- **Memory:** ~2× file size peak RSS (it holds bodies in memory once). A 300 MB
  capture extrapolates to ~1 s / ~650 MB.
- **Per command:** well under a second on a 143 MB input.
- **`--top` and `timeline`:** `timeline` is bounded by `--top` (default 10) — raise
  it (`--top 5000`) for a full chronological dump.

### What HAR can't tell you

Packet loss, full TLS certificate chains, client/OkHttp call stacks, DNS resolver
internals, and service-worker/proxy behavior are not in a HAR unless a tool wrote
them into custom fields. wiretrail analyzes what's present and won't invent the rest.

JWT analysis is **structural only** — claims, expiry, and skew; it does not verify
signatures (a HAR can't). N+1 detection is a best-effort heuristic (fan-out count +
time window + a preceding list call).

---

## 11. Troubleshooting

**"failed to parse HAR JSON"** — the file isn't valid JSON or isn't a HAR. Check it
opens in a JSON viewer and has a top-level `log.entries` array.

**A command prints a header but no rows** — there's nothing to report (e.g. no
storms, no 429s), or `--top` is too low. Many commands legitimately find nothing on
a clean capture.

**Exit code 1 from a "successful" run** — by design: `1` means *findings were
reported* (errors, duplicates, etc.), `0` means clean. Useful as a CI gate. `2`
means the HAR was invalid/unreadable.

**A secret I need is `<redacted>`** — add `--unsafe-include-secrets`. It applies to
`curl`, `show-entry`, `errors`, `report`, `jwt`, and `diff`.

**`subsystems` shows raw hostnames** — that's the fallback when a host isn't a known
vendor and isn't in your `wiretrail.yaml` ownership map. Add a rule to name it.
