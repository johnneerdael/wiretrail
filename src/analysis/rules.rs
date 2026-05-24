use crate::config::{Config, Rule};
use crate::filter::Filter;
use crate::glob::glob_match;
use crate::model::{Capture, Entry};
use crate::opaque::is_opaque;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RulesResult {
    pub findings: Vec<RuleFinding>,
}

#[derive(Debug, Serialize)]
pub struct RuleFinding {
    pub rule: String,
    pub severity: String,
    pub detail: String,
    pub entry_ids: Vec<String>,
}

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

fn matcher_opt(pat: &Option<String>, text: &str) -> bool {
    match pat {
        Some(p) => glob_match(p, text),
        None => true,
    }
}

fn rule_matches(rule: &Rule, e: &Entry) -> bool {
    matcher_opt(&rule.host, &e.host)
        && matcher_opt(&rule.path, &e.path)
        && matcher_opt(&rule.method, &e.method)
        && matcher_opt(&rule.status, &e.status.to_string())
}

fn has_header(e: &Entry, name: &str) -> bool {
    e.req_headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case(name))
}

/// Evaluate one generic rule against an entry: `(rule_name, severity, detail)` tuples.
fn eval_rule(rule: &Rule, e: &Entry) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if !rule_matches(rule, e) {
        return out;
    }
    if rule.forbid {
        out.push((
            rule.name.clone(),
            "high".into(),
            "matched a forbidden rule".into(),
        ));
        return out;
    }
    for h in &rule.require_headers {
        if !has_header(e, h) {
            out.push((
                rule.name.clone(),
                "high".into(),
                format!("missing required header: {h}"),
            ));
        }
    }
    if let Some(budget) = rule.max_latency_ms
        && e.duration_ms > budget
    {
        out.push((
            rule.name.clone(),
            "medium".into(),
            format!("latency {:.0}ms exceeds budget {budget:.0}ms", e.duration_ms),
        ));
    }
    out
}

/// Built-in rule packs expressible with generic `Rule` fields.
fn pack_rules(pack: &str) -> Vec<Rule> {
    match pack {
        "auth" => vec![Rule {
            name: "auth: Authorization required".into(),
            require_headers: vec!["Authorization".into()],
            ..Rule::default()
        }],
        "caching" => vec![Rule {
            name: "caching: GET 200 needs Cache-Control".into(),
            method: Some("GET".into()),
            status: Some("200".into()),
            require_headers: vec!["Cache-Control".into()],
            ..Rule::default()
        }],
        "payments" => vec![
            Rule {
                name: "payments: idempotency key on charges".into(),
                path: Some("*charge*".into()),
                require_headers: vec!["Idempotency-Key".into()],
                ..Rule::default()
            },
            Rule {
                name: "payments: idempotency key on payments".into(),
                path: Some("*payment*".into()),
                require_headers: vec!["Idempotency-Key".into()],
                ..Rule::default()
            },
        ],
        _ => vec![],
    }
}

fn is_special_pack(pack: &str) -> bool {
    matches!(pack, "security" | "rest" | "graphql")
}

/// Packs that need a custom predicate (not expressible via `Rule` fields).
fn eval_special(pack: &str, e: &Entry) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    match pack {
        "security" => {
            for (k, v) in &e.query {
                if is_opaque(v) {
                    out.push((
                        "security: no secrets in query".into(),
                        "high".into(),
                        format!("opaque secret in query param `{k}`"),
                    ));
                }
            }
        }
        "rest" => {
            if e.method.eq_ignore_ascii_case("GET")
                && e.req_body.as_deref().is_some_and(|b| !b.is_empty())
            {
                out.push((
                    "rest: no mutation over GET".into(),
                    "medium".into(),
                    "GET request carries a body".into(),
                ));
            }
        }
        "graphql" => {
            if e.method.eq_ignore_ascii_case("POST")
                && glob_match("*/graphql", &e.path)
                && !e.req_body.as_deref().unwrap_or("").contains("operationName")
            {
                out.push((
                    "graphql: operationName required".into(),
                    "low".into(),
                    "GraphQL POST without operationName".into(),
                ));
            }
        }
        _ => {}
    }
    out
}

