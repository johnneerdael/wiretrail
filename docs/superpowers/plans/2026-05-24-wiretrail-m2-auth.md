# wiretrail M2 — Auth & Token (`jwt`, `auth`, `handoff`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three auth-focused commands — `jwt` (find + decode JWTs, redacted), `auth` (auth-failure + token-refresh analysis), and `handoff` (backend trace-handoff report).

**Architecture:** A new `src/jwt.rs` support module hand-rolls base64url decoding and JWT header/claims summarization (no signature, no new dependency). Three analysis modules consume it / existing helpers, following the established `compute_* → result + render_*_text` pattern, wired through `emit`/`exit`. Absolute timestamps are reconstructed from `cap.meta.start_ms + started_offset_ms` (no `Entry` model change).

**Tech Stack:** Rust 2024, serde/serde_json, chrono, ahash — no new dependencies.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-analysis-expansion-design.md`,
Phase M2. **Plan 2 of 3** (M1 shipped; M3 diff/checks follows).

**Deviation note:** `Entry` stores `started_offset_ms` (f64) but no absolute
timestamp; `handoff` reconstructs an ISO timestamp from `cap.meta.start_ms +
offset` via `chrono::DateTime::from_timestamp_millis`. JWT expiry is evaluated
against that same reconstructed per-entry time.

## Prerequisites (verified present)

- `model::{Capture, Entry}`, `Entry.req_headers/resp_headers/query/req_body/resp_body/host/norm_path/method/status/started_offset_ms/id/correlation/server_ip`, `CaptureMeta.start_ms: Option<i64>`, cfg(test) `sample_entry`/`sample_capture`.
- `analysis::curl::entry_to_curl(&Entry, bool) -> CurlCommand { command, .. }`.
- `filter::Filter`, `Entry::is_error()`, `render::human_ms`, `emit`/`exit` (with the global `--unsafe-include-secrets` flag) in `main.rs`.
- chrono 0.4 (already a dependency).

## File structure

```
src/jwt.rs                  # NEW: base64url decode + JWT decode/summarize (support)
src/lib.rs                  # Modify: pub mod jwt;
src/analysis/mod.rs         # Modify: declare auth, handoff, jwt
src/analysis/jwt.rs         # NEW: find + decode JWTs across an entry
src/analysis/auth.rs        # NEW: auth failures + token refresh
src/analysis/handoff.rs     # NEW: backend trace handoff
src/main.rs                 # Modify: 3 subcommands + dispatch
tests/cli_auth.rs           # NEW: integration tests
```

---

### Task 1: Scaffold modules

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Declare the support module.** In `src/lib.rs`, after `pub mod opaque;`, add:

```rust
pub mod jwt;
```

- [ ] **Step 2: Declare the analysis modules.** Replace the entire contents of `src/analysis/mod.rs` with (alphabetical, adding `auth`, `handoff`, `jwt`):

```rust
pub mod auth;
pub mod curl;
pub mod duplicates;
pub mod endpoints;
pub mod errors;
pub mod handoff;
pub mod hosts;
pub mod jwt;
pub mod pagination;
pub mod rate_limit;
pub mod redirects;
pub mod report;
pub mod retries;
pub mod show_entry;
pub mod slowest;
pub mod storms;
pub mod subsystems;
pub mod summary;
pub mod timeline;
pub mod transitions;
```

- [ ] **Step 3: Create empty files and build.**

Run:
```bash
cd /Users/jneerdael/Scripts/har-rs
touch src/jwt.rs src/analysis/auth.rs src/analysis/handoff.rs src/analysis/jwt.rs
cargo build 2>&1 | tail -4
```
Expected: build SUCCEEDS.

- [ ] **Step 4: Commit**

```bash
git add src/lib.rs src/analysis/mod.rs src/jwt.rs src/analysis/auth.rs src/analysis/handoff.rs src/analysis/jwt.rs
git commit -m "chore: scaffold M2 auth modules"
```

---

### Task 2: `src/jwt.rs` — base64url decode + JWT summarize

**Files:**
- Create: `src/jwt.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/jwt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{base64url_decode, decode_jwt, summarize, token_hash, JwtParts};
    use serde_json::json;

    // jwt.io default token: header {"alg":"HS256","typ":"JWT"},
    // payload {"sub":"1234567890","name":"John Doe","iat":1516239022}
    const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    #[test]
    fn base64url_decodes_header() {
        let bytes = base64url_decode("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9").unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"alg":"HS256","typ":"JWT"}"#);
    }

    #[test]
    fn decodes_jwt_header_and_claims() {
        let parts = decode_jwt(SAMPLE).unwrap();
        assert_eq!(parts.header.get("alg").unwrap(), "HS256");
        assert_eq!(parts.claims.get("iat").unwrap().as_i64().unwrap(), 1516239022);
    }

    #[test]
    fn rejects_non_jwt() {
        assert!(decode_jwt("not.a.jwt").is_none());
        assert!(decode_jwt("only.twoparts").is_none());
    }

    #[test]
    fn summary_redacts_sub_and_flags_expiry() {
        let parts = JwtParts {
            header: json!({"alg": "RS256", "typ": "JWT"}),
            claims: json!({"iss": "acme", "sub": "secret-user", "exp": 1000, "iat": 100}),
        };
        // reference time = 2000s -> exp 1000s is in the past -> expired
        let s = summarize(&parts, Some(2_000_000));
        assert_eq!(s.iss.as_deref(), Some("acme"));
        assert_eq!(s.expired, Some(true));
        assert_eq!(s.seconds_to_expiry, Some(-1000));
        // sub is hashed, never raw
        assert!(s.sub_hash.is_some());
        assert_ne!(s.sub_hash.as_deref(), Some("secret-user"));
    }

    #[test]
    fn summary_detects_future_iat_skew() {
        let parts = JwtParts {
            header: json!({"alg": "HS256"}),
            claims: json!({"iat": 5000}),
        };
        // reference time = 1000s; iat 5000s is far in the future
        let s = summarize(&parts, Some(1_000_000));
        assert!(s.clock_skew_hint.is_some());
    }

    #[test]
    fn token_hash_is_stable_and_not_raw() {
        let h = token_hash(SAMPLE);
        assert_eq!(h, token_hash(SAMPLE));
        assert!(!h.contains("eyJ"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib jwt:: 2>&1 | tail -12`
Expected: FAIL with "cannot find function `base64url_decode`" (etc.).

- [ ] **Step 3: Implement** above the test module in `src/jwt.rs`:

```rust
use serde::Serialize;
use serde_json::Value;

/// Decode url-safe base64 (no padding required). Returns None on invalid input.
pub fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::new();
    for c in s.bytes() {
        let v: u32 = match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'-' => 62,
            b'_' => 63,
            b'=' => break, // padding
            _ => return None,
        };
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    Some(out)
}

#[derive(Debug, Clone)]
pub struct JwtParts {
    pub header: Value,
    pub claims: Value,
}

/// Decode a `header.payload.signature` JWT into header + claims JSON.
/// The signature is ignored (never decoded or returned).
pub fn decode_jwt(token: &str) -> Option<JwtParts> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let header: Value = serde_json::from_slice(&base64url_decode(parts[0])?).ok()?;
    let claims: Value = serde_json::from_slice(&base64url_decode(parts[1])?).ok()?;
    Some(JwtParts { header, claims })
}

#[derive(Debug, Clone, Serialize)]
pub struct JwtSummary {
    pub alg: Option<String>,
    pub typ: Option<String>,
    pub iss: Option<String>,
    pub aud: Option<String>,
    pub sub_hash: Option<String>,
    pub iat: Option<i64>,
    pub nbf: Option<i64>,
    pub exp: Option<i64>,
    pub expired: Option<bool>,
    pub seconds_to_expiry: Option<i64>,
    pub clock_skew_hint: Option<String>,
}

/// Summarize a JWT's header/claims, redacting `sub` to a hash and computing
/// expiry/skew against `ref_epoch_ms` (the using request's reconstructed time).
pub fn summarize(parts: &JwtParts, ref_epoch_ms: Option<i64>) -> JwtSummary {
    let h = &parts.header;
    let c = &parts.claims;
    let get_str = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    let get_i64 = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_i64());

    let aud = c.get("aud").map(|a| match a {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    });
    let sub_hash = c.get("sub").and_then(|s| s.as_str()).map(token_hash);
    let iat = get_i64(c, "iat");
    let nbf = get_i64(c, "nbf");
    let exp = get_i64(c, "exp");

    let (expired, seconds_to_expiry) = match (exp, ref_epoch_ms) {
        (Some(e), Some(r)) => (Some(e * 1000 < r), Some(e - r / 1000)),
        _ => (None, None),
    };
    let clock_skew_hint = match (iat, ref_epoch_ms) {
        (Some(i), Some(r)) if i * 1000 > r + 60_000 => {
            Some("token iat is in the future (clock skew?)".to_string())
        }
        _ => None,
    };

    JwtSummary {
        alg: get_str(h, "alg"),
        typ: get_str(h, "typ"),
        iss: get_str(c, "iss"),
        aud,
        sub_hash,
        iat,
        nbf,
        exp,
        expired,
        seconds_to_expiry,
        clock_skew_hint,
    }
}

