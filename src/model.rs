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

#[cfg(test)]
pub fn sample_entry(index: usize, host: &str, method: &str, path: &str, status: i64) -> Entry {
    Entry {
        id: format_entry_id(index),
        index,
        started_offset_ms: index as f64 * 10.0,
        duration_ms: 10.0,
        method: method.to_string(),
        url: format!("https://{host}{path}"),
        host: host.to_string(),
        path: path.to_string(),
        norm_path: path.to_string(),
        query: Vec::new(),
        status,
        status_text: String::new(),
        resource_type: ResourceType::Api,
        content_type: Some("application/json".to_string()),
        req_headers: Vec::new(),
        resp_headers: Vec::new(),
        req_body: None,
        resp_body: None,
        timings: Phases::default(),
        sizes: Sizes::default(),
        server_ip: None,
        http_version: "HTTP/2".to_string(),
        redirect_url: None,
        correlation: Vec::new(),
    }
}

#[cfg(test)]
pub fn sample_capture(entries: Vec<Entry>) -> Capture {
    let meta = CaptureMeta {
        har_version: "1.2".to_string(),
        creator: "test".to_string(),
        creator_version: "0".to_string(),
        browser: None,
        entry_count: entries.len(),
        start_ms: Some(0),
        end_ms: Some(0),
        duration_ms: 0.0,
    };
    Capture { meta, entries }
}
