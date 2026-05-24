use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::stats::percentiles;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub new_hosts: Vec<String>,
    pub removed_hosts: Vec<String>,
    pub new_endpoints: Vec<String>,
    pub removed_endpoints: Vec<String>,
    pub new_errors: Vec<EndpointDelta>,
    pub latency_regressions: Vec<LatencyDelta>,
    pub payload_growth: Vec<SizeDelta>,
    pub max_severity: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointDelta {
    pub endpoint: String,
    pub status: i64,
    pub count: usize,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct LatencyDelta {
    pub endpoint: String,
    pub base_p50_ms: f64,
    pub new_p50_ms: f64,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct SizeDelta {
    pub endpoint: String,
    pub base_bytes: i64,
    pub new_bytes: i64,
    pub severity: String,
}

/// Severity ordering shared with the CLI `--fail-on` gate.
pub fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

#[derive(Default)]
struct Agg {
    durations: Vec<f64>,
    bytes: Vec<f64>,
    error_statuses: Vec<i64>,
}

fn endpoint_key(e: &Entry) -> String {
    format!("{} {}{}", e.method.to_ascii_uppercase(), e.host, e.norm_path)
}

fn aggregate(cap: &Capture, filter: &Filter) -> (AHashSet<String>, AHashMap<String, Agg>) {
    let mut hosts = AHashSet::new();
    let mut map: AHashMap<String, Agg> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        hosts.insert(e.host.clone());
        let a = map.entry(endpoint_key(e)).or_default();
        a.durations.push(e.duration_ms);
        a.bytes
            .push(e.sizes.resp_content.max(e.sizes.resp_body).max(0) as f64);
        let cls = e.status_class();
        if cls == 4 || cls == 5 {
            a.error_statuses.push(e.status);
        }
    }
    (hosts, map)
}

/// Diff a new capture against a baseline; severity-score the regressions.
pub fn compute_compare(new: &Capture, base: &Capture, filter: &Filter, top: usize) -> CompareResult {
    let (new_hosts_set, new_map) = aggregate(new, filter);
    let (base_hosts_set, base_map) = aggregate(base, filter);

    let mut new_hosts: Vec<String> = new_hosts_set.difference(&base_hosts_set).cloned().collect();
    let mut removed_hosts: Vec<String> =
        base_hosts_set.difference(&new_hosts_set).cloned().collect();

    let new_keys: AHashSet<&String> = new_map.keys().collect();
    let base_keys: AHashSet<&String> = base_map.keys().collect();
    let mut new_endpoints: Vec<String> = new_keys
        .difference(&base_keys)
        .map(|s| (*s).clone())
        .collect();
    let mut removed_endpoints: Vec<String> = base_keys
        .difference(&new_keys)
        .map(|s| (*s).clone())
        .collect();

    let mut new_errors = Vec::new();
    let mut latency_regressions = Vec::new();
    let mut payload_growth = Vec::new();

    for (ep, a) in &new_map {
        // new errors: 4xx/5xx present in new but not in baseline for this endpoint
        if !a.error_statuses.is_empty() {
            let base_had = base_map
                .get(ep)
                .map(|b| !b.error_statuses.is_empty())
                .unwrap_or(false);
            if !base_had {
                let worst = *a.error_statuses.iter().max().unwrap();
                let severity = if worst / 100 == 5 { "high" } else { "medium" };
                new_errors.push(EndpointDelta {
                    endpoint: ep.clone(),
                    status: worst,
                    count: a.error_statuses.len(),
                    severity: severity.into(),
                });
            }
        }

        if let Some(b) = base_map.get(ep) {
            let np = percentiles(&a.durations).p50;
            let bp = percentiles(&b.durations).p50;
            if bp > 0.0 && np > bp * 2.0 && (np - bp) > 200.0 {
                latency_regressions.push(LatencyDelta {
                    endpoint: ep.clone(),
                    base_p50_ms: bp,
                    new_p50_ms: np,
                    severity: "medium".into(),
                });
            }

            let nb = percentiles(&a.bytes).p50;
            let bb = percentiles(&b.bytes).p50;
            if bb > 0.0 && nb > bb * 2.0 {
                payload_growth.push(SizeDelta {
                    endpoint: ep.clone(),
                    base_bytes: bb as i64,
                    new_bytes: nb as i64,
                    severity: "low".into(),
                });
            }
        }
    }

    new_hosts.sort();
    removed_hosts.sort();
    new_endpoints.sort();
    removed_endpoints.sort();
    new_errors.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.count.cmp(&a.count))
            .then(a.endpoint.cmp(&b.endpoint))
    });
    latency_regressions.sort_by(|a, b| {
        (b.new_p50_ms - b.base_p50_ms)
            .partial_cmp(&(a.new_p50_ms - a.base_p50_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.endpoint.cmp(&b.endpoint))
    });
    payload_growth.sort_by(|a, b| {
        (b.new_bytes - b.base_bytes)
            .cmp(&(a.new_bytes - a.base_bytes))
            .then(a.endpoint.cmp(&b.endpoint))
    });

    new_hosts.truncate(top);
    removed_hosts.truncate(top);
    new_endpoints.truncate(top);
    removed_endpoints.truncate(top);
    new_errors.truncate(top);
    latency_regressions.truncate(top);
    payload_growth.truncate(top);

    let mut rank = 0u8;
    for s in new_errors
        .iter()
        .map(|d| d.severity.as_str())
        .chain(latency_regressions.iter().map(|d| d.severity.as_str()))
        .chain(payload_growth.iter().map(|d| d.severity.as_str()))
    {
        rank = rank.max(sev_rank(s));
    }
    let any =
        !new_errors.is_empty() || !latency_regressions.is_empty() || !payload_growth.is_empty();
    let max_severity = match rank {
        3 => "critical",
        2 => "high",
        1 => "medium",
        _ if any => "low",
        _ => "none",
    }
    .to_string();

    CompareResult {
        new_hosts,
        removed_hosts,
        new_endpoints,
        removed_endpoints,
        new_errors,
        latency_regressions,
        payload_growth,
        max_severity,
    }
}

