use crate::filter::Filter;
use crate::jsonpath;
use crate::model::{Capture, Entry};
use crate::opaque::is_opaque;
use crate::redact::REDACTED;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Req,
    Resp,
}

#[derive(Debug, Serialize)]
pub struct ExtractResult {
    pub values: Vec<ExtractValue>,
}

#[derive(Debug, Serialize)]
pub struct ExtractValue {
    pub id: String,
    pub value: String,
}

fn body_of(e: &Entry, target: Target) -> Option<&str> {
    let b = match target {
        Target::Req => &e.req_body,
        Target::Resp => &e.resp_body,
    };
    b.as_deref().filter(|s| !s.is_empty())
}

fn stringify(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Extract a JSON-path value from request/response bodies across entries.
pub fn compute_extract(
    cap: &Capture,
    filter: &Filter,
    path: &str,
    target: Target,
    top: usize,
    unsafe_include: bool,
) -> ExtractResult {
    let mut values = Vec::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let Some(body) = body_of(e, target) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
            continue;
        };
        for v in jsonpath::eval(&json, path) {
            let s = stringify(&v);
            let shown = if !unsafe_include && is_opaque(&s) {
                REDACTED.to_string()
            } else {
                s
            };
            values.push(ExtractValue {
                id: e.id.clone(),
                value: shown,
            });
            if values.len() >= top {
                return ExtractResult { values };
            }
        }
    }
    ExtractResult { values }
}

/// Render extracted values as deterministic terminal text.
pub fn render_extract_text(r: &ExtractResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail extract ==\n");
    for v in &r.values {
        out.push_str(&format!("{}  {}\n", v.id, v.value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Target, compute_extract};
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn with_resp(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/a", 200);
        e.resp_body = Some(body.to_string());
        e
    }

    #[test]
    fn extracts_field_from_response_bodies() {
        let cap = sample_capture(vec![
            with_resp(0, r#"{"error":{"message":"boom"}}"#),
            with_resp(1, r#"{"error":{"message":"nope"}}"#),
        ]);
        let r = compute_extract(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "$.error.message",
            Target::Resp,
            10,
            false,
        );
        let vals: Vec<&str> = r.values.iter().map(|v| v.value.as_str()).collect();
        assert!(vals.contains(&"boom"));
        assert!(vals.contains(&"nope"));
    }

    #[test]
    fn masks_opaque_value_by_default() {
        let cap = sample_capture(vec![with_resp(
            0,
            r#"{"token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"}"#,
        )]);
        let r = compute_extract(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "$.token",
            Target::Resp,
            10,
            false,
        );
        assert_eq!(r.values[0].value, "<redacted>");
        let r2 = compute_extract(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "$.token",
            Target::Resp,
            10,
            true,
        );
        assert!(r2.values[0].value.starts_with("eyJ"));
    }
}
