use crate::errorbody::parse_error_fields;
use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::redact_body;
use ahash::AHashMap;
use serde::Serialize;

const SNIPPET_MAX: usize = 200;

#[derive(Debug, Serialize)]
pub struct ErrorsResult {
    pub groups: Vec<ErrorGroup>,
}

#[derive(Debug, Serialize)]
pub struct ErrorGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub body_snippet: Option<String>,
    pub correlation_ids: Vec<String>,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

/// Group 4xx/5xx/failed responses by (host, method, norm_path, status).
/// `unsafe_include` disables body redaction. `top` bounds the list.
pub fn compute_errors(
    cap: &Capture,
    filter: &Filter,
    top: usize,
    unsafe_include: bool,
) -> ErrorsResult {
    let mut by_key: AHashMap<(String, String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in cap
        .entries
        .iter()
        .filter(|e| filter.matches(e) && e.is_error())
    {
        let key = (
            e.host.clone(),
            e.method.to_ascii_uppercase(),
            e.norm_path.clone(),
            e.status,
        );
        by_key.entry(key).or_default().push(e);
    }

    let mut groups: Vec<ErrorGroup> = by_key
        .into_iter()
        .map(|((host, method, norm_path, status), mut g)| {
            g.sort_by(|a, b| {
                a.started_offset_ms
                    .partial_cmp(&b.started_offset_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.index.cmp(&b.index))
            });
            error_group(host, method, norm_path, status, &g, unsafe_include)
        })
        .collect();

    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.status.cmp(&a.status))
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    ErrorsResult { groups }
}

fn error_group(
    host: String,
    method: String,
    norm_path: String,
    status: i64,
    g: &[&Entry],
    unsafe_include: bool,
) -> ErrorGroup {
    let sample = g[0];
    let fields = sample
        .resp_body
        .as_deref()
        .map(parse_error_fields)
        .unwrap_or_default();
    let body_snippet = sample
        .resp_body
        .as_deref()
        .filter(|b| !b.is_empty())
        .map(|b| redact_body(b, unsafe_include, SNIPPET_MAX));
    let correlation_ids: Vec<String> = sample.correlation.iter().map(|(_, v)| v.clone()).collect();

    ErrorGroup {
        host,
        method,
        norm_path,
        status,
        count: g.len(),
        error_message: fields.message,
        error_code: fields.code,
        body_snippet,
        correlation_ids,
        entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
        last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
    }
}

/// Render errors as deterministic terminal text.
pub fn render_errors_text(r: &ErrorsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail errors ==\n");
    for g in &r.groups {
        out.push_str(&format!(
            "\n{:>4}x  [{}] {} {}{}\n",
            g.count, g.status, g.method, g.host, g.norm_path
        ));
        if let Some(m) = &g.error_message {
            out.push_str(&format!("  message: {m}\n"));
        }
        if let Some(c) = &g.error_code {
            out.push_str(&format!("  code: {c}\n"));
        }
        if !g.correlation_ids.is_empty() {
            out.push_str(&format!(
                "  correlation: {}\n",
                g.correlation_ids.join(", ")
            ));
        }
        if let Some(s) = &g.body_snippet {
            out.push_str(&format!("  body: {s}\n"));
        }
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_errors;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e0 = sample_entry(0, "api.x", "POST", "/bulk", 500);
        e0.resp_body = Some(r#"{"message":"boom","code":"E500"}"#.to_string());
        let mut e1 = sample_entry(1, "api.x", "POST", "/bulk", 500);
        e1.resp_body = Some(r#"{"message":"boom","code":"E500"}"#.to_string());
        let e2 = sample_entry(2, "api.x", "GET", "/ok", 200); // not an error
        let e3 = sample_entry(3, "api.x", "GET", "/missing", 404);
        sample_capture(vec![e0, e1, e2, e3])
    }

    #[test]
    fn groups_4xx_5xx_only() {
        let r = compute_errors(&cap(), &Filter::parse(&[]).unwrap(), 10, false);
        // /bulk 500 (x2) and /missing 404 (x1) -> 2 groups; /ok excluded
        assert_eq!(r.groups.len(), 2);
        let bulk = r.groups.iter().find(|g| g.norm_path == "/bulk").unwrap();
        assert_eq!(bulk.count, 2);
        assert_eq!(bulk.status, 500);
        assert_eq!(bulk.error_message.as_deref(), Some("boom"));
        assert_eq!(bulk.error_code.as_deref(), Some("E500"));
        assert_eq!(bulk.entry_ids, vec!["e000000", "e000001"]);
    }

    #[test]
    fn redacts_body_snippet_by_default() {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 401);
        e.resp_body = Some(r#"{"access_token":"leak","message":"no"}"#.to_string());
        let r = compute_errors(
            &sample_capture(vec![e]),
            &Filter::parse(&[]).unwrap(),
            10,
            false,
        );
        let snip = r.groups[0].body_snippet.as_deref().unwrap();
        assert!(!snip.contains("leak"));
    }
}
