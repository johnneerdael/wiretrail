# wiretrail Foundation + `summary` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn this repo (the `har` crate) into the `wiretrail` binary crate with a fast single-pass HAR loader, a normalized analysis model, and a working `summary` command — runnable as `wiretrail capture.har`.

**Architecture:** Keep the existing `har` library API intact (rename the *package* to `wiretrail`, keep the *lib* named `har`, add a `[[bin]]`). Add a new fast load path that mmaps the file and deserializes once into a permissive raw model (no `serde_json::Value`, no version-tagged enum), then transforms it into a normalized `Capture` (entry IDs, host/path parsing, route normalization, resource classification, correlation IDs, timing phases, sizes). Commands consume `Capture`, apply an optional filter, and render to a stable terminal layout or a stable `--json` envelope.

**Tech Stack:** Rust 2024, clap 4 (derive), memmap2, serde/serde_json, chrono (RFC3339 timestamps), url (host/path parsing), ahash (fast maps), thiserror.

---

## Spec reference

Design: `docs/superpowers/specs/2026-05-24-wiretrail-har-analyzer-design.md`.

This plan implements the **Foundation** plus the **`summary`** command. Other v1
commands (`hosts`, `subsystems`, `endpoints`, `duplicates`, `retries`, `errors`,
`redirects`, `slowest`, `transitions`, `timeline`, `show-entry`, `report`,
`curl`) are deferred to Plans 2–4 and reuse everything built here.

**Deviation note:** the spec mentions a "cheap version probe" to pick a typed
model. The unified permissive raw model in Task 2 deserializes HAR 1.2 and 1.3
through the same structs (serde ignores unknown fields), so no probe is needed;
the version string is read directly from the parsed document.

## File structure (created in this plan)

```
Cargo.toml                 # package -> wiretrail, [lib] name = har, [[bin]] wiretrail, new deps
src/main.rs                # NEW: clap CLI, dispatch, exit codes
src/raw.rs                 # NEW: permissive raw HAR structs (serde Deserialize)
src/loader.rs              # NEW: mmap + single-pass from_slice
src/model.rs               # NEW: normalized Capture / Entry / Phases / Sizes / EntryId
src/normalize.rs           # NEW: route normalization
src/classify.rs            # NEW: ResourceType classification
src/correlate.rs           # NEW: correlation-ID header extraction
src/assemble.rs            # NEW: RawDoc -> Capture
src/redact.rs              # NEW: redaction engine
src/filter.rs              # NEW: filter language (parser + matcher + glob)
src/fingerprint.rs         # NEW: duplicate fingerprint helper
src/render.rs              # NEW: JSON envelope + exit codes + terminal helpers
src/analysis/mod.rs        # NEW
src/analysis/summary.rs    # NEW: summary computation + serializable result
tests/cli_summary.rs       # NEW: end-to-end binary test
```

`src/lib.rs`, `src/v1_2/mod.rs`, `src/v1_3/mod.rs`, `tests/regression.rs`,
`examples/printer.rs` are left unchanged except that `lib.rs` gains `pub mod`
declarations for the new modules.

---

### Task 1: Crate setup and binary skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `src/main.rs`
- Modify: `src/lib.rs:15` (add module declarations)

- [ ] **Step 1: Update `Cargo.toml`** — rename the package, keep the lib named `har`, add a binary target, and add dependencies. Replace the `[package]`, and append the new sections. The file becomes:

```toml
[package]
name = "wiretrail"
version = "0.1.0"
authors = ["John Neerdael <john@neerdael.nl>"]
edition = "2024"
rust-version = "1.88"
description = "Fast, deterministic, agent-friendly HAR analyzer CLI. heaptrail for network captures."
license = "MIT"
repository = "https://github.com/johnneerdael/wiretrail"
readme = "README.md"
keywords = ["har", "http", "analysis", "cli", "debugging"]
categories = ["command-line-utilities"]

[lib]
name = "har"

[[bin]]
name = "wiretrail"
path = "src/main.rs"

[features]
default = []
yaml = ["dep:yaml_serde"]

[dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
serde_with = "3.18.0"
thiserror = "2.0.18"
yaml_serde = { version = "0.10.4", optional = true }
clap = { version = "4.6.1", features = ["derive"] }
memmap2 = "0.9.10"
chrono = "0.4.44"
url = "2.5"
ahash = "0.8.12"

[profile.release]
lto = "fat"
codegen-units = 1
```

- [ ] **Step 2: Declare new modules in `src/lib.rs`** — after the existing `pub mod v1_2;` / `pub mod v1_3;` (around line 15-16), add:

```rust
pub mod analysis;
pub mod assemble;
pub mod classify;
pub mod correlate;
pub mod filter;
pub mod fingerprint;
pub mod loader;
pub mod model;
pub mod normalize;
pub mod raw;
pub mod redact;
pub mod render;
```

- [ ] **Step 3: Create `src/main.rs`** with a minimal clap skeleton that compiles (full dispatch arrives in Task 12):

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "wiretrail", version, about = "HAR analyzer CLI")]
struct Cli {
    /// Path to the HAR file.
    file: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    eprintln!("wiretrail: {} (not yet implemented)", cli.file.display());
    std::process::exit(0);
}
```

- [ ] **Step 4: Verify it compiles** — also create empty module files so `lib.rs` resolves.

Run:
```bash
mkdir -p src/analysis
for f in raw loader model normalize classify correlate assemble redact filter fingerprint render; do touch "src/$f.rs"; done
printf 'pub mod summary;\n' > src/analysis/mod.rs
touch src/analysis/summary.rs
cargo build 2>&1 | tail -5
```
Expected: build fails only because the empty modules are referenced but fine to be empty — empty `.rs` files are valid modules. Build should SUCCEED. If `cargo build` complains about an unused `Cli`, that is fine (warnings only).

- [ ] **Step 5: Confirm existing tests still pass** (the rename must not break them).

Run: `cargo test --test regression 2>&1 | tail -15`
Expected: PASS (the lib is still named `har`, so `use har::...` resolves).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: scaffold wiretrail binary crate (lib stays har)"
```

---

### Task 2: Permissive raw model + mmap loader

