use crate::filter::Filter;
use crate::fingerprint::fingerprint;
use crate::model::{Capture, Entry};
use crate::render::{human_bytes, human_ms};
use crate::stats::percentiles;
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct HostsResult {
    pub hosts: Vec<HostStat>,
}

#[derive(Debug, Serialize)]
pub struct HostStat {
    pub host: String,
    pub count: usize,
    pub methods: BTreeMap<String, usize>,
    pub status_classes: BTreeMap<String, usize>,
    pub error_count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub bytes_sent: i64,
    pub bytes_received: i64,
    pub duplicate_count: usize,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

/// Aggregate the filtered capture per host. `top` bounds the returned list.
pub fn compute_hosts(cap: &Capture, filter: &Filter, top: usize) -> HostsResult {
    let mut by_host: AHashMap<String, Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        by_host.entry(e.host.clone()).or_default().push(e);
    }

    let mut hosts: Vec<HostStat> = by_host
        .into_iter()
        .map(|(host, entries)| host_stat(host, &entries))
        .collect();

    hosts.sort_by(|a, b| b.count.cmp(&a.count).then(a.host.cmp(&b.host)));
    hosts.truncate(top);
    HostsResult { hosts }
}

fn host_stat(host: String, entries: &[&Entry]) -> HostStat {
    let mut methods: BTreeMap<String, usize> = BTreeMap::new();
    let mut status_classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    let mut durations: Vec<f64> = Vec::with_capacity(entries.len());
    let mut error_count = 0usize;
    let mut bytes_sent = 0i64;
    let mut bytes_received = 0i64;
    let mut first = f64::MAX;
    let mut last = f64::MIN;

    for e in entries {
        *methods.entry(e.method.to_ascii_uppercase()).or_default() += 1;
        *status_classes
            .entry(status_class_label(e.status_class()))
            .or_default() += 1;
        if e.is_error() {
            error_count += 1;
        }
        durations.push(e.duration_ms);
        bytes_sent += e.sizes.req_body.max(0);
        bytes_received += e.sizes.resp_content.max(e.sizes.resp_body).max(0);
        first = first.min(e.started_offset_ms);
        last = last.max(e.started_offset_ms);
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }

    let duplicate_count: usize = fp_counts.values().filter(|c| **c > 1).sum();
    let p = percentiles(&durations);

    HostStat {
        host,
        count: entries.len(),
        methods,
        status_classes,
        error_count,
        p50_ms: p.p50,
        p95_ms: p.p95,
        max_ms: p.max,
        bytes_sent,
        bytes_received,
        duplicate_count,
        first_offset_ms: if first == f64::MAX { 0.0 } else { first },
        last_offset_ms: if last == f64::MIN { 0.0 } else { last },
    }
}

fn status_class_label(class: i64) -> String {
    match class {
        2 => "2xx",
        3 => "3xx",
        4 => "4xx",
        5 => "5xx",
        _ => "other",
    }
    .to_string()
}

/// Render hosts as deterministic terminal text.
pub fn render_hosts_text(r: &HostsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail hosts ==\n");
    for h in &r.hosts {
        out.push_str(&format!(
            "\n{}  ({} req, {} err, {} dup)\n",
            h.host, h.count, h.error_count, h.duplicate_count
        ));
        out.push_str(&format!(
            "  latency p50/p95/max: {} / {} / {}\n",
            human_ms(h.p50_ms),
            human_ms(h.p95_ms),
            human_ms(h.max_ms)
        ));
        out.push_str(&format!(
            "  bytes sent/received: {} / {}\n",
            human_bytes(h.bytes_sent),
            human_bytes(h.bytes_received)
        ));
        let methods: Vec<String> = h.methods.iter().map(|(m, c)| format!("{m}:{c}")).collect();
        out.push_str(&format!("  methods: {}\n", methods.join(" ")));
        let statuses: Vec<String> = h
            .status_classes
            .iter()
            .map(|(s, c)| format!("{s}:{c}"))
            .collect();
        out.push_str(&format!("  status: {}\n", statuses.join(" ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_hosts;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "api.foo.com", "GET", "/v1/a", 200),
            sample_entry(1, "api.foo.com", "GET", "/v1/a", 200), // duplicate of e0
            sample_entry(2, "api.foo.com", "POST", "/v1/b", 500),
            sample_entry(3, "cdn.bar.com", "GET", "/img", 200),
        ];
        entries[2].duration_ms = 100.0;
        sample_capture(entries)
    }

    #[test]
    fn groups_by_host_with_counts_and_errors() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let foo = r.hosts.iter().find(|h| h.host == "api.foo.com").unwrap();
        assert_eq!(foo.count, 3);
        assert_eq!(foo.error_count, 1);
        assert_eq!(foo.methods.get("GET"), Some(&2));
        assert_eq!(foo.methods.get("POST"), Some(&1));
        // e0 and e1 are identical -> 2 duplicate members
        assert_eq!(foo.duplicate_count, 2);
        assert_eq!(foo.max_ms, 100.0);
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.hosts[0].host, "api.foo.com"); // 3 > 1
    }

    #[test]
    fn top_bounds_list() {
        let r = compute_hosts(&cap(), &Filter::parse(&[]).unwrap(), 1);
        assert_eq!(r.hosts.len(), 1);
    }
}
