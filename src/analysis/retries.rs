use crate::filter::Filter;
use crate::grouping::{group_by_fingerprint, group_has_retry, is_retry_trigger};
use crate::model::Capture;
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RetriesResult {
    pub groups: Vec<RetryGroup>,
}

#[derive(Debug, Serialize)]
pub struct RetryGroup {
    pub fingerprint: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub attempts: usize,
    pub retry_count: usize,
    pub trigger_statuses: Vec<i64>,
    pub gaps_ms: Vec<f64>,
    pub entry_ids: Vec<String>,
    pub final_status: i64,
}

/// Report fingerprint groups that exhibit retry behavior (an attempt following
/// a failed earlier attempt). `top` bounds the list.
pub fn compute_retries(cap: &Capture, filter: &Filter, top: usize) -> RetriesResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut groups: Vec<RetryGroup> = group_by_fingerprint(&entries)
        .into_iter()
        .filter(|(_, g)| group_has_retry(g))
        .map(|(fp, g)| retry_group(fp, &g))
        .collect();

    groups.sort_by(|a, b| {
        b.retry_count
            .cmp(&a.retry_count)
            .then(a.fingerprint.cmp(&b.fingerprint))
    });
    groups.truncate(top);
    RetriesResult { groups }
}

fn retry_group(fingerprint: String, g: &[&crate::model::Entry]) -> RetryGroup {
    let mut retry_count = 0usize;
    let mut trigger_statuses: Vec<i64> = Vec::new();
    let mut seen_failure = false;
    for e in g {
        if seen_failure {
            retry_count += 1;
        }
        if is_retry_trigger(e) {
            seen_failure = true;
            if !trigger_statuses.contains(&e.status) {
                trigger_statuses.push(e.status);
            }
        }
    }
    let gaps_ms: Vec<f64> = g
        .windows(2)
        .map(|w| (w[1].started_offset_ms - w[0].started_offset_ms).max(0.0))
        .collect();

    RetryGroup {
        fingerprint,
        method: g[0].method.to_ascii_uppercase(),
        host: g[0].host.clone(),
        norm_path: g[0].norm_path.clone(),
        attempts: g.len(),
        retry_count,
        trigger_statuses,
        gaps_ms,
        entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        final_status: g.last().map(|e| e.status).unwrap_or(0),
    }
}

/// Render retries as deterministic terminal text.
pub fn render_retries_text(r: &RetriesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail retries ==\n");
    for g in &r.groups {
        let triggers: Vec<String> = g.trigger_statuses.iter().map(|s| s.to_string()).collect();
        out.push_str(&format!(
            "\n{} {}{}  ({} attempts, {} retries, final {})\n",
            g.method, g.host, g.norm_path, g.attempts, g.retry_count, g.final_status
        ));
        out.push_str(&format!("  triggered by: {}\n", triggers.join(", ")));
        let gaps: Vec<String> = g.gaps_ms.iter().map(|ms| human_ms(*ms)).collect();
        out.push_str(&format!("  backoff gaps: {}\n", gaps.join(", ")));
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_retries;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "h", "POST", "/bulk", 500),
            sample_entry(1, "h", "POST", "/bulk", 500),
            sample_entry(2, "h", "POST", "/bulk", 200),
            sample_entry(3, "h", "GET", "/clean", 200),
            sample_entry(4, "h", "GET", "/clean", 200), // pure duplicate, not a retry
        ];
        entries[1].started_offset_ms = 100.0;
        entries[2].started_offset_ms = 300.0;
        sample_capture(entries)
    }

    #[test]
    fn reports_only_retry_groups() {
        let r = compute_retries(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.attempts, 3);
        assert_eq!(g.retry_count, 2);
        assert_eq!(g.final_status, 200);
        assert!(g.trigger_statuses.contains(&500));
        // gaps between consecutive attempts: 100-0=100, 300-100=200
        assert_eq!(g.gaps_ms, vec![100.0, 200.0]);
    }
}
