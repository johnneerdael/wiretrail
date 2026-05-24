use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct EndpointsResult {
    pub endpoints: Vec<EndpointStat>,
}

#[derive(Debug, Serialize)]
pub struct EndpointStat {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub count: usize,
    pub statuses: BTreeMap<String, usize>,
    pub content_types: Vec<String>,
    pub sample_query_keys: Vec<String>,
    pub error_count: usize,
}

struct Acc<'a> {
    entries: Vec<&'a Entry>,
}

/// Build a normalized endpoint inventory, keyed by (method, host, norm_path).
pub fn compute_endpoints(cap: &Capture, filter: &Filter, top: usize) -> EndpointsResult {
    let mut by_key: AHashMap<(String, String, String), Acc> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let key = (e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone());
        by_key.entry(key).or_insert_with(|| Acc { entries: Vec::new() }).entries.push(e);
    }

    let mut endpoints: Vec<EndpointStat> = by_key
        .into_iter()
        .map(|((method, host, norm_path), acc)| endpoint_stat(method, host, norm_path, &acc.entries))
        .collect();

    endpoints.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
            .then(a.method.cmp(&b.method))
    });
    endpoints.truncate(top);
    EndpointsResult { endpoints }
}

fn endpoint_stat(method: String, host: String, norm_path: String, entries: &[&Entry]) -> EndpointStat {
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut content_types: AHashSet<String> = AHashSet::new();
    let mut query_keys: AHashSet<String> = AHashSet::new();
    let mut error_count = 0usize;

    for e in entries {
        *statuses.entry(e.status.to_string()).or_default() += 1;
        if let Some(ct) = &e.content_type {
            content_types.insert(ct.clone());
        }
        for (k, _) in &e.query {
            query_keys.insert(k.clone());
        }
        if e.is_error() {
            error_count += 1;
        }
    }

    let mut content_types: Vec<String> = content_types.into_iter().collect();
    content_types.sort();
    let mut sample_query_keys: Vec<String> = query_keys.into_iter().collect();
    sample_query_keys.sort();

    EndpointStat {
        host,
        method,
        norm_path,
        count: entries.len(),
        statuses,
        content_types,
        sample_query_keys,
        error_count,
    }
}

/// Render the endpoint inventory as deterministic terminal text.
pub fn render_endpoints_text(r: &EndpointsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail endpoints ==\n");
    for e in &r.endpoints {
        let statuses: Vec<String> = e.statuses.iter().map(|(s, c)| format!("{s}:{c}")).collect();
        out.push_str(&format!(
            "\n{:>4}  {} {}{}\n",
            e.count, e.method, e.host, e.norm_path
        ));
        out.push_str(&format!("  status: {}\n", statuses.join(" ")));
        if !e.content_types.is_empty() {
            out.push_str(&format!("  content-types: {}\n", e.content_types.join(", ")));
        }
        if !e.sample_query_keys.is_empty() {
            out.push_str(&format!("  query keys: {}\n", e.sample_query_keys.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_endpoints;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut e0 = sample_entry(0, "api.foo.com", "GET", "/v1/users/{id}", 200);
        e0.query = vec![("page".into(), "1".into())];
        let mut e1 = sample_entry(1, "api.foo.com", "GET", "/v1/users/{id}", 404);
        e1.query = vec![("expand".into(), "true".into())];
        let e2 = sample_entry(2, "api.foo.com", "POST", "/v1/users/{id}", 200);
        sample_capture(vec![e0, e1, e2])
    }

    #[test]
    fn groups_by_method_host_normpath() {
        let r = compute_endpoints(&cap(), &Filter::parse(&[]).unwrap(), 10);
        // GET .../{id} and POST .../{id} are distinct endpoints
        let get = r
            .endpoints
            .iter()
            .find(|e| e.method == "GET" && e.norm_path == "/v1/users/{id}")
            .unwrap();
        assert_eq!(get.count, 2);
        assert_eq!(get.statuses.get("200"), Some(&1));
        assert_eq!(get.statuses.get("404"), Some(&1));
        assert_eq!(get.error_count, 1);
        // observed query keys are collected and sorted/deduped
        assert_eq!(get.sample_query_keys, vec!["expand".to_string(), "page".to_string()]);
        assert!(r.endpoints.iter().any(|e| e.method == "POST"));
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_endpoints(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.endpoints[0].count, 2);
    }
}
