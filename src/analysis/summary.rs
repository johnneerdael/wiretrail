use crate::fingerprint::fingerprint;
use crate::filter::Filter;
use crate::model::Capture;
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct SummaryResult {
    pub total_entries: usize,
    pub filtered_entries: usize,
    pub duration_ms: f64,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub resource_breakdown: BTreeMap<String, usize>,
    pub status_classes: BTreeMap<String, usize>,
    pub error_count: usize,
    pub top_hosts: Vec<HostCount>,
    pub top_duplicates: Vec<DuplicateGroup>,
    pub slowest: Vec<SlowEntry>,
    pub biggest_payloads: Vec<PayloadEntry>,
    pub hints: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HostCount {
    pub host: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub count: usize,
    pub example_id: String,
}

#[derive(Debug, Serialize)]
pub struct SlowEntry {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub duration_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct PayloadEntry {
    pub id: String,
    pub host: String,
    pub norm_path: String,
    pub bytes: i64,
}

/// Compute the executive summary over the (filtered) capture. `top` bounds list sizes.
pub fn compute_summary(cap: &Capture, filter: &Filter, top: usize) -> SummaryResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut resource_breakdown: BTreeMap<String, usize> = BTreeMap::new();
    let mut status_classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut host_counts: AHashMap<String, usize> = AHashMap::new();
    let mut fp_counts: AHashMap<String, (usize, String)> = AHashMap::new();
    let mut error_count = 0usize;

    for e in &entries {
        let rt = format!("{:?}", e.resource_type).to_ascii_lowercase();
        *resource_breakdown.entry(rt).or_default() += 1;

        let class = match e.status_class() {
            2 => "2xx",
            3 => "3xx",
            4 => "4xx",
            5 => "5xx",
            _ => "other",
        };
        *status_classes.entry(class.to_string()).or_default() += 1;

        if e.is_error() {
            error_count += 1;
        }

        *host_counts.entry(e.host.clone()).or_default() += 1;

        let fp = fingerprint(e);
        let slot = fp_counts.entry(fp).or_insert((0, e.id.clone()));
        slot.0 += 1;
    }

    let top_hosts = top_n_map(&host_counts, top)
        .into_iter()
        .map(|(host, count)| HostCount { host, count })
        .collect();

    let mut dups: Vec<DuplicateGroup> = fp_counts
        .into_iter()
        .filter(|(_, (c, _))| *c > 1)
        .map(|(fp, (c, id))| DuplicateGroup { fingerprint: fp, count: c, example_id: id })
        .collect();
    dups.sort_by(|a, b| b.count.cmp(&a.count).then(a.fingerprint.cmp(&b.fingerprint)));
    dups.truncate(top);

    let mut slow: Vec<SlowEntry> = entries
        .iter()
        .map(|e| SlowEntry {
            id: e.id.clone(),
            method: e.method.clone(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            duration_ms: e.duration_ms,
        })
        .collect();
    slow.sort_by(|a, b| b.duration_ms.partial_cmp(&a.duration_ms).unwrap_or(std::cmp::Ordering::Equal));
    slow.truncate(top);

    let mut payloads: Vec<PayloadEntry> = entries
        .iter()
        .map(|e| PayloadEntry {
            id: e.id.clone(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            bytes: e.sizes.resp_content.max(e.sizes.resp_body),
        })
        .collect();
    payloads.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    payloads.truncate(top);

    let mut hints = Vec::new();
    if let Some(top_dup) = dups.first() {
        if top_dup.count >= 3 {
            hints.push(format!(
                "{}x duplicate calls: {}",
                top_dup.count, top_dup.fingerprint
            ));
        }
    }
    if error_count > 0 {
        hints.push(format!("{error_count} error responses (4xx/5xx/failed)"));
    }

    SummaryResult {
        total_entries: cap.entries.len(),
        filtered_entries: entries.len(),
        duration_ms: cap.meta.duration_ms,
        start_ms: cap.meta.start_ms,
        end_ms: cap.meta.end_ms,
        resource_breakdown,
        status_classes,
        error_count,
        top_hosts,
        top_duplicates: dups,
        slowest: slow,
        biggest_payloads: payloads,
        hints,
    }
}

fn top_n_map(map: &AHashMap<String, usize>, top: usize) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.iter().map(|(k, c)| (k.clone(), *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(top);
    v
}

use crate::render::{human_bytes, human_ms};

/// Render the summary as deterministic, copy-paste-safe terminal text.
pub fn render_summary_text(s: &SummaryResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail summary ==\n");
    out.push_str(&format!(
        "entries: {} total, {} after filter\n",
        s.total_entries, s.filtered_entries
    ));
    out.push_str(&format!("capture window: {}\n", human_ms(s.duration_ms)));

    out.push_str("\nstatus classes:\n");
    for (k, v) in &s.status_classes {
        out.push_str(&format!("  {k}: {v}\n"));
    }

    out.push_str("\nresource types:\n");
    for (k, v) in &s.resource_breakdown {
        out.push_str(&format!("  {k}: {v}\n"));
    }

    out.push_str("\ntop hosts (by request count):\n");
    for h in &s.top_hosts {
        out.push_str(&format!("  {:>5}  {}\n", h.count, h.host));
    }

    if !s.top_duplicates.is_empty() {
        out.push_str("\ntop duplicate calls:\n");
        for d in &s.top_duplicates {
            out.push_str(&format!("  {:>4}x  {}  ({})\n", d.count, d.fingerprint, d.example_id));
        }
    }

    out.push_str("\nslowest requests:\n");
    for e in &s.slowest {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}  [{}]\n",
            human_ms(e.duration_ms),
            e.id,
            e.method,
            e.host,
            e.norm_path,
            e.status
        ));
    }

    out.push_str("\nbiggest payloads:\n");
    for p in &s.biggest_payloads {
        out.push_str(&format!(
            "  {:>10}  {} {}{}\n",
            human_bytes(p.bytes),
            p.id,
            p.host,
            p.norm_path
        ));
    }

    if !s.hints.is_empty() {
        out.push_str("\nhints:\n");
        for h in &s.hints {
            out.push_str(&format!("  - {h}\n"));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::compute_summary;
    use crate::assemble::assemble;
    use crate::filter::Filter;
    use crate::loader::load;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn computes_summary_over_fixture() {
        let cap = assemble(load(&fixture("someapi123.har")).unwrap());
        let f = Filter::parse(&[]).unwrap();
        let s = compute_summary(&cap, &f, 5);
        assert_eq!(s.total_entries, cap.entries.len());
        assert_eq!(s.filtered_entries, cap.entries.len());
        // status_classes counts sum to filtered_entries
        let sum: usize = s.status_classes.values().sum();
        assert_eq!(sum, s.filtered_entries);
        // top_hosts has at most 5 entries
        assert!(s.top_hosts.len() <= 5);
    }

    #[test]
    fn filter_reduces_filtered_count() {
        let cap = assemble(load(&fixture("someapi123.har")).unwrap());
        let f = Filter::parse(&["status:>=400".into()]).unwrap();
        let s = compute_summary(&cap, &f, 5);
        assert!(s.filtered_entries <= s.total_entries);
    }
}
