use crate::filter::Filter;
use crate::model::{Capture, Entry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CascadeResult {
    pub first_failure: Option<FailureContext>,
    pub cascades: Vec<Cascade>,
}

#[derive(Debug, Serialize)]
pub struct FailureContext {
    pub id: String,
    pub status: i64,
    pub host: String,
    pub norm_path: String,
    pub before_ids: Vec<String>,
    pub after_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Cascade {
    pub trigger_id: String,
    pub trigger_kind: String,
    pub downstream_failures: usize,
    pub downstream_ids: Vec<String>,
}

fn trigger_kind(np: &str) -> &'static str {
    let p = np.to_ascii_lowercase();
    if p.contains("/config") {
        "config"
    } else if p.contains("/auth") || p.contains("/token") || p.contains("/oauth") {
        "auth"
    } else if p.contains("bootstrap") || p.contains("/init") {
        "bootstrap"
    } else {
        "request"
    }
}

/// Find the earliest failure and downstream failure cascades.
pub fn compute_cascade(
    cap: &Capture,
    filter: &Filter,
    window_ms: u64,
    min_downstream: usize,
    top: usize,
) -> CascadeResult {
    let mut entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();
    entries.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    // first failure + up to 3 neighbors each side (in time order).
    let first_failure = entries.iter().position(|e| e.is_error()).map(|pos| {
        let e = entries[pos];
        let before_ids = entries[pos.saturating_sub(3)..pos]
            .iter()
            .map(|x| x.id.clone())
            .collect();
        let after_ids = entries[pos + 1..(pos + 4).min(entries.len())]
            .iter()
            .map(|x| x.id.clone())
            .collect();
        FailureContext {
            id: e.id.clone(),
            status: e.status,
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            before_ids,
            after_ids,
        }
    });

    // cascades: a failure followed by >= min_downstream failures within window_ms.
    let w = window_ms as f64;
    let mut cascades: Vec<Cascade> = Vec::new();
    for (i, trigger) in entries.iter().enumerate() {
        if !trigger.is_error() {
            continue;
        }
        let t = trigger.started_offset_ms;
        let downstream: Vec<String> = entries[i + 1..]
            .iter()
            .filter(|e| e.is_error() && e.started_offset_ms > t && e.started_offset_ms <= t + w)
            .map(|e| e.id.clone())
            .collect();
        if downstream.len() >= min_downstream {
            cascades.push(Cascade {
                trigger_id: trigger.id.clone(),
                trigger_kind: trigger_kind(&trigger.norm_path).to_string(),
                downstream_failures: downstream.len(),
                downstream_ids: downstream.into_iter().take(top).collect(),
            });
        }
    }
    cascades.sort_by(|a, b| {
        b.downstream_failures
            .cmp(&a.downstream_failures)
            .then(a.trigger_id.cmp(&b.trigger_id))
    });
    cascades.truncate(top);

    CascadeResult { first_failure, cascades }
}

/// Render cascades as deterministic terminal text.
pub fn render_cascade_text(r: &CascadeResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail cascade ==\n");
    if let Some(f) = &r.first_failure {
        out.push_str(&format!(
            "\nfirst failure: {} [{}] {}{}\n",
            f.id, f.status, f.host, f.norm_path
        ));
        out.push_str(&format!("  before: {}\n", f.before_ids.join(", ")));
        out.push_str(&format!("  after:  {}\n", f.after_ids.join(", ")));
    } else {
        out.push_str("\nno failures in capture\n");
    }
    if !r.cascades.is_empty() {
        out.push_str("\ncascades:\n");
        for c in &r.cascades {
            out.push_str(&format!(
                "  {} [{}] -> {} downstream failures\n",
                c.trigger_id, c.trigger_kind, c.downstream_failures
            ));
            out.push_str(&format!("    {}\n", c.downstream_ids.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_cascade;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    fn at(index: usize, path: &str, status: i64, offset: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", path, status);
        e.started_offset_ms = offset;
        e
    }

    #[test]
    fn finds_first_failure_with_neighbors() {
        let cap = sample_capture(vec![
            at(0, "/ok1", 200, 0.0),
            at(1, "/boom", 500, 10.0),
            at(2, "/ok2", 200, 20.0),
        ]);
        let r = compute_cascade(&cap, &Filter::parse(&[]).unwrap(), 5000, 3, 10);
        let f = r.first_failure.unwrap();
        assert_eq!(f.id, "e000001");
        assert!(f.before_ids.contains(&"e000000".to_string()));
        assert!(f.after_ids.contains(&"e000002".to_string()));
    }

    #[test]
    fn detects_cascade_from_config_failure() {
        let mut es = vec![at(0, "/config", 500, 0.0)];
        for i in 1..=4 {
            es.push(at(i, "/data", 500, i as f64 * 100.0));
        }
        let r = compute_cascade(&sample_capture(es), &Filter::parse(&[]).unwrap(), 5000, 3, 10);
        let c = r.cascades.iter().find(|c| c.trigger_id == "e000000").unwrap();
        assert_eq!(c.trigger_kind, "config");
        assert!(c.downstream_failures >= 3);
    }
}
