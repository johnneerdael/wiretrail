# wiretrail Secret/Opaque Handling Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the URL-path credential leak (and related readability issues) by introducing one shared opaque-segment detector reused by route normalization (`{blob}`) and a value/URL redactor (`<redacted>`).

**Architecture:** A new `opaque` module exposes `is_opaque(&str)` (+ `is_uuid`). `normalize` uses it to collapse opaque path segments to `{blob}` (distinct from `{id}`), fixing every aggregate view at once. `redact` gains `redact_url` and `redact_value`, and its header/query redactors become opaque-aware; `show_entry` and `curl` route their displayed URLs through `redact_url`. A committed synthetic fixture + integration test lock the leak shut.

**Tech Stack:** Rust 2024, serde/serde_json, url, plus the existing wiretrail modules.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-secret-opaque-redaction-fix-design.md`.

**Refinement to spec §1 thresholds (intent unchanged):** the symbol-less base64
case additionally requires mixed case (has upper AND lower) so long lowercase
dashed slugs with digits (e.g. `my-very-long-feature-slug-2024-edition`) are NOT
flagged; the unambiguous standard-base64 markers (`+ / =`) and percent-encoding
(`%`) are treated as strong positive signals. Hex strings and UUIDs are explicitly
excluded (they are readable ids, not secret blobs).

## Current state (verified)

- `src/normalize.rs` has `normalize_path` → `is_id_segment(seg, index)` →
  `is_uuid`, `is_long_hex`, `is_base64ish`. `is_base64ish` rejects `=`/`+`, so the
  real addon blobs are never collapsed. `is_base64ish` will be removed.
- `src/redact.rs` has `REDACTED`, `SENSITIVE_HEADERS`, `SENSITIVE_QUERY_KEYS`,
  `redact_header_value`, `redact_query_value`, `redact_body` (+ `redact_json`,
  `truncate`). Redaction is name-based only.
- `src/analysis/curl.rs` has a private `build_url` that redacts query only.
- `src/analysis/show_entry.rs` `entry_detail` sets `url: e.url.clone()` (raw).
- `lib.rs` declares modules through `pub mod timing;` (last line).

## File structure

```
src/opaque.rs              # NEW: is_opaque + is_uuid (shared detector)
src/lib.rs                 # Modify: pub mod opaque;
src/normalize.rs           # Modify: classifier -> {id}/{blob}; drop is_base64ish; use opaque
src/redact.rs              # Modify: redact_url, redact_value, opaque-aware header/query, body newline strip
src/analysis/show_entry.rs # Modify: url field via redact_url
src/analysis/curl.rs       # Modify: build_url -> redact_url
src/analysis/summary.rs    # Modify: window line relabel (Finding 4)
tests/fixtures/secret_in_path.har  # NEW: synthetic leak fixture
tests/cli_redaction.rs     # NEW: integration test (no leak in any default output)
```

---

### Task 1: `opaque` module

**Files:**
- Create: `src/opaque.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Declare the module.** In `src/lib.rs`, after `pub mod timing;`, add:

```rust
pub mod opaque;
```