/// Stable, non-reversible short hash of a token/value (for grouping + sub redaction).
pub fn token_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib jwt:: 2>&1 | tail -12`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/jwt.rs
git commit -m "feat: JWT decode + summarize support module (hand-rolled base64url)"
```

---

### Task 3: `jwt` command

**Files:**
- Create: `src/analysis/jwt.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/jwt.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_jwt;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    fn with_bearer(index: usize) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/me", 200);
        e.req_headers = vec![("Authorization".to_string(), format!("Bearer {SAMPLE}"))];
        e
    }

    #[test]
    fn finds_and_decodes_bearer_jwt() {
        let cap = sample_capture(vec![with_bearer(0), with_bearer(1)]);
        let r = compute_jwt(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        assert_eq!(r.tokens.len(), 1);
        let t = &r.tokens[0];
        assert_eq!(t.occurrences, 2);
        assert_eq!(t.source, "req.header.authorization");
        assert_eq!(t.summary.iat, Some(1516239022));
        assert!(t.raw_token.is_none()); // redacted by default
    }

    #[test]
    fn unsafe_includes_raw_token() {
        let cap = sample_capture(vec![with_bearer(0)]);
        let r = compute_jwt(&cap, &Filter::parse(&[]).unwrap(), 10, true);
        assert_eq!(r.tokens[0].raw_token.as_deref(), Some(SAMPLE));
    }

    #[test]
    fn finds_jwt_in_body() {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 200);
        e.resp_body = Some(format!(r#"{{"access_token":"{SAMPLE}"}}"#));
        let r = compute_jwt(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), 10, false);
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].source, "resp.body");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::jwt 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_jwt`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/jwt.rs`:

```rust
use crate::filter::Filter;
use crate::jwt::{decode_jwt, summarize, token_hash, JwtSummary};
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct JwtResult {
    pub tokens: Vec<JwtOccurrence>,
}

