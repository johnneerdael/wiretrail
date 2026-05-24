use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::{redact_body, redact_header_value, redact_url};
use serde::Serialize;

const MUTATING_METHODS: &[&str] = &["POST", "PUT", "PATCH", "DELETE"];
const RISKY_KEYWORDS: &[&str] = &["payment", "pay", "order", "checkout", "charge", "refund", "subscription"];
const BODY_MAX: usize = 4000;

#[derive(Debug, Serialize)]
pub struct CurlResult {
    pub commands: Vec<CurlCommand>,
}

#[derive(Debug, Serialize)]
pub struct CurlCommand {
    pub id: String,
    pub safe: bool,
    pub label: String,
    pub command: String,
}

/// Build a sanitized, safety-labeled curl command for one entry.
pub fn entry_to_curl(e: &Entry, unsafe_include: bool) -> CurlCommand {
    let method = e.method.to_ascii_uppercase();
    let url = redact_url(&e.url, unsafe_include);

    let mut parts = vec![format!("curl -X {method} '{url}'")];
    for (k, v) in &e.req_headers {
        if k.starts_with(':') {
            continue; // skip HTTP/2 pseudo-headers (:method, :path, :authority, :scheme)
        }
        let rv = redact_header_value(k, v, unsafe_include);
        parts.push(format!("  -H '{k}: {rv}'"));
    }
    if let Some(body) = e.req_body.as_deref().filter(|b| !b.is_empty()) {
        let rb = redact_body(body, unsafe_include, BODY_MAX);
        parts.push(format!("  --data '{rb}'"));
    }

    let (safe, label) = safety(&method, &e.norm_path);
    CurlCommand {
        id: e.id.clone(),
        safe,
        label,
        command: parts.join(" \\\n"),
    }
}

/// Render curl for every filtered entry, bounded by `top`.
pub fn compute_curl(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> CurlResult {
    let commands: Vec<CurlCommand> = cap
        .entries
        .iter()
        .filter(|e| filter.matches(e))
        .take(top)
        .map(|e| entry_to_curl(e, unsafe_include))
        .collect();
    CurlResult { commands }
}

fn safety(method: &str, norm_path: &str) -> (bool, String) {
    let lp = norm_path.to_ascii_lowercase();
    if RISKY_KEYWORDS.iter().any(|k| lp.contains(k)) {
        return (false, "payment/order endpoint".to_string());
    }
    if MUTATING_METHODS.contains(&method) {
        return (false, format!("mutating method {method}"));
    }
    (true, "safe".to_string())
}

/// Render curl commands as terminal text with safety annotations.
pub fn render_curl_text(r: &CurlResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail curl ==\n");
    for c in &r.commands {
        let tag = if c.safe { "SAFE" } else { "UNSAFE" };
        out.push_str(&format!("\n# {} [{}: {}]\n{}\n", c.id, tag, c.label, c.command));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{compute_curl, entry_to_curl};
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn get_is_safe_and_redacts_auth() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![
            (":method".into(), "GET".into()), // HTTP/2 pseudo-header, must be skipped
            ("Authorization".into(), "Bearer secret".into()),
            ("Accept".into(), "application/json".into()),
        ];
        let c = entry_to_curl(&e, false);
        assert!(c.safe);
        assert_eq!(c.label, "safe");
        assert!(c.command.starts_with("curl -X GET 'https://api.x/data'"));
        assert!(c.command.contains("Authorization: <redacted>"));
        assert!(c.command.contains("Accept: application/json"));
        assert!(!c.command.contains(":method"));
    }

    #[test]
    fn redacts_query_in_url() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.url = "https://api.x/data?access_token=leak&page=2".into();
        let c = entry_to_curl(&e, false);
        assert!(!c.command.contains("leak"));
        assert!(c.command.contains("page=2"));
    }

    #[test]
    fn mutating_method_is_unsafe() {
        let e = sample_entry(0, "api.x", "POST", "/things", 200);
        let c = entry_to_curl(&e, false);
        assert!(!c.safe);
        assert!(c.label.contains("mutating"));
    }

    #[test]
    fn payment_path_is_unsafe_even_for_get() {
        let e = sample_entry(0, "api.x", "GET", "/v1/payment/charge", 200);
        let c = entry_to_curl(&e, false);
        assert!(!c.safe);
        assert!(c.label.contains("payment"));
    }

    #[test]
    fn unsafe_flag_shows_raw_auth() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer secret".into())];
        let c = entry_to_curl(&e, true);
        assert!(c.command.contains("Bearer secret"));
    }

    #[test]
    fn compute_curl_bounds_by_top() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/b", 200),
            sample_entry(2, "h", "GET", "/c", 200),
        ]);
        let r = compute_curl(&cap, &Filter::parse(&[]).unwrap(), 2, false);
        assert_eq!(r.commands.len(), 2);
    }
}