- [ ] **Step 2: Write the failing tests** at the top of `src/opaque.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::is_opaque;

    #[test]
    fn flags_standard_base64_blob() {
        // jackettio-style standard base64 with padding
        assert!(is_opaque("eyJtYXhUb3JyZW50cyI6OCwiZGVicmlkIjp0cnVlfQ=="));
    }

    #[test]
    fn flags_percent_encoded_blob() {
        assert!(is_opaque("%7B%22NexioTorii%22%3A%22eyJ1c2VFbmdsaXNo%22%7D"));
    }

    #[test]
    fn flags_base64url_token() {
        // JWT-like base64url, no +/=, mixed case + digits
        assert!(is_opaque("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn does_not_flag_readable_segments() {
        assert!(!is_opaque("manifest.json"));
        assert!(!is_opaque("videoplayback"));
        assert!(!is_opaque("sync_resolve_account_secret"));
        assert!(!is_opaque("v1"));
        assert!(!is_opaque("popular"));
    }

    #[test]
    fn does_not_flag_lowercase_slug_with_digit() {
        // long, has a digit, but lowercase dashed slug -> readable, must stay
        assert!(!is_opaque("my-very-long-feature-slug-2024-edition"));
    }

    #[test]
    fn does_not_flag_hex_or_uuid() {
        assert!(!is_opaque("0123456789abcdef0123")); // hex hash
        assert!(!is_opaque("550e8400-e29b-41d4-a716-446655440000")); // uuid
    }

    #[test]
    fn does_not_flag_short_strings() {
        assert!(!is_opaque("szpwe4fx4ngs8u9q")); // 16-char token, below blob threshold
        assert!(!is_opaque("abc"));
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib opaque 2>&1 | tail -12`
Expected: FAIL with "cannot find function `is_opaque`".

- [ ] **Step 4: Implement** above the test module in `src/opaque.rs`:

```rust
/// True when a string chunk looks like an opaque blob (base64 / percent-encoded /
/// long high-entropy token) rather than a readable path/identifier. Hex strings
/// and UUIDs are treated as readable ids and excluded.
pub fn is_opaque(s: &str) -> bool {
    let len = s.len();
    if len < 16 {
        return false;
    }
    if s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false; // hex hash/etag -> readable id
    }
    if is_uuid(s) {
        return false; // resource id, not a secret blob
    }

    // Percent-encoded blob (URL-encoded JSON/config).
    if s.contains('%') {
        return true;
    }

    // Must be within the base64 alphabet to be considered a blob.
    let base64_alpha = s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'_' | b'-'));
    if !base64_alpha {
        return false;
    }
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let has_upper = s.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = s.bytes().any(|b| b.is_ascii_lowercase());
    let has_b64_symbol = s.bytes().any(|b| matches!(b, b'+' | b'/' | b'='));

    // Standard base64 markers never appear in slugs/ids -> strong signal.
    if len >= 24 && has_b64_symbol && has_digit {
        return true;
    }
    // Symbol-less base64url token: long, mixed case + digit (slugs are lowercase).
    if len >= 32 && has_digit && has_upper && has_lower {
        return true;
    }
    false
}

/// Canonical 8-4-4-4-12 hex UUID check.
pub fn is_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != groups.len() {
        return false;
    }
    parts
        .iter()
        .zip(groups)
        .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib opaque 2>&1 | tail -10`
Expected: PASS (7 tests).

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/opaque.rs
git commit -m "feat: shared opaque-segment detector"
```

---

### Task 2: `normalize` — `{id}` vs `{blob}`

**Files:**
- Modify: `src/normalize.rs`

- [ ] **Step 1: Add a failing test** for the `{blob}` behavior. In `src/normalize.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add these functions before its closing brace:

```rust
    #[test]
    fn collapses_opaque_blob_to_blob_token() {
        assert_eq!(
            normalize_path("/cfg/eyJtYXhUb3JyZW50cyI6OCwiZGVicmlkIjp0cnVlfQ==/manifest.json"),
            "/cfg/{blob}/manifest.json"
        );
    }

    #[test]
    fn collapses_percent_encoded_blob() {
        assert_eq!(
            normalize_path("/%7B%22NexioTorii%22%3A%22eyJ1c2VFbmdsaXNo%22%7D/manifest.json"),
            "/{blob}/manifest.json"
        );
    }

    #[test]
    fn numeric_id_still_uses_id_token() {
        assert_eq!(normalize_path("/users/123/orders/456"), "/users/{id}/orders/{id}");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib normalize 2>&1 | tail -15`
Expected: FAIL — `collapses_opaque_blob_to_blob_token` expects `{blob}` but current code emits the raw segment (and `collapses_percent_encoded_blob` likewise).

