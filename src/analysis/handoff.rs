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