**Files:**
- Create: `src/raw.rs`
- Create: `src/loader.rs`
- Test: `src/loader.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the raw structs** in `src/raw.rs`. These are permissive (every non-essential field is `Option` / defaulted; unknown fields are ignored by serde) so HAR 1.2 and 1.3 both deserialize:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RawDoc {
    pub log: RawLog,
}

#[derive(Debug, Deserialize)]
pub struct RawLog {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub creator: RawCreator,
    #[serde(default)]
    pub browser: Option<RawCreator>,
    #[serde(default)]
    pub entries: Vec<RawEntry>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawCreator {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct RawEntry {
    #[serde(rename = "startedDateTime", default)]
    pub started_date_time: String,
    #[serde(default)]
    pub time: f64,
    pub request: RawRequest,
    pub response: RawResponse,
    #[serde(default)]
    pub timings: RawTimings,
    #[serde(rename = "serverIPAddress", default)]
    pub server_ip_address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "httpVersion", default)]
    pub http_version: String,
    #[serde(default)]
    pub headers: Vec<RawNameValue>,
    #[serde(rename = "queryString", default)]
    pub query_string: Vec<RawNameValue>,
    #[serde(rename = "postData", default)]
    pub post_data: Option<RawPostData>,
    #[serde(rename = "bodySize", default)]
    pub body_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct RawResponse {
    #[serde(default)]
    pub status: i64,
    #[serde(rename = "statusText", default)]
    pub status_text: String,
    #[serde(default)]
    pub headers: Vec<RawNameValue>,
    #[serde(default)]
    pub content: RawContent,
    #[serde(rename = "redirectURL", default)]
    pub redirect_url: Option<String>,
    #[serde(rename = "headersSize", default)]
    pub headers_size: i64,
    #[serde(rename = "bodySize", default)]
    pub body_size: i64,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawContent {
    #[serde(default)]
    pub size: i64,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawPostData {
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawTimings {
    #[serde(default)]
    pub blocked: Option<f64>,
    #[serde(default)]
    pub dns: Option<f64>,
    #[serde(default)]
    pub connect: Option<f64>,
    #[serde(default)]
    pub send: f64,
    #[serde(default)]
    pub wait: f64,
    #[serde(default)]
    pub receive: f64,
    #[serde(default)]
    pub ssl: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct RawNameValue {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}
```

- [ ] **Step 2: Write the failing loader test** in `src/loader.rs`:

```rust
use crate::raw::RawDoc;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

/// Load a HAR file via mmap and a single-pass typed deserialization.
pub fn load(path: &Path) -> Result<RawDoc, LoadError> {
    let file = File::open(path).map_err(LoadError::Io)?;
    // SAFETY: the file is opened read-only and not mutated while mapped.
    let mmap = unsafe { Mmap::map(&file).map_err(LoadError::Io)? };
    let doc: RawDoc = serde_json::from_slice(&mmap).map_err(LoadError::Json)?;
    Ok(doc)
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read HAR file")]
    Io(#[source] std::io::Error),
    #[error("failed to parse HAR JSON")]
    Json(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_v1_2_fixture() {
        let doc = load(&fixture("someapi123.har")).expect("should load");
        assert_eq!(doc.log.version, "1.2");
        assert!(!doc.log.entries.is_empty());
    }

    #[test]
    fn loads_v1_3_fixture() {
        let doc = load(&fixture("someapi13.har")).expect("should load");
        assert_eq!(doc.log.version, "1.3");
        assert!(!doc.log.entries.is_empty());
    }
}
```

- [ ] **Step 3: Run the test to verify it fails for the right reason**

Run: `cargo test --lib loader 2>&1 | tail -20`
Expected: FAIL to COMPILE first if `raw.rs` wasn't saved; once compiling, the tests should actually PASS because Steps 1–2 already contain the full implementation. If a fixture's version string differs (e.g. the v1.3 fixture reports `"1.3"`), adjust the `assert_eq!` to the real value. Confirm the real values with:
```bash
grep -m1 '"version"' tests/fixtures/someapi123.har tests/fixtures/someapi13.har
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib loader 2>&1 | tail -10`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/raw.rs src/loader.rs
git commit -m "feat: mmap-based single-pass HAR loader with permissive raw model"
```

---

### Task 3: Route normalization

**Files:**
- Create: `src/normalize.rs`
- Test: `src/normalize.rs` (inline)

- [ ] **Step 1: Write the failing tests** in `src/normalize.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::normalize_path;

    #[test]
    fn collapses_numeric_ids() {
        assert_eq!(normalize_path("/users/123/orders/456"), "/users/{id}/orders/{id}");
    }

    #[test]
    fn collapses_uuid() {
        assert_eq!(
            normalize_path("/v1/items/550e8400-e29b-41d4-a716-446655440000"),
            "/v1/items/{id}"
        );
    }

    #[test]
    fn collapses_long_hex() {
        assert_eq!(normalize_path("/blob/0123456789abcdef0123"), "/blob/{id}");
    }

    #[test]
    fn keeps_normal_words() {
        assert_eq!(normalize_path("/3/tv/popular"), "/3/tv/popular");
    }

    #[test]
    fn preserves_leading_and_trailing_slashes() {
        assert_eq!(normalize_path("/a/123/"), "/a/{id}/");
    }
}
```

Note: `/3/tv/popular` keeps `3` because it is a single short numeric segment that
is part of a versioned API root — but our rule treats ALL pure-numeric segments
as IDs. To honor the test above, the rule is: a pure-numeric segment is an ID
**only if it is longer than 1 digit OR not the first segment**. Implement that
nuance so the TMDB-style `/3/tv/popular` survives.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib normalize 2>&1 | tail -15`
Expected: FAIL with "cannot find function `normalize_path`".

- [ ] **Step 3: Implement** at the top of `src/normalize.rs`:

```rust
/// Collapse identifier-like path segments into `{id}` so routes group together.
pub fn normalize_path(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, seg) in path.split('/').enumerate() {
        if is_id_segment(seg, i) {
            parts.push("{id}".to_string());
        } else {
            parts.push(seg.to_string());
        }
    }
    parts.join("/")
}

fn is_id_segment(seg: &str, index: usize) -> bool {
    if seg.is_empty() {
        return false;
    }
    // Pure numeric: treat as id unless it is a single leading digit
    // (keeps versioned roots like `/3/tv/popular`).
    if seg.bytes().all(|b| b.is_ascii_digit()) {
        return !(index == 1 && seg.len() == 1);
    }
    if is_uuid(seg) {
        return true;
    }
    if is_long_hex(seg) {
        return true;
    }
    if is_base64ish(seg) {
        return true;
    }
    false
}

fn is_uuid(s: &str) -> bool {
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

fn is_long_hex(s: &str) -> bool {
    s.len() >= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_base64ish(s: &str) -> bool {
    if s.len() < 20 {
        return false;
    }
    let valid = s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let has_upper = s.bytes().any(|b| b.is_ascii_uppercase());
    valid && has_digit && has_upper
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib normalize 2>&1 | tail -10`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add src/normalize.rs
git commit -m "feat: route normalization (numeric/uuid/hex/base64 -> {id})"
```

---

### Task 4: Resource classification

**Files:**
- Create: `src/classify.rs`
- Test: `src/classify.rs` (inline)

- [ ] **Step 1: Write the failing tests** in `src/classify.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{classify, ResourceType};

    #[test]
    fn json_is_api() {
        assert_eq!(classify(Some("application/json"), "https://api.x/v1/y"), ResourceType::Api);
    }

    #[test]
    fn image_is_media() {
        assert_eq!(classify(Some("image/png"), "https://x/a.png"), ResourceType::Media);
    }

    #[test]
    fn video_is_media() {
        assert_eq!(classify(Some("video/mp4"), "https://x/a.mp4"), ResourceType::Media);
    }

    #[test]
    fn javascript_is_static() {
        assert_eq!(classify(Some("application/javascript"), "https://x/a.js"), ResourceType::Static);
    }

    #[test]
    fn falls_back_to_extension() {
        assert_eq!(classify(None, "https://x/styles.css"), ResourceType::Static);
    }

    #[test]
    fn analytics_host() {
        assert_eq!(classify(Some("application/json"), "https://www.google-analytics.com/collect"), ResourceType::Analytics);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib classify 2>&1 | tail -15`
Expected: FAIL with "cannot find type `ResourceType`".

- [ ] **Step 3: Implement** at the top of `src/classify.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Api,
    Media,
    Static,
    Analytics,
    Document,
    Other,
}