#[derive(Debug, Serialize)]
pub struct JwtOccurrence {
    pub token_hash: String,
    pub source: String,
    pub summary: JwtSummary,
    pub occurrences: usize,
    pub first_entry_id: String,
    pub last_entry_id: String,
    pub raw_token: Option<String>,
}

struct Acc {
    source: String,
    first_id: String,
    last_id: String,
    count: usize,
    ref_ms: Option<i64>,
}

/// Find and decode JWTs across headers, query, cookies, and bodies.
pub fn compute_jwt(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> JwtResult {
    let mut map: AHashMap<String, Acc> = AHashMap::new();

    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let ref_ms = cap.meta.start_ms.map(|s| s + e.started_offset_ms as i64);
        for (token, source) in scan_entry(e) {
            let acc = map.entry(token).or_insert_with(|| Acc {
                source,
                first_id: e.id.clone(),
                last_id: e.id.clone(),
                count: 0,
                ref_ms,
            });
            acc.count += 1;
            acc.last_id = e.id.clone();
        }
    }

    let mut tokens: Vec<JwtOccurrence> = map
        .into_iter()
        .filter_map(|(token, acc)| {
            let parts = decode_jwt(&token)?;
            let summary = summarize(&parts, acc.ref_ms);
            Some(JwtOccurrence {
                token_hash: token_hash(&token),
                source: acc.source,
                summary,
                occurrences: acc.count,
                first_entry_id: acc.first_id,
                last_entry_id: acc.last_id,
                raw_token: if unsafe_include { Some(token) } else { None },
            })
        })
        .collect();

    tokens.sort_by(|a, b| {
        let ax = a.summary.expired == Some(true);
        let bx = b.summary.expired == Some(true);
        bx.cmp(&ax)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.token_hash.cmp(&b.token_hash))
    });
    tokens.truncate(top);
    JwtResult { tokens }
}

