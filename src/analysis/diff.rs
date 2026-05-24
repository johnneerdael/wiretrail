use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::redact::redact_query_value;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

const VOLATILE_KEYS: &[&str] = &[
    "timestamp", "ts", "nonce", "date", "cb", "cachebuster", "requestid", "request_id", "_",
];
const SAMPLE_CAP: usize = 3;

#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub groups: Vec<DiffGroup>,
}

#[derive(Debug, Serialize)]
pub struct DiffGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub count: usize,
    pub varying_query: Vec<QueryVariance>,
    pub varying_header_names: Vec<String>,
    pub body_verdict: String,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryVariance {
    pub key: String,
    pub samples: Vec<String>,
}

/// Show what varies across repeated calls to the same (method, host, norm_path).
pub fn compute_diff(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> DiffResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_route: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut groups: Vec<DiffGroup> = Vec::new();
    for ((method, host, norm_path), mut g) in by_route {
        if g.len() < 2 {
            continue;
        }
        g.sort_by(|a, b| a.index.cmp(&b.index));

        let varying_query = varying_query(&g, unsafe_include);
        let varying_header_names = varying_header_names(&g);
        let body_verdict = body_verdict(&g);

        let has_variance = !varying_query.is_empty()
            || !varying_header_names.is_empty()
            || body_verdict == "volatile-only"
            || body_verdict == "meaningful";
        if !has_variance {
            continue;
        }

        groups.push(DiffGroup {
            host,
            method,
            norm_path,
            count: g.len(),
            varying_query,
            varying_header_names,
            body_verdict,
            entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        });
    }

    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    DiffResult { groups }
}

fn varying_query(members: &[&Entry], unsafe_include: bool) -> Vec<QueryVariance> {
    let all_keys: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.query.iter().map(|(k, _)| k.clone()))
        .collect();
    let mut out = Vec::new();
    for k in all_keys {
        let mut values: Vec<String> = Vec::new();
        let mut distinct: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .query
                .iter()
                .find(|(qk, _)| *qk == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            if distinct.insert(v.clone()) {
                values.push(redact_query_value(&k, &v, unsafe_include));
            }
        }
        if distinct.len() > 1 {
            values.truncate(SAMPLE_CAP);
            out.push(QueryVariance { key: k, samples: values });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

fn varying_header_names(members: &[&Entry]) -> Vec<String> {
    let all_names: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.req_headers.iter().map(|(n, _)| n.to_ascii_lowercase()))
        .collect();
    let mut out = Vec::new();
    for name in all_names {
        let mut distinct: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .req_headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&name))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            distinct.insert(v);
        }
        if distinct.len() > 1 {
            out.push(name);
        }
    }
    out.sort();
    out
}

fn is_volatile(key: &str) -> bool {
    let lk = key.to_ascii_lowercase();
    VOLATILE_KEYS.iter().any(|v| lk == *v || lk.contains(v))
}

fn body_verdict(members: &[&Entry]) -> String {
    let bodies: Vec<&String> = members
        .iter()
        .filter_map(|e| e.req_body.as_ref().filter(|b| !b.is_empty()))
        .collect();
    if bodies.len() < 2 {
        return "none".to_string();
    }
    if bodies.iter().all(|b| *b == bodies[0]) {
        return "identical".to_string();
    }
    // Try parsing every body as a JSON object.
    let objs: Option<Vec<serde_json::Map<String, serde_json::Value>>> = bodies
        .iter()
        .map(|b| {
            serde_json::from_str::<serde_json::Value>(b)
                .ok()
                .and_then(|v| v.as_object().cloned())
        })
        .collect();
    if let Some(objs) = objs {
        let mut keys: AHashSet<String> = AHashSet::new();
        for o in &objs {
            for k in o.keys() {
                keys.insert(k.clone());
            }
        }
        let mut differing: Vec<String> = Vec::new();
        for k in &keys {
            let mut distinct: AHashSet<String> = AHashSet::new();
            for o in &objs {
                distinct.insert(o.get(k).map(|v| v.to_string()).unwrap_or_default());
            }
            if distinct.len() > 1 {
                differing.push(k.clone());
            }
        }
        if differing.iter().all(|k| is_volatile(k)) {
            return "volatile-only".to_string();
        }
        return "meaningful".to_string();
    }
    "meaningful".to_string()
}

/// Render diff groups as deterministic terminal text.
pub fn render_diff_text(r: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail diff ==\n");
    for g in &r.groups {
        out.push_str(&format!(
            "\n{} {}{}  ({} calls, body: {})\n",
            g.method, g.host, g.norm_path, g.count, g.body_verdict
        ));
        for q in &g.varying_query {
            out.push_str(&format!("  query {} varies: {}\n", q.key, q.samples.join(", ")));
        }
        if !g.varying_header_names.is_empty() {
            out.push_str(&format!("  headers vary: {}\n", g.varying_header_names.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_diff;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn post(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", "/items", 200);
        e.req_body = Some(body.to_string());
        e
    }

    #[test]
    fn body_volatile_only() {
        let cap = sample_capture(vec![
            post(0, r#"{"name":"a","ts":1}"#),
            post(1, r#"{"name":"a","ts":2}"#),
        ]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/items").unwrap();
        assert_eq!(g.body_verdict, "volatile-only");
    }

    #[test]
    fn body_meaningful() {
        let cap = sample_capture(vec![
            post(0, r#"{"name":"a"}"#),
            post(1, r#"{"name":"b"}"#),
        ]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/items").unwrap();
        assert_eq!(g.body_verdict, "meaningful");
    }

    #[test]
    fn varying_query_is_reported_and_redacted() {
        let mut a = sample_entry(0, "api.x", "GET", "/y", 200);
        a.query = vec![("page".into(), "1".into()), ("token".into(), "AAA".into())];
        let mut b = sample_entry(1, "api.x", "GET", "/y", 200);
        b.query = vec![("page".into(), "2".into()), ("token".into(), "BBB".into())];
        let r = compute_diff(&sample_capture(vec![a, b]), &Filter::parse(&[]).unwrap(), 10, false);
        let g = r.groups.iter().find(|g| g.norm_path == "/y").unwrap();
        let keys: Vec<&str> = g.varying_query.iter().map(|q| q.key.as_str()).collect();
        assert!(keys.contains(&"page"));
        // sensitive query value is redacted in samples
        let tok = g.varying_query.iter().find(|q| q.key == "token").unwrap();
        assert!(tok.samples.iter().all(|s| s == "<redacted>"));
    }

    #[test]
    fn identical_group_is_skipped() {
        let cap = sample_capture(vec![post(0, r#"{"name":"a"}"#), post(1, r#"{"name":"a"}"#)]);
        let r = compute_diff(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        assert!(r.groups.is_empty()); // no variance -> not reported (duplicates covers it)
    }
}