- [ ] **Step 3: Replace the classifier.** In `src/normalize.rs`, replace the `normalize_path` function, the `is_id_segment` function, the `is_uuid` function, and the `is_base64ish` function (keep `is_long_hex`) with:

```rust
use crate::opaque::{is_opaque, is_uuid};

/// Collapse identifier-like path segments into `{id}` and opaque blobs into
/// `{blob}` so routes group together and secret-bearing config blobs are hidden.
pub fn normalize_path(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, seg) in path.split('/').enumerate() {
        match segment_token(seg, i) {
            Some(tok) => parts.push(tok.to_string()),
            None => parts.push(seg.to_string()),
        }
    }
    parts.join("/")
}

fn segment_token(seg: &str, index: usize) -> Option<&'static str> {
    if seg.is_empty() {
        return None;
    }
    // Pure numeric: id unless a single leading digit (keeps `/3/tv/popular`).
    if seg.bytes().all(|b| b.is_ascii_digit()) {
        return if index == 1 && seg.len() == 1 { None } else { Some("{id}") };
    }
    if is_uuid(seg) {
        return Some("{id}");
    }
    if is_long_hex(seg) {
        return Some("{id}");
    }
    if is_opaque(seg) {
        return Some("{blob}");
    }
    None
}

fn is_long_hex(s: &str) -> bool {
    s.len() >= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
```

Note: the previous `is_uuid` and `is_base64ish` are now removed from this file —
`is_uuid` is imported from `crate::opaque`, and `is_base64ish` is replaced by
`is_opaque`.

- [ ] **Step 4: Run to verify pass** (new tests + the original 5 normalization tests).

Run: `cargo test --lib normalize 2>&1 | tail -14`
Expected: PASS (8 tests: 5 original + 3 new). If the original `collapses_long_hex`
or `collapses_uuid` fail, re-check `segment_token` ordering — they must still map to `{id}`.

- [ ] **Step 5: Commit**

```bash
git add src/normalize.rs
git commit -m "feat: normalize opaque path segments to {blob}, ids to {id}"
```

---

### Task 3: `redact` — URL + value redaction

**Files:**
- Modify: `src/redact.rs`

