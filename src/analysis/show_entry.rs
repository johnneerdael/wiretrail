use crate::model::{Capture, Entry};
use crate::redact::{redact_body, redact_header_value, redact_query_value, redact_url};
use crate::timing::PhaseBreakdown;
use serde::Serialize;

const BODY_MAX: usize = 2000;

#[derive(Debug, Serialize)]
pub struct EntryDetail {
    pub id: String,
    pub index: usize,
    pub method: String,
    pub url: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub status_text: String,
    pub http_version: String,
    pub server_ip: Option<String>,
    pub resource_type: String,
    pub content_type: Option<String>,
    pub started_offset_ms: f64,
    pub duration_ms: f64,
    pub query: Vec<(String, String)>,
    pub req_headers: Vec<(String, String)>,
    pub resp_headers: Vec<(String, String)>,
    pub correlation: Vec<(String, String)>,
    pub timings: PhaseBreakdown,
    pub req_body_snippet: Option<String>,
    pub resp_body_snippet: Option<String>,
}

/// Find an entry by its `eNNNNNN` id, or by bare index (`123` or `e123`).
pub fn find_entry<'a>(cap: &'a Capture, id_arg: &str) -> Option<&'a Entry> {
    if let Some(e) = cap.entries.iter().find(|e| e.id == id_arg) {
        return Some(e);
    }
    let digits = id_arg.strip_prefix('e').unwrap_or(id_arg);
    let idx: usize = digits.parse().ok()?;
    cap.entries.iter().find(|e| e.index == idx)
}

/// Build a redacted, serializable detail view of one entry.
pub fn entry_detail(e: &Entry, unsafe_include: bool) -> EntryDetail {
    let query = e
        .query
        .iter()
        .map(|(k, v)| (k.clone(), redact_query_value(k, v, unsafe_include)))
        .collect();
    let req_headers = e
        .req_headers
        .iter()
        .map(|(k, v)| (k.clone(), redact_header_value(k, v, unsafe_include)))
        .collect();
    let resp_headers = e
        .resp_headers
        .iter()
        .map(|(k, v)| (k.clone(), redact_header_value(k, v, unsafe_include)))
        .collect();
    let req_body_snippet = e
        .req_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, BODY_MAX));
    let resp_body_snippet = e
        .resp_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, BODY_MAX));

    EntryDetail {
        id: e.id.clone(),
        index: e.index,
        method: e.method.to_ascii_uppercase(),
        url: redact_url(&e.url, unsafe_include),
        host: e.host.clone(),
        norm_path: e.norm_path.clone(),
        status: e.status,
        status_text: e.status_text.clone(),
        http_version: e.http_version.clone(),
        server_ip: e.server_ip.clone(),
        resource_type: format!("{:?}", e.resource_type).to_ascii_lowercase(),
        content_type: e.content_type.clone(),
        started_offset_ms: e.started_offset_ms,
        duration_ms: e.duration_ms,
        query,
        req_headers,
        resp_headers,
        correlation: e.correlation.clone(),
        timings: PhaseBreakdown::from_phases(&e.timings),
        req_body_snippet,
        resp_body_snippet,
    }
}

/// Render an entry detail as deterministic terminal text.
pub fn render_entry_detail_text(d: &EntryDetail) -> String {
    let mut out = String::new();
    out.push_str(&format!("== wiretrail entry {} ==\n", d.id));
    out.push_str(&format!(
        "{} {}  [{}] {}\n",
        d.method, d.url, d.status, d.status_text
    ));
    out.push_str(&format!(
        "host: {}  http: {}  type: {}\n",
        d.host, d.http_version, d.resource_type
    ));
    if let Some(ip) = &d.server_ip {
        out.push_str(&format!("server ip: {ip}\n"));
    }
    out.push_str(&format!(
        "offset: {}ms  duration: {}ms\n",
        d.started_offset_ms as i64, d.duration_ms as i64
    ));
    if !d.query.is_empty() {
        out.push_str("query:\n");
        for (k, v) in &d.query {
            out.push_str(&format!("  {k} = {v}\n"));
        }
    }
    out.push_str("request headers:\n");
    for (k, v) in &d.req_headers {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push_str("response headers:\n");
    for (k, v) in &d.resp_headers {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    if let Some(b) = &d.req_body_snippet {
        out.push_str(&format!("request body: {b}\n"));
    }
    if let Some(b) = &d.resp_body_snippet {
        out.push_str(&format!("response body: {b}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{entry_detail, find_entry};
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer secret".into())];
        e.query = vec![
            ("access_token".into(), "leak".into()),
            ("page".into(), "2".into()),
        ];
        e.resp_body = Some(r#"{"token":"abc","ok":true}"#.to_string());
        let e1 = sample_entry(1, "api.x", "GET", "/other", 200);
        sample_capture(vec![e, e1])
    }

    #[test]
    fn finds_by_id_and_index() {
        let c = cap();
        assert_eq!(find_entry(&c, "e000001").unwrap().norm_path, "/other");
        assert_eq!(find_entry(&c, "1").unwrap().norm_path, "/other");
        assert!(find_entry(&c, "e999999").is_none());
    }

    #[test]
    fn redacts_headers_query_and_body_by_default() {
        let c = cap();
        let d = entry_detail(find_entry(&c, "e000000").unwrap(), false);
        // header value redacted
        let auth = d
            .req_headers
            .iter()
            .find(|(n, _)| n == "Authorization")
            .unwrap();
        assert_eq!(auth.1, "<redacted>");
        // sensitive query redacted, safe one kept
        let tok = d.query.iter().find(|(n, _)| n == "access_token").unwrap();
        assert_eq!(tok.1, "<redacted>");
        let page = d.query.iter().find(|(n, _)| n == "page").unwrap();
        assert_eq!(page.1, "2");
        // body token redacted
        assert!(!d.resp_body_snippet.as_deref().unwrap().contains("abc"));
    }

    #[test]
    fn unsafe_shows_raw() {
        let c = cap();
        let d = entry_detail(find_entry(&c, "e000000").unwrap(), true);
        let auth = d
            .req_headers
            .iter()
            .find(|(n, _)| n == "Authorization")
            .unwrap();
        assert_eq!(auth.1, "Bearer secret");
    }

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
}