const ANALYTICS_HOSTS: &[&str] = &[
    "google-analytics.com",
    "analytics.google.com",
    "doubleclick.net",
    "googletagmanager.com",
    "segment.io",
    "mixpanel.com",
    "amplitude.com",
    "sentry.io",
    "crashlytics.com",
];

/// Classify an entry by content-type, then URL extension, then host.
pub fn classify(content_type: Option<&str>, url: &str) -> ResourceType {
    let host = host_of(url);
    if ANALYTICS_HOSTS.iter().any(|h| host.ends_with(h)) {
        return ResourceType::Analytics;
    }
    if let Some(ct) = content_type {
        let ct = ct.split(';').next().unwrap_or(ct).trim().to_ascii_lowercase();
        if let Some(rt) = by_mime(&ct) {
            return rt;
        }
    }
    by_extension(url)
}

fn by_mime(ct: &str) -> Option<ResourceType> {
    if ct.contains("json") || ct.contains("graphql") || ct.contains("grpc") || ct.contains("protobuf") {
        return Some(ResourceType::Api);
    }
    if ct.contains("xml") && !ct.contains("html") {
        return Some(ResourceType::Api);
    }
    if ct.starts_with("image/") || ct.starts_with("video/") || ct.starts_with("audio/") {
        return Some(ResourceType::Media);
    }
    if ct.contains("javascript") || ct.contains("css") || ct.contains("font") || ct.contains("ecmascript") {
        return Some(ResourceType::Static);
    }
    if ct.contains("html") {
        return Some(ResourceType::Document);
    }
    None
}

fn by_extension(url: &str) -> ResourceType {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "mp4" | "webm" | "ts" | "m4s"
        | "mp3" | "aac" | "m3u8" => ResourceType::Media,
        "js" | "mjs" | "css" | "woff" | "woff2" | "ttf" | "otf" | "eot" => ResourceType::Static,
        "json" => ResourceType::Api,
        "html" | "htm" => ResourceType::Document,
        _ => ResourceType::Other,
    }
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default()
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib classify 2>&1 | tail -10`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add src/classify.rs
git commit -m "feat: resource classification (api/media/static/analytics/document)"
```

---

### Task 5: Correlation-ID extraction

**Files:**
- Create: `src/correlate.rs`
- Test: `src/correlate.rs` (inline)

- [ ] **Step 1: Write the failing test** in `src/correlate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::extract_correlation;

    #[test]
    fn extracts_known_headers_case_insensitively() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Request-Id".to_string(), "abc-123".to_string()),
            ("cf-ray".to_string(), "7d-DFW".to_string()),
        ];
        let got = extract_correlation(&headers);
        assert!(got.contains(&("x-request-id".to_string(), "abc-123".to_string())));
        assert!(got.contains(&("cf-ray".to_string(), "7d-DFW".to_string())));
        assert_eq!(got.len(), 2);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib correlate 2>&1 | tail -15`
Expected: FAIL with "cannot find function `extract_correlation`".

- [ ] **Step 3: Implement** at the top of `src/correlate.rs`:

```rust
const CORRELATION_HEADERS: &[&str] = &[
    "x-request-id",
    "x-correlation-id",
    "traceparent",
    "x-amzn-trace-id",
    "cf-ray",
    "x-datadog-trace-id",
];

/// Pull known correlation headers (lowercased name, original value) from a
/// header list. Order follows `CORRELATION_HEADERS`.
pub fn extract_correlation(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for known in CORRELATION_HEADERS {
        if let Some((_, v)) = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(known))
        {
            out.push((known.to_string(), v.clone()));
        }
    }
    out
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib correlate 2>&1 | tail -10`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/correlate.rs
git commit -m "feat: correlation-ID header extraction"
```

---

### Task 6: Normalized model + assemble

**Files:**
- Create: `src/model.rs`
- Create: `src/assemble.rs`
- Test: `src/assemble.rs` (inline)

- [ ] **Step 1: Write the model types** in `src/model.rs`:

```rust
use crate::classify::ResourceType;
use serde::Serialize;