- [ ] **Step 1: Add failing tests.** In `src/redact.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add before its closing brace:

```rust
    #[test]
    fn redact_url_masks_opaque_path_keeps_numeric() {
        let url = "https://h.example.com/cfg/eyJrZXkiOiJzZWNyZXQiLCJuIjoxMjN9==/users/123";
        let out = super::redact_url(url, false);
        assert!(out.contains("/cfg/<redacted>/users/123"));
        assert!(!out.contains("eyJrZXki"));
    }

    #[test]
    fn redact_url_masks_opaque_query_keeps_safe() {
        let url = "https://h.example.com/x?token=eyJhbGciOiJIUzI1NiJ9abc123XYZ&page=2";
        let out = super::redact_url(url, false);
        assert!(out.contains("page=2"));
        assert!(out.contains("token=<redacted>"));
    }

    #[test]
    fn redact_url_unsafe_is_raw() {
        let url = "https://h.example.com/cfg/eyJrZXkiOiJzZWNyZXQiLCJuIjoxMjN9==/x";
        assert_eq!(super::redact_url(url, true), url);
    }

    #[test]
    fn header_location_value_is_url_redacted() {
        let v = "https://h.example.com/%7B%22k%22%3A%22eyJzZWNyZXQiOnRydWV9%22%7D/manifest.json";
        let out = super::redact_header_value("Location", v, false);
        assert!(out.contains("<redacted>"));
        assert!(!out.contains("eyJzZWNyZXQi"));
    }

    #[test]
    fn header_value_opaque_substring_redacted() {
        let v = "report-to; s=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ123";
        let out = super::redact_header_value("Report-To", v, false);
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn header_accept_untouched() {
        assert_eq!(
            super::redact_header_value("Accept", "application/json", false),
            "application/json"
        );
    }

    #[test]
    fn query_value_redacted_when_opaque() {
        // benign name, but opaque value -> redacted
        assert_eq!(
            super::redact_query_value("d", "eyJhbGciOiJIUzI1NiJ9abc123XYZ", false),
            "<redacted>"
        );
    }

    #[test]
    fn body_snippet_is_single_line() {
        let body = "line one\nline two\tindented\r\nline three";
        let out = super::redact_body(body, false, 1000);
        assert!(!out.contains('\n'));
        assert!(!out.contains('\t'));
        assert!(!out.contains('\r'));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib redact 2>&1 | tail -15`
Expected: FAIL with "cannot find function `redact_url`" (and others).

- [ ] **Step 3: Add the new redactors and import.** At the top of `src/redact.rs`, add the opaque import directly below the existing first line (the file currently starts with `pub const REDACTED...`); add this line above it:

```rust
use crate::opaque::is_opaque;
```

Then add these items directly above the existing `#[cfg(test)] mod tests` block:

```rust
const URL_VALUED_HEADERS: &[&str] = &["location", "referer", "content-location"];

const VALUE_DELIMS: &[char] = &[' ', '\t', '\n', '\r', ';', ',', '&', '=', '/', '?', '"', '{', '}', '[', ']', ':'];

/// Redact secret-bearing chunks from a free-form value: split on common
/// delimiters and replace any opaque chunk with the redaction marker.
pub fn redact_value(value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chunk = String::new();
    for ch in value.chars() {
        if VALUE_DELIMS.contains(&ch) {
            flush_chunk(&mut out, &mut chunk);
            out.push(ch);
        } else {
            chunk.push(ch);
        }
    }
    flush_chunk(&mut out, &mut chunk);
    out
}

fn flush_chunk(out: &mut String, chunk: &mut String) {
    if chunk.is_empty() {
        return;
    }
    if is_opaque(chunk) {
        out.push_str(REDACTED);
    } else {
        out.push_str(chunk);
    }
    chunk.clear();
}

/// Rebuild a URL with opaque path segments and sensitive/opaque query values
/// redacted. `unsafe_include` returns the raw URL. Falls back to `redact_value`
/// on parse failure.
pub fn redact_url(url: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return url.to_string();
    }
    let Ok(u) = url::Url::parse(url) else {
        return redact_value(url, false);
    };

    let path: String = u
        .path()
        .split('/')
        .map(|seg| if is_opaque(seg) { REDACTED } else { seg })
        .collect::<Vec<_>>()
        .join("/");

    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| {
            let rv = redact_query_value(k.as_ref(), v.as_ref(), false);
            (k.into_owned(), rv)
        })
        .collect();

    let mut out = String::new();
    out.push_str(u.scheme());
    out.push_str("://");
    if let Some(host) = u.host_str() {
        out.push_str(host);
    }
    if let Some(port) = u.port() {
        out.push_str(&format!(":{port}"));
    }
    out.push_str(&path);
    if !pairs.is_empty() {
        out.push('?');
        let q: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        out.push_str(&q.join("&"));
    }
    out
}
```

- [ ] **Step 4: Upgrade `redact_header_value`.** Replace the existing `redact_header_value` function body with:

```rust
pub fn redact_header_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_HEADERS.iter().any(|h| *h == lname) {
        return REDACTED.to_string();
    }
    if URL_VALUED_HEADERS.iter().any(|h| *h == lname) {
        return redact_url(value, false);
    }
    redact_value(value, false)
}
```

- [ ] **Step 5: Upgrade `redact_query_value`.** Replace the existing `redact_query_value` function body with:

```rust
pub fn redact_query_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_QUERY_KEYS.iter().any(|k| *k == lname) || is_opaque(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}
```

- [ ] **Step 6: Strip newlines in body snippets.** Replace the existing `redact_body` function with:

```rust
/// Redact and truncate a request/response body for safe single-line display.
pub fn redact_body(body: &str, unsafe_include: bool, max: usize) -> String {
    let scrubbed = if unsafe_include {
        body.to_string()
    } else if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) {
        redact_json(&mut v);
        serde_json::to_string(&v).unwrap_or_default()
    } else {
        body.to_string()
    };
    truncate(&collapse_newlines(&scrubbed), max)
}

fn collapse_newlines(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect()
}
```

- [ ] **Step 7: Run to verify pass** (new tests + the original redact tests).

Run: `cargo test --lib redact 2>&1 | tail -16`
Expected: PASS (the original 7 + 8 new = 15). The original `passes_through_safe_header`
now routes through `redact_value("application/json")` → splits on `/` into
`application` / `json`, neither opaque → rejoined unchanged. ✓

- [ ] **Step 8: Commit**

```bash
git add src/redact.rs
git commit -m "feat: opaque-aware URL/value redaction + body newline strip"
```

---

### Task 4: Wire `show-entry` and `curl`

**Files:**
- Modify: `src/analysis/show_entry.rs`
- Modify: `src/analysis/curl.rs`

- [ ] **Step 1: Add a failing test to `show_entry`.** In `src/analysis/show_entry.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add before its closing brace:

```rust
    #[test]
    fn url_path_blob_is_redacted_by_default() {
        let mut e = sample_entry(0, "h.example.com", "GET", "/manifest.json", 200);
        e.url = "https://h.example.com/cfg/eyJrZXkiOiJzZWNyZXQiLCJuIjoxMjN9==/manifest.json".into();
        let d = entry_detail(&e, false);
        assert!(d.url.contains("<redacted>"));
        assert!(!d.url.contains("eyJrZXki"));
        // unsafe shows raw
        let d2 = entry_detail(&e, true);
        assert!(d2.url.contains("eyJrZXki"));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib show_entry 2>&1 | tail -12`
Expected: FAIL — `d.url` currently equals the raw URL, so `<redacted>` is absent.

- [ ] **Step 3: Route the url through `redact_url`.** In `src/analysis/show_entry.rs`:

First update the redact import line. It currently reads:
```rust
use crate::redact::{redact_body, redact_header_value, redact_query_value};
```
Change it to:
```rust
use crate::redact::{redact_body, redact_header_value, redact_query_value, redact_url};
```

Then in `entry_detail`, change the `url` field assignment from:
```rust
        url: e.url.clone(),
```
to:
```rust
        url: redact_url(&e.url, unsafe_include),
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib show_entry 2>&1 | tail -10`
Expected: PASS (4 tests: 3 original + 1 new).

- [ ] **Step 5: Replace `curl`'s `build_url` with `redact_url`.** In `src/analysis/curl.rs`:

Update the redact import line. It currently reads:
```rust
use crate::redact::{redact_body, redact_header_value, redact_query_value};
```
Change it to:
```rust
use crate::redact::{redact_body, redact_header_value, redact_url};
```

In `entry_to_curl`, change:
```rust
    let url = build_url(e, unsafe_include);
```
to:
```rust
    let url = redact_url(&e.url, unsafe_include);
```

Delete the entire `fn build_url(e: &Entry, unsafe_include: bool) -> String { ... }`
function (now unused; `redact_url` replaces it).

- [ ] **Step 6: Run the curl tests** (the existing `redacts_query_in_url` and
`get_is_safe_and_redacts_auth` now exercise `redact_url`).

Run: `cargo test --lib curl 2>&1 | tail -10`
Expected: PASS (6 tests). `redacts_query_in_url` still passes — `redact_url` keeps
`page=2` and redacts `access_token=leak` (sensitive name). `get_is_safe_and_redacts_auth`
still starts with `curl -X GET 'https://api.x/data'` (no query → URL rebuilt unchanged).

- [ ] **Step 7: Commit**

```bash
git add src/analysis/show_entry.rs src/analysis/curl.rs
git commit -m "feat: route show-entry and curl URLs through redact_url"
```

---

### Task 5: Summary window relabel (Finding 4)

**Files:**
- Modify: `src/analysis/summary.rs`

- [ ] **Step 1: Relabel the window line.** In `src/analysis/summary.rs`, in
`render_summary_text`, change the line:

```rust
    out.push_str(&format!("capture window: {}\n", human_ms(s.duration_ms)));
```
to:
```rust
    out.push_str(&format!(
        "duration (first start to last response): {}\n",
        human_ms(s.duration_ms)
    ));
```

- [ ] **Step 2: Build and verify the summary still renders**

Run: `cargo run --quiet -- tests/fixtures/someapi123.har summary 2>/dev/null | head -4`
Expected: the header block now shows `duration (first start to last response): 72ms`
instead of `capture window: 72ms`.

- [ ] **Step 3: Run the summary unit + CLI tests** (no assertion referenced the old label).

Run: `cargo test --lib summary 2>&1 | tail -5 && cargo test --test cli_summary 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/analysis/summary.rs
git commit -m "fix: clarify summary duration label (Finding 4)"
```

---

### Task 6: Regression fixture + integration test

**Files:**
- Create: `tests/fixtures/secret_in_path.har`
- Create: `tests/cli_redaction.rs`

- [ ] **Step 1: Create the synthetic leak fixture** at `tests/fixtures/secret_in_path.har`. The request path embeds the literal sentinel `FAKEKEY_a1b2c3d4e5f6g7h8` inside a base64-ish opaque segment, and the `Location` response header echoes the full URL:

```json
{
  "log": {
    "version": "1.2",
    "creator": { "name": "test", "version": "1.0" },
    "entries": [
      {
        "startedDateTime": "2026-05-24T15:24:00.000Z",
        "time": 12.0,
        "request": {
          "method": "GET",
          "url": "https://addon.example.com/cfg/eyJrZXkiOiJGQUtFS0VZX2ExYjJjM2Q0ZTVmNmc3aDgifQ==FAKEKEY_a1b2c3d4e5f6g7h8AAAA1234/manifest.json",
          "httpVersion": "HTTP/1.1",
          "cookies": [],
          "headers": [
            { "name": "Accept", "value": "application/json" }
          ],
          "queryString": [],
          "headersSize": -1,
          "bodySize": 0
        },
        "response": {
          "status": 308,
          "statusText": "Permanent Redirect",
          "httpVersion": "HTTP/1.1",
          "cookies": [],
          "headers": [
            { "name": "Location", "value": "https://addon.example.com/cfg/eyJrZXkiOiJGQUtFS0VZX2ExYjJjM2Q0ZTVmNmc3aDgifQ==FAKEKEY_a1b2c3d4e5f6g7h8AAAA1234/manifest.json" }
          ],
          "content": { "size": 0, "mimeType": "application/json" },
          "redirectURL": "https://addon.example.com/cfg/eyJrZXkiOiJGQUtFS0VZX2ExYjJjM2Q0ZTVmNmc3aDgifQ==FAKEKEY_a1b2c3d4e5f6g7h8AAAA1234/manifest.json",
          "headersSize": -1,
          "bodySize": 0
        },
        "cache": {},
        "timings": { "send": 1.0, "wait": 10.0, "receive": 1.0 }
      }
    ]
  }
}
```

- [ ] **Step 2: Write the integration test** in `tests/cli_redaction.rs`:

```rust
use std::process::Command;

const SENTINEL: &str = "FAKEKEY_a1b2c3d4e5f6g7h8";

fn fixture() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/secret_in_path.har")
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn no_command_leaks_path_secret_by_default() {
    let f = fixture();
    let commands: &[&[&str]] = &[
        &[&f, "summary"],
        &[&f, "duplicates"],
        &[&f, "redirects"],
        &[&f, "endpoints"],
        &[&f, "timeline"],
        &[&f, "report"],
        &[&f, "show-entry", "e000000"],
        &[&f, "curl", "e000000"],
    ];
    for args in commands {
        let out = run(args);
        assert!(
            !out.contains(SENTINEL),
            "command {:?} leaked the path secret:\n{out}",
            args
        );
    }
}

#[test]
fn unsafe_flag_reveals_secret_in_show_entry() {
    let f = fixture();
    let out = run(&[&f, "show-entry", "e000000", "--unsafe-include-secrets"]);
    assert!(out.contains(SENTINEL), "unsafe show-entry should reveal the secret:\n{out}");
}
```

- [ ] **Step 3: Run the integration test**

Run: `cargo test --test cli_redaction 2>&1 | tail -12`
Expected: PASS (2 tests). If `no_command_leaks_path_secret_by_default` fails for a
specific command, that command renders the raw URL/redirect somewhere not yet routed
through the redactor — fix that render path. (`redirects` shows `target_host` only,
not the full target URL, so it is safe; `show-entry` shows the redacted `url`.)

- [ ] **Step 4: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures.

- [ ] **Step 5: Verify against the real HAR** (manual confidence check, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
./target/release/wiretrail "/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har" duplicates 2>/dev/null | head -8
./target/release/wiretrail "/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har" show-entry e000009 2>/dev/null | head -3
```
Expected: the addon manifest duplicate lines now show `{blob}` instead of the
500-char base64; `show-entry e000009`'s URL shows `<redacted>` in place of the
config blob. Neither `e2574d74` nor `szpwe4fx4ngs8u9q` should appear.

- [ ] **Step 6: Commit**

```bash
git add tests/fixtures/secret_in_path.har tests/cli_redaction.rs
git commit -m "test: regression fixture + integration test for path-secret redaction"
```

---

## Self-review

**Spec coverage:**
- Finding 1 (path secrets leak) → Task 1 (`is_opaque`), Task 3 (`redact_url`), Task 4 (show-entry/curl), Task 6 (regression lock). ✓
- Finding 2 (opaque segments not normalized) → Task 2 (`{blob}`). ✓
- Finding 3 (newlines in body snippets) → Task 3 Step 6 (`collapse_newlines`). ✓
- Finding 4 (window labeling) → Task 5. ✓
- Finding 5 (header-value secrets) → Task 3 (`redact_header_value` URL/opaque-aware, `redact_value`). ✓
- Shared detector reused by normalize + redact → Tasks 1, 2, 3. ✓
- `{blob}` token distinct from `{id}` → Task 2. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code; every command step states expected output. ✓

**Type consistency:**
- `is_opaque(&str) -> bool`, `is_uuid(&str) -> bool` (Task 1) consumed by `normalize` (Task 2) and `redact` (Task 3). ✓
- `redact_url(&str, bool) -> String`, `redact_value(&str, bool) -> String` (Task 3) used by `redact_header_value` (Task 3), `show_entry` (Task 4), `curl` (Task 4). ✓
- `redact_query_value` keeps its `(&str, &str, bool)` signature (Task 3) — called by `redact_url` and unchanged elsewhere. ✓
- `redact_header_value` keeps its `(&str, &str, bool)` signature — call sites in `show_entry`/`curl` unchanged. ✓
- `entry_detail(&Entry, bool)` (Task 4) unchanged signature; only the `url` field source changes. ✓
- `curl::entry_to_curl` unchanged signature; internal `build_url` removed, `redact_url` substituted. ✓
- `REDACTED` constant (existing) reused as the mask everywhere in `redact`. ✓
- Fixture entry id `e000000` matches `format_entry_id(0)` from the model. ✓

**Note on residual scope (per spec non-goals):** the duplicate *fingerprint* string
includes raw query key=values (not path — `norm_path` is now `{blob}`); a secret in a
query *value* would still appear in a fingerprint display. The regression fixture's
secret is in the path (the reported vector), which is fully covered. Query-value
fingerprint redaction is out of scope for this pass.