/// Evaluate config rules + built-in packs against the filtered capture.
pub fn compute_rules(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    packs: &[String],
    top: usize,
) -> RulesResult {
    let mut rules: Vec<Rule> = config.rules.clone();
    for p in packs {
        rules.extend(pack_rules(p));
    }

    // key = (rule, severity, detail) -> entry ids
    let mut map: AHashMap<(String, String, String), Vec<String>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        for rule in &rules {
            for (name, sev, detail) in eval_rule(rule, e) {
                map.entry((name, sev, detail)).or_default().push(e.id.clone());
            }
        }
        for p in packs {
            if is_special_pack(p) {
                for (name, sev, detail) in eval_special(p, e) {
                    map.entry((name, sev, detail)).or_default().push(e.id.clone());
                }
            }
        }
    }

    let mut findings: Vec<RuleFinding> = map
        .into_iter()
        .map(|((rule, severity, detail), entry_ids)| RuleFinding {
            rule,
            severity,
            detail,
            entry_ids,
        })
        .collect();
    findings.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.entry_ids.len().cmp(&a.entry_ids.len()))
            .then(a.rule.cmp(&b.rule))
            .then(a.detail.cmp(&b.detail))
    });
    findings.truncate(top);
    RulesResult { findings }
}

/// Render rule findings as deterministic terminal text.
pub fn render_rules_text(r: &RulesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail rules ==\n");
    for f in &r.findings {
        out.push_str(&format!(
            "\n[{}] {}\n  {} ({} entries)\n",
            f.severity,
            f.rule,
            f.detail,
            f.entry_ids.len()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_rules;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn no_filter() -> Filter {
        Filter::parse(&[]).unwrap()
    }

    #[test]
    fn config_rule_require_header_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: needs-auth\n    host: \"api.x\"\n    require_headers: [\"Authorization\"]\n",
        )
        .unwrap();
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(r.findings.iter().any(|f| f.rule == "needs-auth"
            && f.severity == "high"
            && f.detail.contains("Authorization")));
    }

    #[test]
    fn config_rule_max_latency_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: too-slow\n    host: \"api.x\"\n    max_latency_ms: 5\n",
        )
        .unwrap();
        // sample_entry sets duration_ms = 10.0 > 5
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == "too-slow" && f.severity == "medium")
        );
    }

    #[test]
    fn config_rule_forbid_fires() {
        let cfg = Config::from_yaml_str(
            "rules:\n  - name: no-staging\n    host: \"*.staging\"\n    forbid: true\n",
        )
        .unwrap();
        let cap = sample_capture(vec![sample_entry(0, "api.staging", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &cfg, &[], 50);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == "no-staging" && f.severity == "high")
        );
    }

    #[test]
    fn auth_pack_flags_missing_authorization() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_rules(&cap, &no_filter(), &Config::default(), &["auth".to_string()], 50);
        assert!(r.findings.iter().any(|f| f.detail.contains("Authorization")));
    }

    #[test]
    fn security_pack_flags_opaque_query_secret() {
        let mut e: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        e.query = vec![(
            "token".into(),
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ".into(),
        )];
        let r = compute_rules(
            &sample_capture(vec![e]),
            &no_filter(),
            &Config::default(),
            &["security".to_string()],
            50,
        );
        assert!(
            r.findings
                .iter()
                .any(|f| f.severity == "high" && f.detail.contains("token"))
        );
    }

    #[test]
    fn present_header_not_flagged() {
        let mut e = sample_entry(0, "api.x", "GET", "/a", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        let r = compute_rules(
            &sample_capture(vec![e]),
            &no_filter(),
            &Config::default(),
            &["auth".to_string()],
            50,
        );
        assert!(r.findings.is_empty());
    }
}
