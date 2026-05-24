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
    items
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect()
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
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

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