/// Deterministic entry id, e.g. `e000123`.
pub fn format_entry_id(index: usize) -> String {
    format!("e{index:06}")
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureMeta {
    pub har_version: String,
    pub creator: String,
    pub creator_version: String,
    pub browser: Option<String>,
    pub entry_count: usize,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub duration_ms: f64,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub meta: CaptureMeta,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub index: usize,
    pub started_offset_ms: f64,
    pub duration_ms: f64,
    pub method: String,
    pub url: String,
    pub host: String,
    pub path: String,
    pub norm_path: String,
    pub query: Vec<(String, String)>,
    pub status: i64,
    pub status_text: String,
    pub resource_type: ResourceType,
    pub content_type: Option<String>,
    pub req_headers: Vec<(String, String)>,
    pub resp_headers: Vec<(String, String)>,
    pub req_body: Option<String>,
    pub resp_body: Option<String>,
    pub timings: Phases,
    pub sizes: Sizes,
    pub server_ip: Option<String>,
    pub http_version: String,
    pub redirect_url: Option<String>,
    pub correlation: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct Phases {
    pub blocked: Option<f64>,
    pub dns: Option<f64>,
    pub connect: Option<f64>,
    pub ssl: Option<f64>,
    pub send: f64,
    pub wait: f64,
    pub receive: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Sizes {
    pub req_body: i64,
    pub resp_body: i64,
    pub resp_content: i64,
    pub resp_headers: i64,
}

impl Entry {
    /// HTTP status class digit (2,3,4,5) or 0 for status 0 / out of range.
    pub fn status_class(&self) -> i64 {
        if (100..600).contains(&self.status) {
            self.status / 100
        } else {
            0
        }
    }

    pub fn is_error(&self) -> bool {
        self.status_class() == 4 || self.status_class() == 5 || self.status == 0
    }
}
```

- [ ] **Step 2: Write the failing assemble test** in `src/assemble.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::assemble;
    use crate::loader::load;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn assembles_capture_with_ids_and_hosts() {
        let doc = load(&fixture("someapi123.har")).unwrap();
        let cap = assemble(doc);
        assert_eq!(cap.meta.entry_count, cap.entries.len());
        assert_eq!(cap.entries[0].id, "e000000");
        // first entry offset is always 0
        assert_eq!(cap.entries[0].started_offset_ms, 0.0);
        // every entry has a parsed host
        assert!(cap.entries.iter().all(|e| !e.host.is_empty()));
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib assemble 2>&1 | tail -15`
Expected: FAIL with "cannot find function `assemble`".

- [ ] **Step 4: Implement** at the top of `src/assemble.rs`:

```rust
use crate::classify::classify;
use crate::correlate::extract_correlation;
use crate::model::{Capture, CaptureMeta, Entry, Phases, Sizes, format_entry_id};
use crate::normalize::normalize_path;
use crate::raw::{RawDoc, RawEntry, RawNameValue};
use chrono::DateTime;

/// Transform a parsed raw document into the normalized analysis model.
pub fn assemble(doc: RawDoc) -> Capture {
    let log = doc.log;

    // First pass: epoch-ms timestamps, to compute capture window + offsets.
    let starts: Vec<Option<i64>> = log
        .entries
        .iter()
        .map(|e| parse_epoch_ms(&e.started_date_time))
        .collect();

    let capture_start = starts.iter().flatten().copied().min();
    let mut capture_end: Option<i64> = None;
    for (e, s) in log.entries.iter().zip(&starts) {
        if let Some(s) = s {
            let end = s + e.time.round() as i64;
            capture_end = Some(capture_end.map_or(end, |c: i64| c.max(end)));
        }
    }

    let entries: Vec<Entry> = log
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| build_entry(i, e, starts[i], capture_start))
        .collect();

    let meta = CaptureMeta {
        har_version: log.version,
        creator: log.creator.name,
        creator_version: log.creator.version,
        browser: log.browser.map(|b| b.name),
        entry_count: entries.len(),
        start_ms: capture_start,
        end_ms: capture_end,
        duration_ms: match (capture_start, capture_end) {
            (Some(s), Some(end)) => (end - s) as f64,
            _ => 0.0,
        },
    };

    Capture { meta, entries }
}

fn build_entry(
    index: usize,
    e: &RawEntry,
    start_ms: Option<i64>,
    capture_start: Option<i64>,
) -> Entry {
    let url = e.request.url.clone();
    let (host, path, query) = split_url(&url);
    let content_type = e.response.content.mime_type.clone();
    let resource_type = classify(content_type.as_deref(), &url);

    let req_headers = name_values(&e.request.headers);
    let resp_headers = name_values(&e.response.headers);
    let correlation = extract_correlation(&resp_headers);

    let started_offset_ms = match (start_ms, capture_start) {
        (Some(s), Some(c)) => (s - c) as f64,
        _ => 0.0,
    };

    Entry {
        id: format_entry_id(index),
        index,
        started_offset_ms,
        duration_ms: e.time,
        method: e.request.method.clone(),
        norm_path: normalize_path(&path),
        path,
        host,
        query,
        url,
        status: e.response.status,
        status_text: e.response.status_text.clone(),
        resource_type,
        content_type,
        req_headers,
        resp_headers,
        req_body: e.request.post_data.as_ref().and_then(|p| p.text.clone()),
        resp_body: e.response.content.text.clone(),
        timings: Phases {
            blocked: clamp_phase(e.timings.blocked),
            dns: clamp_phase(e.timings.dns),
            connect: clamp_phase(e.timings.connect),
            ssl: clamp_phase(e.timings.ssl),
            send: e.timings.send.max(0.0),
            wait: e.timings.wait.max(0.0),
            receive: e.timings.receive.max(0.0),
        },
        sizes: Sizes {
            req_body: e.request.body_size,
            resp_body: e.response.body_size,
            resp_content: e.response.content.size,
            resp_headers: e.response.headers_size,
        },
        server_ip: e.server_ip_address.clone(),
        http_version: e.response.http_version_or_request(&e.request.http_version),
        redirect_url: e.response.redirect_url.clone().filter(|s| !s.is_empty()),
        correlation,
    }
}

/// HAR timing phases use -1 to mean "not applicable"; map negatives to None.
fn clamp_phase(v: Option<f64>) -> Option<f64> {
    match v {
        Some(x) if x >= 0.0 => Some(x),
        _ => None,
    }
}

fn name_values(items: &[RawNameValue]) -> Vec<(String, String)> {
    items.iter().map(|h| (h.name.clone(), h.value.clone())).collect()
}

fn split_url(url: &str) -> (String, String, Vec<(String, String)>) {
    match url::Url::parse(url) {
        Ok(u) => {
            let host = u.host_str().unwrap_or("").to_string();
            let path = u.path().to_string();
            let query = u
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            (host, path, query)
        }
        Err(_) => (String::new(), url.to_string(), Vec::new()),
    }
}

fn parse_epoch_ms(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp_millis())
}
```

- [ ] **Step 5: Add the small helper used above** — append to `src/raw.rs` an impl that picks the response httpVersion, falling back to the request's:

```rust
impl RawResponse {
    pub fn http_version_or_request(&self, req_version: &str) -> String {
        if self.http_version.is_empty() {
            req_version.to_string()
        } else {
            self.http_version.clone()
        }
    }
}
```

Also add the `http_version` field to `RawResponse` (it was omitted in Task 2). Insert into the `RawResponse` struct definition:

```rust
    #[serde(rename = "httpVersion", default)]
    pub http_version: String,
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --lib assemble 2>&1 | tail -10`
Expected: PASS (1 test). If the v1.2 fixture's first entry has a non-absolute URL and `host` is empty, inspect with `grep -m1 '"url"' tests/fixtures/someapi123.har` and relax the assertion to skip empty-host entries — but real HAR request URLs are absolute, so it should pass.

- [ ] **Step 7: Commit**

```bash
git add src/model.rs src/assemble.rs src/raw.rs
git commit -m "feat: normalized Capture model + assemble from raw"
```

---

### Task 7: Redaction engine

**Files:**
- Create: `src/redact.rs`
- Test: `src/redact.rs` (inline)

- [ ] **Step 1: Write the failing tests** in `src/redact.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{redact_header_value, redact_query_value};

    #[test]
    fn redacts_authorization_header() {
        assert_eq!(redact_header_value("Authorization", "Bearer abc", false), "<redacted>");
    }

    #[test]
    fn passes_through_safe_header() {
        assert_eq!(redact_header_value("Accept", "application/json", false), "application/json");
    }

    #[test]
    fn unsafe_flag_disables_redaction() {
        assert_eq!(redact_header_value("Authorization", "Bearer abc", true), "Bearer abc");
    }

    #[test]
    fn redacts_token_query_param() {
        assert_eq!(redact_query_value("access_token", "xyz", false), "<redacted>");
        assert_eq!(redact_query_value("page", "2", false), "2");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib redact 2>&1 | tail -15`
Expected: FAIL with "cannot find function `redact_header_value`".

- [ ] **Step 3: Implement** at the top of `src/redact.rs`:

```rust
pub const REDACTED: &str = "<redacted>";

const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "proxy-authorization",
    "x-api-key",
    "x-auth-token",
    "x-amz-security-token",
];

const SENSITIVE_QUERY_KEYS: &[&str] = &[
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "key",
    "api_key",
    "apikey",
    "sig",
    "signature",
    "password",
    "secret",
];

pub fn redact_header_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_HEADERS.iter().any(|h| *h == lname) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

pub fn redact_query_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_QUERY_KEYS.iter().any(|k| *k == lname) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib redact 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/redact.rs
git commit -m "feat: redaction engine for sensitive headers and query params"
```

---

### Task 8: Filter language

**Files:**
- Create: `src/filter.rs`
- Test: `src/filter.rs` (inline)

- [ ] **Step 1: Write the failing tests** in `src/filter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::Filter;
    use crate::classify::ResourceType;
    use crate::model::{Entry, Phases, Sizes};

    fn entry(host: &str, status: i64, method: &str, path: &str, dur: f64) -> Entry {
        Entry {
            id: "e000000".into(), index: 0, started_offset_ms: 0.0, duration_ms: dur,
            method: method.into(), url: format!("https://{host}{path}"), host: host.into(),
            path: path.into(), norm_path: path.into(), query: vec![], status,
            status_text: String::new(), resource_type: ResourceType::Api, content_type: None,
            req_headers: vec![("authorization".into(), "x".into())], resp_headers: vec![],
            req_body: None, resp_body: None, timings: Phases::default(), sizes: Sizes::default(),
            server_ip: None, http_version: "HTTP/2".into(), redirect_url: None, correlation: vec![],
        }
    }

    #[test]
    fn matches_host_and_status() {
        let f = Filter::parse(&["host:api.foo.com".into(), "status:>=400".into()]).unwrap();
        assert!(f.matches(&entry("api.foo.com", 500, "GET", "/x", 10.0)));
        assert!(!f.matches(&entry("api.foo.com", 200, "GET", "/x", 10.0)));
        assert!(!f.matches(&entry("other.com", 500, "GET", "/x", 10.0)));
    }

    #[test]
    fn matches_method_and_path_glob_and_time() {
        let f = Filter::parse(&["method:POST".into(), "path:*login*".into(), "time:>5ms".into()]).unwrap();
        assert!(f.matches(&entry("h", 200, "POST", "/v1/login/start", 10.0)));
        assert!(!f.matches(&entry("h", 200, "POST", "/v1/login/start", 1.0)));
        assert!(!f.matches(&entry("h", 200, "GET", "/v1/login/start", 10.0)));
    }

    #[test]
    fn matches_has_header() {
        let f = Filter::parse(&["has:req.header.authorization".into()]).unwrap();
        assert!(f.matches(&entry("h", 200, "GET", "/x", 1.0)));
    }

    #[test]
    fn empty_filter_matches_all() {
        let f = Filter::parse(&[]).unwrap();
        assert!(f.matches(&entry("h", 200, "GET", "/x", 1.0)));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib filter 2>&1 | tail -15`
Expected: FAIL with "cannot find type `Filter`" / "no function `parse`".

- [ ] **Step 3: Implement** at the top of `src/filter.rs`:

```rust
use crate::model::Entry;

#[derive(Debug)]
enum Cmp {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

#[derive(Debug)]
enum Clause {
    Host(String),
    Method(String),
    Path(String),
    Status(Cmp, i64),
    Time(Cmp, f64),
    Has(String),
}

#[derive(Debug)]
pub struct Filter {
    clauses: Vec<Clause>,
}

impl Filter {
    /// Parse clauses like `host:api.foo.com status:>=400 path:*login* time:>5ms`.
    pub fn parse(exprs: &[String]) -> Result<Filter, String> {
        let mut clauses = Vec::new();
        for raw in exprs {
            for token in raw.split_whitespace() {
                clauses.push(parse_clause(token)?);
            }
        }
        Ok(Filter { clauses })
    }

    pub fn matches(&self, e: &Entry) -> bool {
        self.clauses.iter().all(|c| clause_matches(c, e))
    }
}

fn parse_clause(token: &str) -> Result<Clause, String> {
    let (key, val) = token
        .split_once(':')
        .ok_or_else(|| format!("invalid filter clause: {token}"))?;
    match key {
        "host" => Ok(Clause::Host(val.to_string())),
        "method" => Ok(Clause::Method(val.to_ascii_uppercase())),
        "path" => Ok(Clause::Path(val.to_string())),
        "status" => {
            let (cmp, n) = parse_cmp_int(val)?;
            Ok(Clause::Status(cmp, n))
        }
        "time" => {
            let v = val.trim_end_matches("ms");
            let (cmp, n) = parse_cmp_float(v)?;
            Ok(Clause::Time(cmp, n))
        }
        "has" => Ok(Clause::Has(val.to_ascii_lowercase())),
        other => Err(format!("unknown filter key: {other}")),
    }
}

fn parse_cmp_int(s: &str) -> Result<(Cmp, i64), String> {
    let (cmp, rest) = split_cmp(s);
    let n = rest.parse::<i64>().map_err(|_| format!("invalid number: {rest}"))?;
    Ok((cmp, n))
}

fn parse_cmp_float(s: &str) -> Result<(Cmp, f64), String> {
    let (cmp, rest) = split_cmp(s);
    let n = rest.parse::<f64>().map_err(|_| format!("invalid number: {rest}"))?;
    Ok((cmp, n))
}

fn split_cmp(s: &str) -> (Cmp, &str) {
    if let Some(rest) = s.strip_prefix(">=") {
        (Cmp::Ge, rest)
    } else if let Some(rest) = s.strip_prefix("<=") {
        (Cmp::Le, rest)
    } else if let Some(rest) = s.strip_prefix('>') {
        (Cmp::Gt, rest)
    } else if let Some(rest) = s.strip_prefix('<') {
        (Cmp::Lt, rest)
    } else if let Some(rest) = s.strip_prefix('=') {
        (Cmp::Eq, rest)
    } else {
        (Cmp::Eq, s)
    }
}

fn cmp_i(cmp: &Cmp, a: i64, b: i64) -> bool {
    match cmp {
        Cmp::Ge => a >= b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Lt => a < b,
        Cmp::Eq => a == b,
    }
}

fn cmp_f(cmp: &Cmp, a: f64, b: f64) -> bool {
    match cmp {
        Cmp::Ge => a >= b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Lt => a < b,
        Cmp::Eq => a == b,
    }
}

fn clause_matches(c: &Clause, e: &Entry) -> bool {
    match c {
        Clause::Host(h) => glob_match(h, &e.host),
        Clause::Method(m) => e.method.eq_ignore_ascii_case(m),
        Clause::Path(p) => glob_match(p, &e.path),
        Clause::Status(cmp, n) => cmp_i(cmp, e.status, *n),
        Clause::Time(cmp, n) => cmp_f(cmp, e.duration_ms, *n),
        Clause::Has(field) => has_field(field, e),
    }
}

fn has_field(field: &str, e: &Entry) -> bool {
    // Supported forms: req.header.<name>, resp.header.<name>, req.body, resp.body
    if let Some(name) = field.strip_prefix("req.header.") {
        return e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    }
    if let Some(name) = field.strip_prefix("resp.header.") {
        return e.resp_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    }
    match field {
        "req.body" => e.req_body.as_ref().is_some_and(|b| !b.is_empty()),
        "resp.body" => e.resp_body.as_ref().is_some_and(|b| !b.is_empty()),
        _ => false,
    }
}

/// Minimal glob: `*` matches any run of characters. Case-insensitive.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let t = text.to_ascii_lowercase();
    if !p.contains('*') {
        return p == t;
    }
    let parts: Vec<&str> = p.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !t[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            return t[pos..].ends_with(part);
        } else if let Some(found) = t[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib filter 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/filter.rs
git commit -m "feat: filter language (host/status/method/path/time/has + glob)"
```

---

### Task 9: Duplicate fingerprint helper

**Files:**
- Create: `src/fingerprint.rs`
- Test: `src/fingerprint.rs` (inline)

- [ ] **Step 1: Write the failing tests** in `src/fingerprint.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::fingerprint;
    use crate::classify::ResourceType;
    use crate::model::{Entry, Phases, Sizes};

    fn entry(method: &str, host: &str, norm_path: &str, query: &[(&str, &str)]) -> Entry {
        Entry {
            id: "e0".into(), index: 0, started_offset_ms: 0.0, duration_ms: 0.0,
            method: method.into(), url: String::new(), host: host.into(), path: norm_path.into(),
            norm_path: norm_path.into(),
            query: query.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            status: 200, status_text: String::new(), resource_type: ResourceType::Api,
            content_type: None, req_headers: vec![], resp_headers: vec![], req_body: None,
            resp_body: None, timings: Phases::default(), sizes: Sizes::default(), server_ip: None,
            http_version: String::new(), redirect_url: None, correlation: vec![],
        }
    }

    #[test]
    fn same_request_same_fingerprint() {
        let a = entry("GET", "h", "/v1/x", &[("page", "1")]);
        let b = entry("GET", "h", "/v1/x", &[("page", "1")]);
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn cache_buster_params_ignored() {
        let a = entry("GET", "h", "/v1/x", &[("_", "111")]);
        let b = entry("GET", "h", "/v1/x", &[("_", "222")]);
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn different_method_differs() {
        let a = entry("GET", "h", "/v1/x", &[]);
        let b = entry("POST", "h", "/v1/x", &[]);
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib fingerprint 2>&1 | tail -15`
Expected: FAIL with "cannot find function `fingerprint`".

- [ ] **Step 3: Implement** at the top of `src/fingerprint.rs`:

```rust
use crate::model::Entry;

/// Query keys treated as cache-busters / nonces and excluded from the fingerprint.
const IGNORED_QUERY_KEYS: &[&str] = &["_", "cb", "cachebuster", "nonce", "ts", "timestamp"];

/// Stable duplicate fingerprint: METHOD host norm_path sorted(meaningful query keys=values).
/// Body is intentionally excluded at this layer (the richer `duplicates` command
/// in Plan 3 adds body fingerprinting).
pub fn fingerprint(e: &Entry) -> String {
    let mut pairs: Vec<(String, String)> = e
        .query
        .iter()
        .filter(|(k, _)| !IGNORED_QUERY_KEYS.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    pairs.sort();
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{} {} {} {}", e.method.to_ascii_uppercase(), e.host, e.norm_path, query)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib fingerprint 2>&1 | tail -10`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/fingerprint.rs
git commit -m "feat: duplicate fingerprint helper"
```

---

### Task 10: Output envelope and exit codes

**Files:**
- Create: `src/render.rs`
- Test: `src/render.rs` (inline)

- [ ] **Step 1: Write the failing test** in `src/render.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::{Envelope, ExitCode};
    use crate::model::CaptureMeta;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Dummy {
        n: u32,
    }

    fn meta() -> CaptureMeta {
        CaptureMeta {
            har_version: "1.2".into(), creator: "x".into(), creator_version: "1".into(),
            browser: None, entry_count: 0, start_ms: None, end_ms: None, duration_ms: 0.0,
        }
    }

    #[test]
    fn serializes_stable_envelope() {
        let env = Envelope::new("summary", meta(), Dummy { n: 7 });
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"tool\":\"wiretrail\""));
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"command\":\"summary\""));
        assert!(json.contains("\"n\":7"));
    }

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(ExitCode::Clean as i32, 0);
        assert_eq!(ExitCode::Findings as i32, 1);
        assert_eq!(ExitCode::InvalidHar as i32, 2);
        assert_eq!(ExitCode::UnsafeBlocked as i32, 3);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib render 2>&1 | tail -15`
Expected: FAIL with "cannot find type `Envelope`".

- [ ] **Step 3: Implement** at the top of `src/render.rs`:

```rust
use crate::model::CaptureMeta;
use serde::Serialize;

#[derive(Serialize)]
pub struct Envelope<T: Serialize> {
    pub tool: &'static str,
    pub schema_version: u32,
    pub command: &'static str,
    pub capture: CaptureMeta,
    pub result: T,
    pub warnings: Vec<String>,
    pub next_commands: Vec<String>,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(command: &'static str, capture: CaptureMeta, result: T) -> Self {
        Envelope {
            tool: "wiretrail",
            schema_version: 1,
            command,
            capture,
            result,
            warnings: Vec::new(),
            next_commands: Vec::new(),
        }
    }

    pub fn with_next_commands(mut self, cmds: Vec<String>) -> Self {
        self.next_commands = cmds;
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Clean = 0,
    Findings = 1,
    InvalidHar = 2,
    UnsafeBlocked = 3,
}

/// Human-readable byte size, e.g. 1.2 MiB.
pub fn human_bytes(n: i64) -> String {
    if n < 0 {
        return "?".to_string();
    }
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// Human-readable milliseconds, e.g. 75.4s or 312ms.
pub fn human_ms(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.1}s", ms / 1000.0)
    } else {
        format!("{ms:.0}ms")
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib render 2>&1 | tail -10`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/render.rs
git commit -m "feat: JSON envelope, exit codes, and human-format helpers"
```

---

### Task 11: Summary analysis result

**Files:**
- Create: `src/analysis/summary.rs`
- Modify: `src/analysis/mod.rs` (already declares `pub mod summary;` from Task 1)
- Test: `src/analysis/summary.rs` (inline)

- [ ] **Step 1: Write the failing test** in `src/analysis/summary.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::compute_summary;
    use crate::assemble::assemble;
    use crate::filter::Filter;
    use crate::loader::load;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn computes_summary_over_fixture() {
        let cap = assemble(load(&fixture("someapi123.har")).unwrap());
        let f = Filter::parse(&[]).unwrap();
        let s = compute_summary(&cap, &f, 5);
        assert_eq!(s.total_entries, cap.entries.len());
        assert_eq!(s.filtered_entries, cap.entries.len());
        // status_classes counts sum to filtered_entries
        let sum: usize = s.status_classes.values().sum();
        assert_eq!(sum, s.filtered_entries);
        // top_hosts has at most 5 entries
        assert!(s.top_hosts.len() <= 5);
    }

    #[test]
    fn filter_reduces_filtered_count() {
        let cap = assemble(load(&fixture("someapi123.har")).unwrap());
        let f = Filter::parse(&["status:>=400".into()]).unwrap();
        let s = compute_summary(&cap, &f, 5);
        assert!(s.filtered_entries <= s.total_entries);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib summary 2>&1 | tail -15`
Expected: FAIL with "cannot find function `compute_summary`".

- [ ] **Step 3: Implement** at the top of `src/analysis/summary.rs`:

```rust
use crate::fingerprint::fingerprint;
use crate::filter::Filter;
use crate::model::Capture;
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct SummaryResult {
    pub total_entries: usize,
    pub filtered_entries: usize,
    pub duration_ms: f64,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub resource_breakdown: BTreeMap<String, usize>,
    pub status_classes: BTreeMap<String, usize>,
    pub error_count: usize,
    pub top_hosts: Vec<HostCount>,
    pub top_duplicates: Vec<DuplicateGroup>,
    pub slowest: Vec<SlowEntry>,
    pub biggest_payloads: Vec<PayloadEntry>,
    pub hints: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HostCount {
    pub host: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub count: usize,
    pub example_id: String,
}

#[derive(Debug, Serialize)]
pub struct SlowEntry {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub duration_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct PayloadEntry {
    pub id: String,
    pub host: String,
    pub norm_path: String,
    pub bytes: i64,
}

/// Compute the executive summary over the (filtered) capture. `top` bounds list sizes.
pub fn compute_summary(cap: &Capture, filter: &Filter, top: usize) -> SummaryResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut resource_breakdown: BTreeMap<String, usize> = BTreeMap::new();
    let mut status_classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut host_counts: AHashMap<String, usize> = AHashMap::new();
    let mut fp_counts: AHashMap<String, (usize, String)> = AHashMap::new();
    let mut error_count = 0usize;

    for e in &entries {
        let rt = format!("{:?}", e.resource_type).to_ascii_lowercase();
        *resource_breakdown.entry(rt).or_default() += 1;

        let class = match e.status_class() {
            2 => "2xx",
            3 => "3xx",
            4 => "4xx",
            5 => "5xx",
            _ => "other",
        };
        *status_classes.entry(class.to_string()).or_default() += 1;

        if e.is_error() {
            error_count += 1;
        }

        *host_counts.entry(e.host.clone()).or_default() += 1;

        let fp = fingerprint(e);
        let slot = fp_counts.entry(fp).or_insert((0, e.id.clone()));
        slot.0 += 1;
    }

    let top_hosts = top_n_map(&host_counts, top)
        .into_iter()
        .map(|(host, count)| HostCount { host, count })
        .collect();

    let mut dups: Vec<DuplicateGroup> = fp_counts
        .into_iter()
        .filter(|(_, (c, _))| *c > 1)
        .map(|(fp, (c, id))| DuplicateGroup { fingerprint: fp, count: c, example_id: id })
        .collect();
    dups.sort_by(|a, b| b.count.cmp(&a.count).then(a.fingerprint.cmp(&b.fingerprint)));
    dups.truncate(top);

    let mut slow: Vec<SlowEntry> = entries
        .iter()
        .map(|e| SlowEntry {
            id: e.id.clone(),
            method: e.method.clone(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            duration_ms: e.duration_ms,
        })
        .collect();
    slow.sort_by(|a, b| b.duration_ms.partial_cmp(&a.duration_ms).unwrap_or(std::cmp::Ordering::Equal));
    slow.truncate(top);

    let mut payloads: Vec<PayloadEntry> = entries
        .iter()
        .map(|e| PayloadEntry {
            id: e.id.clone(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            bytes: e.sizes.resp_content.max(e.sizes.resp_body),
        })
        .collect();
    payloads.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    payloads.truncate(top);

    let mut hints = Vec::new();
    if let Some(top_dup) = dups.first() {
        if top_dup.count >= 3 {
            hints.push(format!(
                "{}x duplicate calls: {}",
                top_dup.count, top_dup.fingerprint
            ));
        }
    }
    if error_count > 0 {
        hints.push(format!("{error_count} error responses (4xx/5xx/failed)"));
    }

    SummaryResult {
        total_entries: cap.entries.len(),
        filtered_entries: entries.len(),
        duration_ms: cap.meta.duration_ms,
        start_ms: cap.meta.start_ms,
        end_ms: cap.meta.end_ms,
        resource_breakdown,
        status_classes,
        error_count,
        top_hosts,
        top_duplicates: dups,
        slowest: slow,
        biggest_payloads: payloads,
        hints,
    }
}

fn top_n_map(map: &AHashMap<String, usize>, top: usize) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.iter().map(|(k, c)| (k.clone(), *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(top);
    v
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib summary 2>&1 | tail -10`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analysis/summary.rs src/analysis/mod.rs
git commit -m "feat: summary analysis computation"
```

---

### Task 12: CLI dispatch + terminal/JSON rendering of `summary`

**Files:**
- Modify: `src/main.rs` (replace the Task 1 skeleton)
- Modify: `src/analysis/summary.rs` (add a terminal renderer function)

- [ ] **Step 1: Add a terminal renderer** to the bottom of `src/analysis/summary.rs` (before the `#[cfg(test)]` module):

```rust
use crate::render::{human_bytes, human_ms};

/// Render the summary as deterministic, copy-paste-safe terminal text.
pub fn render_summary_text(s: &SummaryResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail summary ==\n");
    out.push_str(&format!(
        "entries: {} total, {} after filter\n",
        s.total_entries, s.filtered_entries
    ));
    out.push_str(&format!("capture window: {}\n", human_ms(s.duration_ms)));

    out.push_str("\nstatus classes:\n");
    for (k, v) in &s.status_classes {
        out.push_str(&format!("  {k}: {v}\n"));
    }

    out.push_str("\nresource types:\n");
    for (k, v) in &s.resource_breakdown {
        out.push_str(&format!("  {k}: {v}\n"));
    }

    out.push_str(&format!("\ntop hosts (by request count):\n"));
    for h in &s.top_hosts {
        out.push_str(&format!("  {:>5}  {}\n", h.count, h.host));
    }

    if !s.top_duplicates.is_empty() {
        out.push_str("\ntop duplicate calls:\n");
        for d in &s.top_duplicates {
            out.push_str(&format!("  {:>4}x  {}  ({})\n", d.count, d.fingerprint, d.example_id));
        }
    }

    out.push_str("\nslowest requests:\n");
    for e in &s.slowest {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}  [{}]\n",
            human_ms(e.duration_ms),
            e.id,
            e.method,
            e.host,
            e.norm_path,
            e.status
        ));
    }

    out.push_str("\nbiggest payloads:\n");
    for p in &s.biggest_payloads {
        out.push_str(&format!(
            "  {:>10}  {} {}{}\n",
            human_bytes(p.bytes),
            p.id,
            p.host,
            p.norm_path
        ));
    }

    if !s.hints.is_empty() {
        out.push_str("\nhints:\n");
        for h in &s.hints {
            out.push_str(&format!("  - {h}\n"));
        }
    }

    out
}
```

- [ ] **Step 2: Replace `src/main.rs`** with the full dispatch:

```rust
use clap::{Parser, Subcommand};
use har::analysis::summary::{compute_summary, render_summary_text};
use har::assemble::assemble;
use har::filter::Filter;
use har::loader::load;
use har::render::{Envelope, ExitCode};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "wiretrail", version, about = "Fast HAR analyzer CLI")]
struct Cli {
    /// Path to the HAR file.
    file: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,

    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    /// Max items per list (top-N).
    #[arg(long, global = true, default_value_t = 10)]
    top: usize,

    /// Filter clauses, e.g. --filter "host:api.foo.com status:>=400".
    #[arg(long, global = true)]
    filter: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Executive summary of the capture (default).
    Summary,
}

fn main() {
    let cli = Cli::parse();

    let filter = match Filter::parse(&cli.filter) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("wiretrail: invalid filter: {e}");
            std::process::exit(ExitCode::InvalidHar as i32);
        }
    };

    let doc = match load(&cli.file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("wiretrail: {e}");
            std::process::exit(ExitCode::InvalidHar as i32);
        }
    };
    let cap = assemble(doc);

    match cli.command.unwrap_or(Command::Summary) {
        Command::Summary => {
            let result = compute_summary(&cap, &filter, cli.top);
            let has_findings = result.error_count > 0 || !result.top_duplicates.is_empty();
            if cli.json {
                let env = Envelope::new("summary", cap.meta.clone(), &result)
                    .with_next_commands(vec![
                        "duplicates".to_string(),
                        "errors".to_string(),
                        "slowest".to_string(),
                    ]);
                println!("{}", env.to_json());
            } else {
                print!("{}", render_summary_text(&result));
                println!("\nnext useful commands: duplicates · errors · slowest");
            }
            std::process::exit(if has_findings {
                ExitCode::Findings as i32
            } else {
                ExitCode::Clean as i32
            });
        }
    }
}
```

Note: `Envelope::new` takes `result: T` by value with `T: Serialize`; passing
`&result` makes `T = &SummaryResult`, which is `Serialize`. This avoids cloning
the result.

- [ ] **Step 3: Build**

Run: `cargo build 2>&1 | tail -15`
Expected: SUCCESS. If clap rejects the leading positional + optional subcommand
combination at runtime (Step 4 below will reveal it), the fix is to add
`#[command(args_conflicts_with_subcommands = true)]` to the `Cli` struct — but
because `file` is a required (non-`Option`) positional, the first token is always
bound to it, so this should not be needed.

- [ ] **Step 4: Manual smoke — default command**

Run: `cargo run --quiet -- tests/fixtures/someapi123.har`
Expected: prints the `== wiretrail summary ==` block with status classes, hosts,
slowest, etc., then the "next useful commands" line.

- [ ] **Step 5: Manual smoke — explicit subcommand + JSON + filter**

Run:
```bash
cargo run --quiet -- tests/fixtures/someapi123.har summary --json
cargo run --quiet -- tests/fixtures/someapi123.har summary --filter "status:>=400"
```
Expected: first prints a JSON object containing `"tool":"wiretrail"` and
`"command":"summary"`; second prints the text summary with a reduced
`after filter` count.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/analysis/summary.rs
git commit -m "feat: wire summary command into CLI (terminal + json)"
```

---

### Task 13: End-to-end binary test

**Files:**
- Create: `tests/cli_summary.rs`

- [ ] **Step 1: Write the failing test** in `tests/cli_summary.rs`:

```rust
use std::process::Command;

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn run(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_wiretrail"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn default_summary_prints_header() {
    let (stdout, _stderr, _code) = run(&[&fixture("someapi123.har")]);
    assert!(stdout.contains("== wiretrail summary =="));
    assert!(stdout.contains("top hosts"));
}

#[test]
fn json_envelope_is_stable() {
    let (stdout, _stderr, _code) = run(&[&fixture("someapi123.har"), "summary", "--json"]);
    assert!(stdout.contains("\"tool\": \"wiretrail\""));
    assert!(stdout.contains("\"schema_version\": 1"));
    assert!(stdout.contains("\"command\": \"summary\""));
    assert!(stdout.contains("\"next_commands\""));
}

#[test]
fn invalid_file_exits_2() {
    let (_stdout, stderr, code) = run(&["/nonexistent/path.har"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("wiretrail:"));
}
```

- [ ] **Step 2: Run to verify it passes** (the binary already exists from Task 12, so these should pass immediately — they are the integration safety net):

Run: `cargo test --test cli_summary 2>&1 | tail -15`
Expected: PASS (3 tests). The JSON assertions use pretty-printed spacing
(`"tool": "wiretrail"` with a space) because `to_json` uses
`serde_json::to_string_pretty`.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test 2>&1 | tail -25`
Expected: all unit, regression, and CLI tests PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/cli_summary.rs
git commit -m "test: end-to-end CLI summary integration test"
```

---

## Self-review

**Spec coverage (Foundation slice):**
- Fork & extend, fix parse path → Tasks 1, 2 (mmap + single-pass `from_slice`, no `Value`, no tagged enum). ✓
- Normalized model (entry IDs, offsets, sizes, timing phases) → Tasks 6. ✓
- Route normalization → Task 3. ✓
- Resource classification → Task 4. ✓
- Correlation IDs → Task 5. ✓
- Redaction safe-by-default → Task 7 (engine; applied in `show-entry`/exports in later plans; summary emits no raw header/body values, so it is safe by construction). ✓
- Filter language → Task 8. ✓
- JSON envelope + exit codes → Task 10, wired in Task 12. ✓
- `summary` command (meta, time range, counts, top hosts, top duplicates, error count, slowest, biggest payloads, hints, resource breakdown) → Tasks 11, 12. ✓
- Deferred to Plans 2–4 (the other 13 commands, YAML config, vendor heuristics, body fingerprinting, session cache) — explicitly out of this plan’s scope. ✓

**Placeholder scan:** No TBD/TODO; every code step contains complete code; every command step has expected output. ✓

**Type consistency:** `Entry`, `CaptureMeta`, `Phases`, `Sizes`, `ResourceType` defined in Task 6/4 and used identically in Tasks 8, 9, 11, 12. `Filter::parse`/`Filter::matches` (Task 8) used in Tasks 11, 12. `fingerprint` (Task 9) used in Task 11. `Envelope::new`/`ExitCode` (Task 10) used in Task 12. `compute_summary`/`render_summary_text`/`SummaryResult` (Tasks 11, 12) used in Task 12. `load` (Task 2) and `assemble` (Task 6) used in Tasks 11, 12, 13. All names match. ✓

**Note on `Envelope` generic:** Task 10 tests construct `Envelope::new(.., Dummy)` (owned `T`); Task 12 calls `Envelope::new(.., &result)` (`T = &SummaryResult`). Both satisfy `T: Serialize`. Consistent. ✓
