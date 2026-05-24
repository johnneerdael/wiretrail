use crate::filter::Filter;
use crate::grouping::{group_by_fingerprint, group_has_retry};
use crate::model::Capture;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct DuplicatesResult {
    pub groups: Vec<DuplicateGroup>,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub count: usize,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub statuses: BTreeMap<String, usize>,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub is_retry_pattern: bool,
}

/// Group repeated requests (count >= 2) by fingerprint. `top` bounds the list.
pub fn compute_duplicates(cap: &Capture, filter: &Filter, top: usize) -> DuplicatesResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut groups: Vec<DuplicateGroup> = group_by_fingerprint(&entries)
        .into_iter()
        .filter(|(_, g)| g.len() >= 2)
        .map(|(fp, g)| {
            let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
            for e in &g {
                *statuses.entry(e.status.to_string()).or_default() += 1;
            }
            DuplicateGroup {
                fingerprint: fp,
                count: g.len(),
                method: g[0].method.to_ascii_uppercase(),
                host: g[0].host.clone(),
                norm_path: g[0].norm_path.clone(),
                statuses,
                entry_ids: g.iter().map(|e| e.id.clone()).collect(),
                first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
                last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
                is_retry_pattern: group_has_retry(&g),
            }
        })
        .collect();

    groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.fingerprint.cmp(&b.fingerprint)));
    groups.truncate(top);
    DuplicatesResult { groups }
}

/// Render duplicates as deterministic terminal text.
pub fn render_duplicates_text(r: &DuplicatesResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail duplicates ==\n");
    for g in &r.groups {
        let tag = if g.is_retry_pattern { " [retry pattern]" } else { "" };
        out.push_str(&format!(
            "\n{:>4}x{}  {} {}{}\n",
            g.count, tag, g.method, g.host, g.norm_path
        ));
        let statuses: Vec<String> = g.statuses.iter().map(|(s, c)| format!("{s}:{c}")).collect();
        out.push_str(&format!("  statuses: {}\n", statuses.join(" ")));
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_duplicates;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "h", "POST", "/resolve", 200),
            sample_entry(1, "h", "POST", "/resolve", 200),
            sample_entry(2, "h", "POST", "/resolve", 200),
            sample_entry(3, "h", "GET", "/once", 200), // unique -> not a duplicate
        ])
    }

    #[test]
    fn reports_only_groups_with_repeats() {
        let r = compute_duplicates(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.count, 3);
        assert_eq!(g.method, "POST");
        assert_eq!(g.norm_path, "/resolve");
        assert_eq!(g.entry_ids, vec!["e000000", "e000001", "e000002"]);
        assert!(!g.is_retry_pattern); // all 200
    }

    #[test]
    fn flags_retry_pattern() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 500),
            sample_entry(1, "h", "POST", "/x", 200),
        ]);
        let r = compute_duplicates(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups[0].is_retry_pattern);
    }
}
