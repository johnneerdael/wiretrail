# wiretrail — Secret/Opaque Handling Fix Design

Date: 2026-05-24
Status: Approved (brainstorming complete; ready for implementation planning)
Author: John Neerdael

## Summary

A real-data run of wiretrail v1 against an 11 MB HTTP Toolkit capture
(`HTTPToolkit_2026-05-24_15-24.har`, the nexio dossier's source) surfaced a
credential leak and several smaller issues. This fix closes the leak and improves
output readability without changing the analysis semantics.

The capture's Stremio addon manifest URLs embed base64 / URL-encoded config blobs
in the **path** that contain real debrid API keys (torbox
`e2574d74-c23d-4846-a66e-d756b43f94ec`, premiumize `szpwe4fx4ngs8u9q`). wiretrail
printed these unredacted in `summary`, `duplicates`, `redirects`, and `show-entry`
(both the request URL and the `Location` response header), because redaction only
covered query params, header *names*, and JSON body keys — never the URL path, and
never header *values* that happen to carry secrets. Since `report`, `curl`, and
`show-entry` are the explicitly "safe to share" outputs, this is a safe-by-default
violation.

## Findings addressed

- **Finding 1 (HIGH):** secrets in URL path segments leak unredacted across
  `summary`, `duplicates`, `redirects`, `show-entry`, `curl`.
- **Finding 2 (MEDIUM):** long opaque path segments (500+ char base64 / URL-encoded
  blobs) are never collapsed to a token, producing enormous unreadable fingerprints.
- **Finding 3 (LOW):** raw HTML body snippets contain newlines that break the
  aligned terminal layout.
- **Finding 4 (LOW):** `summary` reports a 75.6 s window (last *response* end) while
  subsystems/dossier say 75.4 s (last *start*) — two valid measures, read as
  contradictory.
- **Finding 5 (LOW):** benign-named headers (`Location`, `Report-To`, `CF-Ray`)
  carry secret/opaque values not caught by name-based redaction.

## Locked decisions (from brainstorming)

1. **Approach:** a single shared opaque-segment detector, reused by normalization
   (collapse → token) and redaction (mask in displayed URLs / header values).
2. **Token:** collapsed opaque segments use a distinct `{blob}` token (vs `{id}`
   for numeric/UUID/hex resource ids), so an agent can tell a hidden blob from a
   normal id.
3. **Finding 5 in scope now:** URL-valued headers run through the URL redactor;
   other header values are scanned for opaque chunks.

## Design

### 1. `opaque` module (new) — shared detector

`src/opaque.rs`: `pub fn is_opaque(s: &str) -> bool`, true when `s` is:

- **percent-encoded blob** — contains a `%XX` escape and `len >= 16`
  (catches `%7B%22NexioTorii%22…`); or
- **base64 blob** — `len >= 24`, every char in `[A-Za-z0-9+/=_-]`, has at least
  one ASCII digit, and has at least one ASCII letter (catches the addon `eyJ…==`
  config); or
- **catch-all** — `len >= 40` and contains at least one ASCII digit.

Tuned to NOT fire on readable tokens: `manifest.json`, `videoplayback`,
`sync_resolve_account_secret` (no digit), or word-slugs like
`the-quick-brown-fox-jumps-over-the-lazy-dog` (no digit) stay untouched.

### 2. `normalize` — `{id}` vs `{blob}`

Refactor the segment classifier in `src/normalize.rs`:

- numeric (not a single leading digit), UUID, or long-hex (`>= 16` hex) → `Id` →
  rendered `{id}` (readable resource ids).
- `opaque::is_opaque(seg)` → `Blob` → rendered `{blob}`.
- otherwise → unchanged.

Because every aggregate view (`summary`, `duplicates`, `redirects`, `endpoints`,
`subsystems`, `timeline`) renders `norm_path`, this one change removes the embedded
keys from all of them. The existing `is_base64ish` helper is replaced by
`opaque::is_opaque`.

### 3. `redact` — value-level redaction

In `src/redact.rs`:

- **`redact_url(url: &str, unsafe_include: bool) -> String`** — parse the URL;
  rebuild it with: each path segment where `is_opaque(seg)` replaced by
  `<redacted>` (numeric/readable segments preserved — you still see `/users/123`);
  query values redacted when the name is sensitive (existing) **or**
  `is_opaque(value)`. `unsafe_include` returns the raw URL. On parse failure,
  fall back to scanning the raw string with `redact_value`.
- **`redact_value(value: &str, unsafe_include: bool) -> String`** — split on common
  delimiters (whitespace, `;,&=/?"`), replace any chunk where `is_opaque(chunk)`
  with `<redacted>`, rejoin. Used for non-URL header values.
- **`redact_header_value`** upgraded: `unsafe_include` → raw; sensitive name (existing
  list) → `<redacted>`; URL-valued header name (`location`, `referer`,
  `content-location`) → `redact_url(value)`; otherwise → `redact_value(value)`.
- **`redact_query_value`** upgraded: also redact when `is_opaque(value)`, not only on
  a sensitive name.
- **Body snippets:** `redact_body` collapses `\n`, `\r`, `\t` runs to a single space
  so snippets stay single-line (Finding 3).

Mask vocabulary: the redactor uses `<redacted>` (consistent with existing
header/query redaction); normalization uses `{blob}`. They serve different roles
(safety mask vs structural grouping token) and are intentionally distinct.

### 4. Apply at the display boundary

- `src/analysis/show_entry.rs`: build the `url` field via `redact_url(&e.url, unsafe)`;
  headers already pass through `redact_header_value`, which is now URL/opaque-aware.
- `src/analysis/curl.rs`: `build_url` delegates to `redact_url`.
- Aggregate views: no change — they render `norm_path` (now `{blob}`).

### 5. Finding 4 — window labeling

No semantic change. Relabel the `summary` terminal line from `capture window: <x>`
to `duration (first start → last response): <x>` so it is not read as conflicting
with the per-subsystem offset windows. `CaptureMeta.duration_ms` stays the canonical
total span.

## Components and boundaries

- `opaque` — pure predicate over a `&str` chunk. No deps. Unit-testable in isolation.
- `normalize` — depends on `opaque`; owns the `{id}`/`{blob}` policy.
- `redact` — depends on `opaque` and `url`; owns all masking. Display modules call it.
- Display modules (`show_entry`, `curl`) — call `redact_url`/`redact_header_value`;
  no redaction logic of their own.

## Testing

- **`opaque` unit tests:** positives — addon base64 (`eyJ…==`), percent-encoded
  `%7B…` blob, 40+char-with-digit; negatives — `manifest.json`,
  `sync_resolve_account_secret`, `the-quick-brown-fox-jumps-over-the-lazy-dog`,
  `v1`, `123`, `videoplayback`.
- **`normalize`:** opaque segment → `{blob}`; numeric/UUID → `{id}`; readable word
  preserved; existing route-normalization tests still pass.
- **`redact_url`:** opaque path segment → `<redacted>`; numeric path kept; opaque
  query value → `<redacted>`; `unsafe_include` → raw.
- **`redact_header_value`:** `Location` URL with opaque path → redacted; `Report-To`
  opaque substring → redacted; `Accept` untouched; sensitive name still fully redacted.
- **`redact_body`:** newlines collapsed to single line.
- **Regression fixture (committed):** a small synthetic HAR with a fake key
  (`FAKEKEY_a1b2c3d4e5f6g7h8`) inside a base64-encoded path segment and echoed in a
  `Location` header. Integration test asserts the fake key appears in **no** command's
  default output (`summary`, `duplicates`, `redirects`, `show-entry`, `curl`,
  `report`) and **does** appear under `--unsafe-include-secrets`. This locks the leak
  shut against regressions.

## Non-goals

- Body-content secret scanning beyond the existing JSON-key redaction (a general
  content secret-scanner was considered and deferred — see brainstorming). Bodies
  are still redacted by JSON key name + truncation only.
- Decoding base64/percent blobs to redact individual fields inside them; the whole
  opaque segment is masked as one unit.