/// Render the comparison as deterministic terminal text.
pub fn render_compare_text(r: &CompareResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail compare ==\n");
    out.push_str(&format!("max severity: {}\n", r.max_severity));
    if !r.new_hosts.is_empty() {
        out.push_str(&format!("new hosts: {}\n", r.new_hosts.join(", ")));
    }
    if !r.removed_hosts.is_empty() {
        out.push_str(&format!("removed hosts: {}\n", r.removed_hosts.join(", ")));
    }
    if !r.new_endpoints.is_empty() {
        out.push_str(&format!("new endpoints: {}\n", r.new_endpoints.len()));
    }
    if !r.removed_endpoints.is_empty() {
        out.push_str(&format!(
            "removed endpoints: {}\n",
            r.removed_endpoints.len()
        ));
    }
    if !r.new_errors.is_empty() {
        out.push_str("\nnew errors:\n");
        for d in &r.new_errors {
            out.push_str(&format!(
                "  [{}] {} -> {} ({}x)\n",
                d.severity, d.endpoint, d.status, d.count
            ));
        }
    }
    if !r.latency_regressions.is_empty() {
        out.push_str("\nlatency regressions:\n");
        for d in &r.latency_regressions {
            out.push_str(&format!(
                "  [{}] {} p50 {:.0}ms -> {:.0}ms\n",
                d.severity, d.endpoint, d.base_p50_ms, d.new_p50_ms
            ));
        }
    }
    if !r.payload_growth.is_empty() {
        out.push_str("\npayload growth:\n");
        for d in &r.payload_growth {
            out.push_str(&format!(
                "  [{}] {} {}B -> {}B\n",
                d.severity, d.endpoint, d.base_bytes, d.new_bytes
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_compare;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn no_filter() -> Filter {
        Filter::parse(&[]).unwrap()
    }

    #[test]
    fn detects_new_host() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.y", "GET", "/b", 200),
        ]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert!(r.new_hosts.contains(&"api.y".to_string()));
        assert!(!r.new_hosts.contains(&"api.x".to_string()));
    }

    #[test]
    fn detects_new_5xx_as_high() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 500)]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert!(
            r.new_errors
                .iter()
                .any(|d| d.status == 500 && d.severity == "high")
        );
        assert_eq!(r.max_severity, "high");
    }

    #[test]
    fn detects_latency_regression() {
        let mut b = sample_entry(0, "api.x", "GET", "/a", 200);
        b.duration_ms = 100.0;
        let mut n = sample_entry(0, "api.x", "GET", "/a", 200);
        n.duration_ms = 900.0; // > 2x and > 200ms over baseline
        let r = compute_compare(
            &sample_capture(vec![n]),
            &sample_capture(vec![b]),
            &no_filter(),
            50,
        );
        assert_eq!(r.latency_regressions.len(), 1);
        assert_eq!(r.latency_regressions[0].severity, "medium");
    }

    #[test]
    fn detects_payload_growth() {
        let mut b: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        b.sizes.resp_content = 100;
        let mut n: Entry = sample_entry(0, "api.x", "GET", "/a", 200);
        n.sizes.resp_content = 500; // > 2x
        let r = compute_compare(
            &sample_capture(vec![n]),
            &sample_capture(vec![b]),
            &no_filter(),
            50,
        );
        assert_eq!(r.payload_growth.len(), 1);
        assert_eq!(r.payload_growth[0].severity, "low");
    }

    #[test]
    fn no_findings_is_none() {
        let base = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let new = sample_capture(vec![sample_entry(0, "api.x", "GET", "/a", 200)]);
        let r = compute_compare(&new, &base, &no_filter(), 50);
        assert_eq!(r.max_severity, "none");
    }
}
