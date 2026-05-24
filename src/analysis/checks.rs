use crate::config::Config;
use crate::filter::Filter;
use crate::glob::glob_match;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ChecksResult {
    pub findings: Vec<CheckFinding>,
}

#[derive(Debug, Serialize)]
pub struct CheckFinding {
    pub rule: String,
    pub host: String,
    pub norm_path: String,
    pub detail: String,
    pub entry_ids: Vec<String>,
}

fn req_content_type(e: &Entry) -> Option<String> {
    e.req_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_ascii_lowercase())
}

fn looks_like_json(body: &str) -> bool {
    let t = body.trim_start();
    t.starts_with('{') || t.starts_with('[')
}

fn content_type_issues(e: &Entry) -> Vec<String> {
    let mut v = Vec::new();
    let req_ct = req_content_type(e).unwrap_or_default();
    if let Some(b) = e.req_body.as_deref().filter(|b| !b.is_empty()) {
        if looks_like_json(b) && !req_ct.contains("json") {
            v.push("request JSON body without application/json content-type".to_string());
        }
    }
    let resp_ct = e.content_type.clone().unwrap_or_default().to_ascii_lowercase();
    match e.resp_body.as_deref().filter(|b| !b.is_empty()) {
        Some(b) => {
            if looks_like_json(b) && resp_ct.contains("html") {
                v.push("JSON response served as text/html".to_string());
            }
        }
        None => {
            if resp_ct.contains("json") && e.status == 200 {
                v.push("empty body with JSON content-type".to_string());
            }
        }
    }
    v
}

/// Run built-in checks: missing required headers (config) + content-type mismatch.
pub fn compute_checks(cap: &Capture, filter: &Filter, config: &Config, top: usize) -> ChecksResult {
    // key = (rule, host, norm_path, detail) -> entry ids
    let mut map: AHashMap<(String, String, String, String), Vec<String>> = AHashMap::new();

    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        // missing required headers
        for rule in &config.required_headers {
            if glob_match(&rule.host, &e.host) {
                for h in &rule.headers {
                    let present = e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(h));
                    if !present {
                        let key = (
                            "missing-header".to_string(),
                            e.host.clone(),
                            e.norm_path.clone(),
                            format!("missing required header: {h}"),
                        );
                        map.entry(key).or_default().push(e.id.clone());
                    }
                }
            }
        }
        // content-type mismatches
        for detail in content_type_issues(e) {
            let key = ("content-type".to_string(), e.host.clone(), e.norm_path.clone(), detail);
            map.entry(key).or_default().push(e.id.clone());
        }
    }

    let mut findings: Vec<CheckFinding> = map
        .into_iter()
        .map(|((rule, host, norm_path, detail), entry_ids)| CheckFinding {
            rule,
            host,
            norm_path,
            detail,
            entry_ids,
        })
        .collect();
    findings.sort_by(|a, b| {
        b.entry_ids
            .len()
            .cmp(&a.entry_ids.len())
            .then(a.rule.cmp(&b.rule))
            .then(a.host.cmp(&b.host))
            .then(a.detail.cmp(&b.detail))
    });
    findings.truncate(top);
    ChecksResult { findings }
}

/// Render checks findings as deterministic terminal text.
pub fn render_checks_text(r: &ChecksResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail checks ==\n");
    for f in &r.findings {
        out.push_str(&format!(
            "\n[{}] {} {}\n  {} ({} entries)\n",
            f.rule,
            f.host,
            f.norm_path,
            f.detail,
            f.entry_ids.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_checks;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cfg_required(host: &str, headers: &[&str]) -> Config {
        let yaml = format!(
            "required_headers:\n  - host: \"{host}\"\n    headers: [{}]\n",
            headers.iter().map(|h| format!("\"{h}\"")).collect::<Vec<_>>().join(", ")
        );
        Config::from_yaml_str(&yaml).unwrap()
    }

    #[test]
    fn flags_missing_required_header() {
        let e = sample_entry(0, "api.x", "GET", "/data", 200); // no Authorization
        let cap = sample_capture(vec![e]);
        let cfg = cfg_required("api.x", &["Authorization"]);
        let r = compute_checks(&cap, &Filter::parse(&[]).unwrap(), &cfg, 50);
        assert!(r.findings.iter().any(|f| f.rule == "missing-header"
            && f.detail.contains("Authorization")
            && f.entry_ids.contains(&"e000000".to_string())));
    }

    #[test]
    fn present_header_not_flagged() {
        let mut e = sample_entry(0, "api.x", "GET", "/data", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        let cfg = cfg_required("api.x", &["Authorization"]);
        let r = compute_checks(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), &cfg, 50);
        assert!(r.findings.iter().all(|f| f.rule != "missing-header"));
    }

    #[test]
    fn flags_json_body_without_json_content_type() {
        let mut e = sample_entry(0, "api.x", "POST", "/data", 200);
        e.req_headers = vec![("Content-Type".into(), "text/plain".into())];
        e.req_body = Some(r#"{"a":1}"#.to_string());
        let r = compute_checks(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), &Config::default(), 50);
        assert!(r.findings.iter().any(|f| f.rule == "content-type"
            && f.detail.contains("JSON body")));
    }
}