/// Scan an entry's headers, query, and bodies for JWTs; returns (token, source).
fn scan_entry(e: &Entry) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (n, v) in &e.req_headers {
        for t in scan_jwts(v) {
            found.push((t, format!("req.header.{}", n.to_ascii_lowercase())));
        }
    }
    for (n, v) in &e.resp_headers {
        for t in scan_jwts(v) {
            found.push((t, format!("resp.header.{}", n.to_ascii_lowercase())));
        }
    }
    for (k, v) in &e.query {
        for t in scan_jwts(v) {
            found.push((t, format!("query.{k}")));
        }
    }
    if let Some(b) = &e.req_body {
        for t in scan_jwts(b) {
            found.push((t, "req.body".to_string()));
        }
    }
    if let Some(b) = &e.resp_body {
        for t in scan_jwts(b) {
            found.push((t, "resp.body".to_string()));
        }
    }
    found
}

/// Extract decodable JWT substrings from free text (tokenize on non-JWT chars).
fn scan_jwts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cand in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')) {
        if cand.matches('.').count() == 2 && cand.len() >= 20 && decode_jwt(cand).is_some() {
            out.push(cand.to_string());
        }
    }
    out
}

/// Render JWTs as deterministic terminal text.
pub fn render_jwt_text(r: &JwtResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail jwt ==\n");
    for t in &r.tokens {
        let exp = match t.summary.expired {
            Some(true) => " [EXPIRED]",
            Some(false) => "",
            None => "",
        };
        out.push_str(&format!(
            "\n{} ({}x, {}){}\n",
            t.token_hash, t.occurrences, t.source, exp
        ));
        if let Some(iss) = &t.summary.iss {
            out.push_str(&format!("  iss: {iss}\n"));
        }
        if let Some(aud) = &t.summary.aud {
            out.push_str(&format!("  aud: {aud}\n"));
        }
        if let Some(sub) = &t.summary.sub_hash {
            out.push_str(&format!("  sub (hashed): {sub}\n"));
        }
        if let Some(exp) = t.summary.exp {
            out.push_str(&format!(
                "  exp: {} ({})\n",
                exp,
                match t.summary.seconds_to_expiry {
                    Some(s) if s < 0 => format!("expired {}s ago", -s),
                    Some(s) => format!("{s}s left"),
                    None => "unknown".to_string(),
                }
            ));
        }
        if let Some(hint) = &t.summary.clock_skew_hint {
            out.push_str(&format!("  warning: {hint}\n"));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::jwt 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/jwt.rs
git commit -m "feat: jwt command (find + decode JWTs, redacted)"
```

---

### Task 4: `auth` command

**Files:**
- Create: `src/analysis/auth.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_auth;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn with_auth(index: usize, host: &str, path: &str, status: i64, auth: Option<&str>, offset: f64) -> Entry {
        let mut e = sample_entry(index, host, "GET", path, status);
        e.started_offset_ms = offset;
        if let Some(a) = auth {
            e.req_headers = vec![("Authorization".to_string(), a.to_string())];
        } else {
            e.req_headers = vec![];
        }
        e
    }

    #[test]
    fn groups_401_failures() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/me", 401, Some("Bearer a"), 0.0),
            with_auth(1, "api.x", "/me", 401, Some("Bearer a"), 10.0),
            with_auth(2, "api.x", "/ok", 200, Some("Bearer a"), 20.0),
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        let f = r.failures.iter().find(|f| f.norm_path == "/me").unwrap();
        assert_eq!(f.count, 2);
        assert_eq!(f.status, 401);
    }

    #[test]
    fn flags_host_missing_auth_inconsistently() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/a", 200, Some("Bearer a"), 0.0),
            with_auth(1, "api.x", "/b", 200, None, 10.0),
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.missing_auth_hosts.contains(&"api.x".to_string()));
    }

    #[test]
    fn detects_old_token_reuse_after_refresh() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/data", 200, Some("Bearer OLD"), 0.0),
            // refresh call (path contains /token + grant_type query)
            {
                let mut e = sample_entry(1, "auth.x", "POST", "/auth/v1/token", 200);
                e.started_offset_ms = 100.0;
                e.query = vec![("grant_type".to_string(), "refresh_token".to_string())];
                e
            },
            with_auth(2, "api.x", "/data", 200, Some("Bearer OLD"), 200.0), // reuses old
            with_auth(3, "api.x", "/data", 200, Some("Bearer NEW"), 300.0), // new token seen
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.refreshes.len(), 1);
        let rf = &r.refreshes[0];
        assert!(rf.success);
        assert!(rf.old_token_reused);
        assert!(rf.reusing_ids.contains(&"e000002".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::auth 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_auth`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/auth.rs`:

```rust
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuthResult {
    pub failures: Vec<AuthFailure>,
    pub missing_auth_hosts: Vec<String>,
    pub token_changes: Vec<TokenChange>,
    pub refreshes: Vec<RefreshEvent>,
}

#[derive(Debug, Serialize)]
pub struct AuthFailure {
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenChange {
    pub host: String,
    pub distinct_tokens: usize,
}

#[derive(Debug, Serialize)]
pub struct RefreshEvent {
    pub id: String,
    pub host: String,
    pub status: i64,
    pub success: bool,
    pub concurrent: bool,
    pub old_token_reused: bool,
    pub reusing_ids: Vec<String>,
}

fn auth_value(e: &Entry) -> Option<&str> {
    e.req_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str())
}

fn is_refresh(e: &Entry) -> bool {
    let p = e.norm_path.to_ascii_lowercase();
    let path_hit = p.contains("/token") || p.contains("/oauth") || p.contains("/auth/refresh");
    let query_hit = e
        .query
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("grant_type") && v == "refresh_token");
    path_hit || query_hit
}

/// Analyze auth failures, missing/rotating auth, and token-refresh flows.
pub fn compute_auth(cap: &Capture, filter: &Filter, top: usize) -> AuthResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    // --- failures (401/403) ---
    let mut fail_map: AHashMap<(String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        if e.status == 401 || e.status == 403 {
            fail_map
                .entry((e.host.clone(), e.norm_path.clone(), e.status))
                .or_default()
                .push(e);
        }
    }
    let mut failures: Vec<AuthFailure> = fail_map
        .into_iter()
        .map(|((host, norm_path, status), g)| AuthFailure {
            host,
            norm_path,
            status,
            count: g.len(),
            entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        })
        .collect();
    failures.sort_by(|a, b| b.count.cmp(&a.count).then(a.host.cmp(&b.host)).then(a.norm_path.cmp(&b.norm_path)));
    failures.truncate(top);

    // --- per-host auth presence + distinct tokens ---
    let mut host_has_auth: AHashMap<String, bool> = AHashMap::new();
    let mut host_no_auth: AHashMap<String, bool> = AHashMap::new();
    let mut host_tokens: AHashMap<String, AHashSet<String>> = AHashMap::new();
    for e in &entries {
        match auth_value(e) {
            Some(a) => {
                *host_has_auth.entry(e.host.clone()).or_default() = true;
                host_tokens.entry(e.host.clone()).or_default().insert(a.to_string());
            }
            None => {
                *host_no_auth.entry(e.host.clone()).or_default() = true;
            }
        }
    }
    let mut missing_auth_hosts: Vec<String> = host_has_auth
        .keys()
        .filter(|h| host_no_auth.get(*h).copied().unwrap_or(false))
        .cloned()
        .collect();
    missing_auth_hosts.sort();

    let mut token_changes: Vec<TokenChange> = host_tokens
        .into_iter()
        .filter(|(_, set)| set.len() > 1)
        .map(|(host, set)| TokenChange { host, distinct_tokens: set.len() })
        .collect();
    token_changes.sort_by(|a, b| b.distinct_tokens.cmp(&a.distinct_tokens).then(a.host.cmp(&b.host)));

    // --- token refresh flows ---
    // Global timeline of (offset, auth value) for reuse analysis.
    let mut auth_timeline: Vec<(f64, String, String)> = entries
        .iter()
        .filter_map(|e| auth_value(e).map(|a| (e.started_offset_ms, a.to_string(), e.id.clone())))
        .collect();
    auth_timeline.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let refresh_entries: Vec<&&Entry> = entries.iter().filter(|e| is_refresh(e)).collect();
    let mut refreshes: Vec<RefreshEvent> = Vec::new();
    for rf in &refresh_entries {
        let t = rf.started_offset_ms;
        let success = (200..300).contains(&rf.status);

        let pre: AHashSet<&String> = auth_timeline
            .iter()
            .filter(|(o, _, _)| *o < t)
            .map(|(_, a, _)| a)
            .collect();
        let new_token_seen = auth_timeline
            .iter()
            .any(|(o, a, _)| *o > t && !pre.contains(a));
        let reusing_ids: Vec<String> = auth_timeline
            .iter()
            .filter(|(o, a, _)| *o > t && pre.contains(a))
            .map(|(_, _, id)| id.clone())
            .collect();
        let old_token_reused = success && new_token_seen && !reusing_ids.is_empty();

        let concurrent = refresh_entries.iter().any(|other| {
            other.id != rf.id && (other.started_offset_ms - t).abs() < rf.duration_ms.max(1.0)
        });

        refreshes.push(RefreshEvent {
            id: rf.id.clone(),
            host: rf.host.clone(),
            status: rf.status,
            success,
            concurrent,
            old_token_reused,
            reusing_ids,
        });
    }
    refreshes.sort_by(|a, b| a.id.cmp(&b.id));

    AuthResult {
        failures,
        missing_auth_hosts,
        token_changes,
        refreshes,
    }
}

/// Render auth analysis as deterministic terminal text.
pub fn render_auth_text(r: &AuthResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail auth ==\n");
    if !r.failures.is_empty() {
        out.push_str("\nauth failures:\n");
        for f in &r.failures {
            out.push_str(&format!("  {}x [{}] {} {}\n", f.count, f.status, f.host, f.norm_path));
        }
    }
    if !r.missing_auth_hosts.is_empty() {
        out.push_str(&format!(
            "\nhosts with inconsistent Authorization: {}\n",
            r.missing_auth_hosts.join(", ")
        ));
    }
    if !r.token_changes.is_empty() {
        out.push_str("\ntoken rotation:\n");
        for t in &r.token_changes {
            out.push_str(&format!("  {} ({} distinct tokens)\n", t.host, t.distinct_tokens));
        }
    }
    if !r.refreshes.is_empty() {
        out.push_str("\ntoken refreshes:\n");
        for rf in &r.refreshes {
            let mut tags = Vec::new();
            if !rf.success {
                tags.push("failed".to_string());
            }
            if rf.old_token_reused {
                tags.push("old-token-reused".to_string());
            }
            if rf.concurrent {
                tags.push("concurrent".to_string());
            }
            let tagstr = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) };
            out.push_str(&format!("  {} {} [{}]{}\n", rf.id, rf.host, rf.status, tagstr));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::auth 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/auth.rs
git commit -m "feat: auth command (failures + token-refresh analysis)"
```

---

### Task 5: `handoff` command

**Files:**
- Create: `src/analysis/handoff.rs`

- [ ] **Step 1: Write the failing tests** at the top of `src/analysis/handoff.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_handoff;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn emits_block_for_failed_request() {
        let mut e = sample_entry(0, "api.x", "POST", "/bulk", 500);
        e.correlation = vec![("x-request-id".to_string(), "abc-123".to_string())];
        e.server_ip = Some("10.0.0.1".to_string());
        let cap = sample_capture(vec![e, sample_entry(1, "api.x", "GET", "/ok", 200)]);
        let r = compute_handoff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        // the 500 is included; the 200 is not an error and not top-slow enough to matter,
        // but top-N slowest may include it. The failed one must be present:
        let item = r.items.iter().find(|i| i.id == "e000000").unwrap();
        assert_eq!(item.status, 500);
        assert!(item.curl.contains("curl -X POST"));
        assert_eq!(item.correlation_ids, vec!["abc-123".to_string()]);
        assert_eq!(item.server_ip.as_deref(), Some("10.0.0.1"));
    }

    #[test]
    fn redacts_curl_by_default() {
        let mut e = sample_entry(0, "api.x", "GET", "/x", 500);
        e.req_headers = vec![("Authorization".to_string(), "Bearer secret".to_string())];
        let r = compute_handoff(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), 10, false);
        assert!(!r.items[0].curl.contains("Bearer secret"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib analysis::handoff 2>&1 | tail -12`
Expected: FAIL with "cannot find function `compute_handoff`".

- [ ] **Step 3: Implement** above the test module in `src/analysis/handoff.rs`:

```rust
use crate::analysis::curl::entry_to_curl;
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::AHashSet;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HandoffResult {
    pub items: Vec<HandoffItem>,
}

#[derive(Debug, Serialize)]
pub struct HandoffItem {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub timestamp: Option<String>,
    pub offset_ms: f64,
    pub correlation_ids: Vec<String>,
    pub server_ip: Option<String>,
    pub curl: String,
}

fn abs_iso(start_ms: Option<i64>, offset_ms: f64) -> Option<String> {
    let ms = start_ms? + offset_ms as i64;
    chrono::DateTime::from_timestamp_millis(ms).map(|dt| dt.to_rfc3339())
}

/// Backend trace-handoff blocks for every failed request and the top-N slowest.
pub fn compute_handoff(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> HandoffResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    // top-N slowest
    let mut by_dur: Vec<&Entry> = entries.clone();
    by_dur.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let slow_ids: AHashSet<String> = by_dur.iter().take(top).map(|e| e.id.clone()).collect();

    let mut selected: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.is_error() || slow_ids.contains(&e.id))
        .copied()
        .collect();
    selected.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    let items = selected
        .iter()
        .map(|e| HandoffItem {
            id: e.id.clone(),
            method: e.method.to_ascii_uppercase(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            timestamp: abs_iso(cap.meta.start_ms, e.started_offset_ms),
            offset_ms: e.started_offset_ms,
            correlation_ids: e.correlation.iter().map(|(_, v)| v.clone()).collect(),
            server_ip: e.server_ip.clone(),
            curl: entry_to_curl(e, unsafe_include).command,
        })
        .collect();

    HandoffResult { items }
}

/// Render handoff blocks as deterministic terminal text.
pub fn render_handoff_text(r: &HandoffResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail handoff ==\n");
    for i in &r.items {
        out.push_str(&format!(
            "\n# {} [{}] {} {}{}\n",
            i.id, i.status, i.method, i.host, i.norm_path
        ));
        if let Some(ts) = &i.timestamp {
            out.push_str(&format!("  time: {ts} (+{}ms)\n", i.offset_ms as i64));
        }
        if !i.correlation_ids.is_empty() {
            out.push_str(&format!("  correlation: {}\n", i.correlation_ids.join(", ")));
        }
        if let Some(ip) = &i.server_ip {
            out.push_str(&format!("  server ip: {ip}\n"));
        }
        out.push_str(&format!("  {}\n", i.curl.replace('\n', "\n  ")));
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib analysis::handoff 2>&1 | tail -10`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/handoff.rs
git commit -m "feat: handoff command (backend trace handoff report)"
```

---

### Task 6: Wire the three commands into the CLI

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports.** In `src/main.rs`, directly below the existing `use har::analysis::rate_limit::...;` line, add:

```rust
use har::analysis::jwt::{compute_jwt, render_jwt_text};
use har::analysis::auth::{compute_auth, render_auth_text};
use har::analysis::handoff::{compute_handoff, render_handoff_text};
```

- [ ] **Step 2: Add the subcommand variants.** Inside `enum Command { ... }`, after the `RateLimit,` variant, add:

```rust
    /// Find and decode JWTs (redacted: no signature, hashed sub).
    Jwt,
    /// Auth failures (401/403), inconsistent auth, and token-refresh flows.
    Auth,
    /// Backend trace-handoff blocks for failed + slowest requests.
    Handoff,
```

- [ ] **Step 3: Add the dispatch arms.** Inside the `match cli.command.unwrap_or(Command::Summary) { ... }` block, after the `Command::RateLimit => { ... }` arm, add:

```rust
        Command::Jwt => {
            let result = compute_jwt(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = result.tokens.iter().any(|t| t.summary.expired == Some(true));
            emit(
                cli.json,
                "jwt",
                &cap.meta,
                &result,
                &render_jwt_text(&result),
                &["auth", "show-entry", "errors"],
            );
            exit(findings);
        }
        Command::Auth => {
            let result = compute_auth(&cap, &filter, cli.top);
            let findings = !result.failures.is_empty()
                || !result.missing_auth_hosts.is_empty()
                || result.refreshes.iter().any(|r| !r.success || r.old_token_reused || r.concurrent);
            emit(
                cli.json,
                "auth",
                &cap.meta,
                &result,
                &render_auth_text(&result),
                &["jwt", "transitions", "errors"],
            );
            exit(findings);
        }
        Command::Handoff => {
            let result = compute_handoff(&cap, &filter, cli.top, cli.unsafe_include_secrets);
            let findings = !result.items.is_empty();
            emit(
                cli.json,
                "handoff",
                &cap.meta,
                &result,
                &render_handoff_text(&result),
                &["errors", "slowest", "curl"],
            );
            exit(findings);
        }
```

- [ ] **Step 4: Build**

Run: `cargo build 2>&1 | tail -8`
Expected: SUCCESS.

- [ ] **Step 5: Manual smokes**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har jwt
cargo run --quiet -- tests/fixtures/someapi123.har auth --json | head -6
cargo run --quiet -- tests/fixtures/someapi123.har handoff
```
Expected: `jwt` prints its header (the fixture has no JWT → no tokens); `auth --json`
prints an envelope with `"command": "auth"`; `handoff` prints its header (no errors
in the single-entry fixture, though the lone entry may appear as top-slowest).

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire jwt/auth/handoff commands into CLI"
```

---

### Task 7: Integration tests + real-HAR check

**Files:**
- Create: `tests/cli_auth.rs`

- [ ] **Step 1: Write the tests** in `tests/cli_auth.rs`:

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
fn jwt_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "jwt", "--json"]);
    assert!(stdout.contains("\"command\": \"jwt\""));
    assert!(stdout.contains("\"tokens\""));
}

#[test]
fn auth_json_envelope() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "auth", "--json"]);
    assert!(stdout.contains("\"command\": \"auth\""));
    assert!(stdout.contains("\"failures\""));
    assert!(stdout.contains("\"refreshes\""));
}

#[test]
fn handoff_text_header() {
    let (stdout, _) = run(&[&fixture("someapi123.har"), "handoff"]);
    assert!(stdout.contains("== wiretrail handoff =="));
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test --test cli_auth 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 3: Run the full suite**

Run: `cargo test 2>&1 | grep -E "test result"`
Expected: all suites PASS, 0 failures.

- [ ] **Step 4: Real-HAR confidence check** (manual, not committed):

Run:
```bash
cargo build --release 2>&1 | tail -1
HAR="/Users/jneerdael/Downloads/HTTPToolkit_2026-05-24_15-24.har"
./target/release/wiretrail "$HAR" auth 2>/dev/null | head -20
./target/release/wiretrail "$HAR" jwt 2>/dev/null | head -16
```
Expected: `auth` surfaces the Supabase 401 (`sync_pull_profiles`) and the
`/auth/v1/token?grant_type=refresh_token` refresh event; `jwt` decodes any bearer
tokens present (Supabase JWTs), showing `iss`/`exp`/expiry — with `sub` hashed and
no raw token unless `--unsafe-include-secrets`.

- [ ] **Step 5: Commit**

```bash
git add tests/cli_auth.rs
git commit -m "test: end-to-end tests for jwt/auth/handoff"
```

---

## Self-review

**Spec coverage (Phase M2):**
- `jwt` (#20) — scan headers/query/cookies/bodies, base64url decode, redacted summary (hashed sub, no signature/raw token by default), expiry/skew → Tasks 2, 3. ✓
- `auth` (#19) — 401/403 grouping, inconsistent-auth hosts, token rotation → Task 4. ✓
- token refresh (#21) — refresh detection, old-token-reuse, concurrent, failed → Task 4. ✓
- `handoff` (#16) — failed + slowest blocks with method/template/status/timestamp/correlation/server-ip/sanitized cURL → Task 5. ✓
- All: `--json`, filter, `--top`, next_commands, findings exit codes; `jwt`/`handoff` honor `--unsafe-include-secrets` → Task 6. ✓
- No new dependencies (base64url hand-rolled in `src/jwt.rs`) → Task 2. ✓

**Placeholder scan:** No TBD/TODO; every code step complete; every command step states expected output. ✓

**Type consistency:**
- `base64url_decode(&str) -> Option<Vec<u8>>`, `decode_jwt(&str) -> Option<JwtParts>` (pub `header`/`claims`), `summarize(&JwtParts, Option<i64>) -> JwtSummary`, `token_hash(&str) -> String` (Task 2) used in `analysis::jwt` (Task 3). ✓
- `JwtSummary` (Serialize) embedded in `JwtOccurrence` (Task 3). ✓
- `compute_jwt(&Capture,&Filter,usize,bool)`, `compute_auth(&Capture,&Filter,usize)`, `compute_handoff(&Capture,&Filter,usize,bool)` and their `render_*_text` — Task 6 dispatch passes matching args (jwt/handoff take `unsafe_include_secrets`; auth does not). ✓
- `entry_to_curl(&Entry, bool) -> CurlCommand`; `.command` field used in `handoff` (Task 5). ✓
- `cap.meta.start_ms: Option<i64>` used for `abs_iso` (Task 5) and JWT ref time (Task 3). ✓
- `RefreshEvent` fields `{success, old_token_reused, concurrent, reusing_ids}` referenced in Task 4 test + Task 6 findings. ✓
- Result structs derive `Serialize`; `emit`/`exit` reused unchanged. ✓
- `Entry` fields used (`req_headers`,`resp_headers`,`query`,`req_body`,`resp_body`,`host`,`norm_path`,`method`,`status`,`started_offset_ms`,`duration_ms`,`id`,`index`,`correlation`,`server_ip`) all exist; `Entry::is_error()` exists. ✓
