use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TransitionsResult {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Serialize)]
pub struct Transition {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub from_status: i64,
    pub to_status: i64,
    pub from_id: String,
    pub to_id: String,
    pub gap_ms: f64,
    pub label: String,
}

/// Detect endpoint-local status transitions where a failed attempt is followed
/// by another attempt of the same (method, host, norm_path). `top` bounds the list.
pub fn compute_transitions(cap: &Capture, filter: &Filter, top: usize) -> TransitionsResult {
    // Group by endpoint, preserving time order.
    let mut by_key: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let key = (e.method.to_ascii_uppercase(), e.host.clone(), e.norm_path.clone());
        by_key.entry(key).or_default().push(e);
    }

    let mut transitions: Vec<Transition> = Vec::new();
    for (_, mut group) in by_key {
        group.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });
        for w in group.windows(2) {
            let (prev, curr) = (w[0], w[1]);
            if let Some(label) = label_for(prev.status, curr.status) {
                transitions.push(Transition {
                    host: prev.host.clone(),
                    method: prev.method.to_ascii_uppercase(),
                    norm_path: prev.norm_path.clone(),
                    from_status: prev.status,
                    to_status: curr.status,
                    from_id: prev.id.clone(),
                    to_id: curr.id.clone(),
                    gap_ms: (curr.started_offset_ms - prev.started_offset_ms).max(0.0),
                    label: label.to_string(),
                });
            }
        }
    }

    transitions.sort_by(|a, b| a.from_id.cmp(&b.from_id).then(a.to_id.cmp(&b.to_id)));
    transitions.truncate(top);
    TransitionsResult { transitions }
}

/// Classify a transition between two consecutive same-endpoint attempts. Returns
/// None when the prior attempt did not fail (no transition worth reporting).
fn label_for(prev: i64, curr: i64) -> Option<&'static str> {
    match (prev, curr) {
        (401 | 403, c) if class_of(c) == 2 => Some("auth-recovered"),
        (429, 429) => Some("rate-limit-persisted"),
        (429, c) if class_of(c) == 2 => Some("rate-limit-recovered"),
        (p, c) if class_of(p) == 5 && class_of(c) == 2 => Some("recovered-5xx"),
        (p, c) if is_failure(p) && c != p && is_failure(c) => Some("error-changed"),
        _ => None,
    }
}

fn class_of(status: i64) -> i64 {
    if (100..600).contains(&status) {
        status / 100
    } else {
        0
    }
}

fn is_failure(status: i64) -> bool {
    status == 0 || class_of(status) == 4 || class_of(status) == 5
}

/// Render transitions as deterministic terminal text.
pub fn render_transitions_text(r: &TransitionsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail transitions ==\n");
    for t in &r.transitions {
        out.push_str(&format!(
            "\n{} -> {}  [{}]  {} {}{}\n",
            t.from_status, t.to_status, t.label, t.method, t.host, t.norm_path
        ));
        out.push_str(&format!(
            "  {} -> {}  (gap {})\n",
            t.from_id,
            t.to_id,
            human_ms(t.gap_ms)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_transitions;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    #[test]
    fn detects_auth_recovery() {
        // same endpoint: 401 then 200 -> auth-recovered
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/me", 401),
            sample_entry(1, "h", "GET", "/me", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.transitions.len(), 1);
        let t = &r.transitions[0];
        assert_eq!(t.from_status, 401);
        assert_eq!(t.to_status, 200);
        assert_eq!(t.label, "auth-recovered");
        assert_eq!(t.from_id, "e000000");
        assert_eq!(t.to_id, "e000001");
    }

    #[test]
    fn detects_rate_limit_persisted_and_recovered_5xx() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 429),
            sample_entry(1, "h", "GET", "/a", 429),
            sample_entry(2, "h", "POST", "/b", 500),
            sample_entry(3, "h", "POST", "/b", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.transitions.iter().any(|t| t.label == "rate-limit-persisted"));
        assert!(r.transitions.iter().any(|t| t.label == "recovered-5xx"));
    }

    #[test]
    fn no_transition_when_no_prior_error() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/a", 200),
        ]);
        let r = compute_transitions(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.transitions.is_empty());
    }
}
