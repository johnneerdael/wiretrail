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
    format!(
        "{} {} {} {}",
        e.method.to_ascii_uppercase(),
        e.host,
        e.norm_path,
        query
    )
}

#[cfg(test)]
mod tests {
    use super::fingerprint;
    use crate::classify::ResourceType;
    use crate::model::{Entry, Phases, Sizes};

    fn entry(method: &str, host: &str, norm_path: &str, query: &[(&str, &str)]) -> Entry {
        Entry {
            id: "e0".into(),
            index: 0,
            started_offset_ms: 0.0,
            duration_ms: 0.0,
            method: method.into(),
            url: String::new(),
            host: host.into(),
            path: norm_path.into(),
            norm_path: norm_path.into(),
            query: query
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            status: 200,
            status_text: String::new(),
            resource_type: ResourceType::Api,
            content_type: None,
            req_headers: vec![],
            resp_headers: vec![],
            req_body: None,
            resp_body: None,
            timings: Phases::default(),
            sizes: Sizes::default(),
            server_ip: None,
            http_version: String::new(),
            redirect_url: None,
            correlation: vec![],
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
