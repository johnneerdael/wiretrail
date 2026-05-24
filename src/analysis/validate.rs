use crate::model::{Capture, Entry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ValidateResult {
    pub har_version: String,
    pub creator: String,
    pub entry_count: usize,
    pub pct_with_timings: f64,
    pub pct_with_resp_body: f64,
    pub pct_post_with_req_body: f64,
    pub with_auth: bool,
    pub with_cookies: bool,
    pub anomalies: Vec<Anomaly>,
    pub sanitized: bool,
    pub sufficiency_notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Anomaly {
    pub kind: String,
    pub count: usize,
}

fn has_header(e: &Entry, name: &str) -> bool {
    e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
}

fn has_body(b: &Option<String>) -> bool {
    b.as_deref().is_some_and(|s| !s.is_empty())
}

/// Assess HAR quality and analysis sufficiency.
pub fn compute_validate(cap: &Capture) -> ValidateResult {
    let n = cap.entries.len();
    let denom = n.max(1) as f64;

    let with_timings = cap
        .entries
        .iter()
        .filter(|e| e.timings.wait > 0.0 || e.timings.receive > 0.0 || e.timings.send > 0.0)
        .count();
    let with_resp_body = cap.entries.iter().filter(|e| has_body(&e.resp_body)).count();

    let posts: Vec<&Entry> = cap
        .entries
        .iter()
        .filter(|e| matches!(e.method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH"))
        .collect();
    let posts_with_body = posts.iter().filter(|e| has_body(&e.req_body)).count();

    let with_auth = cap.entries.iter().any(|e| has_header(e, "authorization"));
    let with_cookies = cap.entries.iter().any(|e| has_header(e, "cookie"));

    let count = |pred: &dyn Fn(&Entry) -> bool, kind: &str| -> Option<Anomaly> {
        let c = cap.entries.iter().filter(|e| pred(e)).count();
        if c > 0 {
            Some(Anomaly { kind: kind.to_string(), count: c })
        } else {
            None
        }
    };
    let mut anomalies = Vec::new();
    if let Some(a) = count(&|e| e.status == 0, "status-0") {
        anomalies.push(a);
    }
    if let Some(a) = count(&|e| e.method.is_empty(), "missing-method") {
        anomalies.push(a);
    }
    if let Some(a) = count(
        &|e| e.duration_ms == 0.0 && has_body(&e.resp_body),
        "zero-duration-with-body",
    ) {
        anomalies.push(a);
    }
    if let Some(a) = count(
        &|e| e.sizes.resp_body < -1 || e.sizes.req_body < -1 || e.sizes.resp_content < -1,
        "negative-size",
    ) {
        anomalies.push(a);
    }

    let pct_with_resp_body = with_resp_body as f64 / denom;
    let sanitized = !with_auth && !with_cookies && pct_with_resp_body < 0.10;

    let mut notes = Vec::new();
    if pct_with_resp_body < 0.10 {
        notes.push("few/no response bodies captured — `errors`/`search`/`extract` limited".to_string());
    }
    if !with_auth {
        notes.push("no Authorization headers — `auth`/`jwt` limited".to_string());
    }
    if !posts.is_empty() && posts_with_body == 0 {
        notes.push("no request bodies on POST/PUT/PATCH — `diff` body verdicts limited".to_string());
    }
    if with_timings == 0 {
        notes.push("no timing data — `slowest`/`startup` limited".to_string());
    }

    ValidateResult {
        har_version: cap.meta.har_version.clone(),
        creator: cap.meta.creator.clone(),
        entry_count: n,
        pct_with_timings: with_timings as f64 / denom,
        pct_with_resp_body,
        pct_post_with_req_body: if posts.is_empty() {
            0.0
        } else {
            posts_with_body as f64 / posts.len() as f64
        },
        with_auth,
        with_cookies,
        anomalies,
        sanitized,
        sufficiency_notes: notes,
    }
}

fn pct(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// Render the validation report as deterministic terminal text.
pub fn render_validate_text(r: &ValidateResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail validate ==\n");
    out.push_str(&format!(
        "HAR {} via {}  ({} entries)\n",
        r.har_version, r.creator, r.entry_count
    ));
    out.push_str(&format!(
        "with timings: {} · response bodies: {} · POST req bodies: {}\n",
        pct(r.pct_with_timings),
        pct(r.pct_with_resp_body),
        pct(r.pct_post_with_req_body)
    ));
    out.push_str(&format!("auth headers: {} · cookies: {}\n", r.with_auth, r.with_cookies));
    if r.sanitized {
        out.push_str("sanitized: yes (no auth/cookies and few response bodies)\n");
    }
    if !r.anomalies.is_empty() {
        out.push_str("\nanomalies:\n");
        for a in &r.anomalies {
            out.push_str(&format!("  {}: {}\n", a.kind, a.count));
        }
    }
    if !r.sufficiency_notes.is_empty() {
        out.push_str("\nsufficiency:\n");
        for n in &r.sufficiency_notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_validate;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn flags_sanitized_capture_with_no_bodies_or_auth() {
        let cap = sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.x", "GET", "/b", 200),
        ]);
        let r = compute_validate(&cap);
        assert!(r.sanitized);
        assert!(!r.with_auth);
        assert_eq!(r.pct_with_resp_body, 0.0);
        assert!(r.sufficiency_notes.iter().any(|n| n.contains("response bodies")));
    }

    #[test]
    fn detects_status_zero_anomaly() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 0)]);
        let r = compute_validate(&cap);
        assert!(r.anomalies.iter().any(|a| a.kind == "status-0" && a.count == 1));
    }

    #[test]
    fn reports_body_and_auth_presence() {
        let mut e = sample_entry(0, "api.x", "POST", "/a", 200);
        e.req_headers = vec![("Authorization".into(), "Bearer x".into())];
        e.req_body = Some(r#"{"k":1}"#.into());
        e.resp_body = Some(r#"{"ok":true}"#.into());
        let r = compute_validate(&sample_capture(vec![e]));
        assert!(r.with_auth);
        assert_eq!(r.pct_with_resp_body, 1.0);
        assert_eq!(r.pct_post_with_req_body, 1.0);
        assert!(!r.sanitized);
    }
}
