use crate::filter::Filter;
use crate::grouping::densest_window;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StormsResult {
    pub storms: Vec<Storm>,
}

#[derive(Debug, Serialize)]
pub struct Storm {
    pub scope_kind: String, // "host" | "endpoint"
    pub scope: String,
    pub peak_count: usize,
    pub window_ms: u64,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub calls_per_sec: f64,
    pub entry_ids: Vec<String>,
}

/// Detect bursts of many calls to the same host or endpoint within `window_ms`.
pub fn compute_storms(
    cap: &Capture,
    filter: &Filter,
    window_ms: u64,
    min_count: usize,
    top: usize,
) -> StormsResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_host: AHashMap<String, Vec<&Entry>> = AHashMap::new();
    let mut by_endpoint: AHashMap<(String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_host.entry(e.host.clone()).or_default().push(e);
        by_endpoint
            .entry((e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut storms = Vec::new();
    for (host, mut g) in by_host {
        sort_by_offset(&mut g);
        if let Some(s) = storm_for("host", host, &g, window_ms, min_count) {
            storms.push(s);
        }
    }
    for ((host, np), mut g) in by_endpoint {
        sort_by_offset(&mut g);
        if let Some(s) = storm_for("endpoint", format!("{host}{np}"), &g, window_ms, min_count) {
            storms.push(s);
        }
    }

    storms.sort_by(|a, b| {
        b.peak_count
            .cmp(&a.peak_count)
            .then(a.scope.cmp(&b.scope))
            .then(a.scope_kind.cmp(&b.scope_kind))
    });
    storms.truncate(top);
    StormsResult { storms }
}

fn sort_by_offset(g: &mut [&Entry]) {
    g.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });
}

fn storm_for(
    kind: &str,
    scope: String,
    g: &[&Entry],
    window_ms: u64,
    min_count: usize,
) -> Option<Storm> {
    let (count, l, r) = densest_window(g, window_ms as f64);
    if count < min_count {
        return None;
    }
    let win = &g[l..=r];
    Some(Storm {
        scope_kind: kind.to_string(),
        scope,
        peak_count: count,
        window_ms,
        first_offset_ms: win.first().unwrap().started_offset_ms,
        last_offset_ms: win.last().unwrap().started_offset_ms,
        calls_per_sec: count as f64 * 1000.0 / window_ms as f64,
        entry_ids: win.iter().map(|e| e.id.clone()).collect(),
    })
}

/// Render storms as deterministic terminal text.
pub fn render_storms_text(r: &StormsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail storms ==\n");
    for s in &r.storms {
        out.push_str(&format!(
            "\n{} {}  {} calls in {} ({:.1}/s)\n",
            s.scope_kind,
            s.scope,
            s.peak_count,
            human_ms(s.window_ms as f64),
            s.calls_per_sec
        ));
        out.push_str(&format!(
            "  window: {} - {}\n",
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms)
        ));
        out.push_str(&format!("  entries: {}\n", s.entry_ids.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_storms;
    use crate::filter::Filter;
    use crate::model::{Capture, Entry, sample_capture, sample_entry};

    fn at(index: usize, host: &str, path: &str, offset_ms: f64) -> Entry {
        let mut e = sample_entry(index, host, "GET", path, 200);
        e.started_offset_ms = offset_ms;
        e
    }

    fn burst() -> Capture {
        // 6 calls to same endpoint within 250ms
        let mut es = Vec::new();
        for i in 0..6 {
            es.push(at(i, "h", "/x", i as f64 * 50.0));
        }
        sample_capture(es)
    }

    #[test]
    fn detects_endpoint_and_host_burst() {
        let r = compute_storms(&burst(), &Filter::parse(&[]).unwrap(), 1000, 5, 10);
        assert!(
            r.storms
                .iter()
                .any(|s| s.scope_kind == "endpoint" && s.peak_count == 6)
        );
        assert!(
            r.storms
                .iter()
                .any(|s| s.scope_kind == "host" && s.peak_count == 6)
        );
    }

    #[test]
    fn no_storm_when_spread_out() {
        let mut es = Vec::new();
        for i in 0..6 {
            es.push(at(i, "h", "/x", i as f64 * 1000.0)); // 1s apart
        }
        let r = compute_storms(
            &sample_capture(es),
            &Filter::parse(&[]).unwrap(),
            500,
            5,
            10,
        );
        assert!(r.storms.is_empty());
    }

    #[test]
    fn min_count_gates() {
        let r = compute_storms(&burst(), &Filter::parse(&[]).unwrap(), 1000, 7, 10);
        assert!(r.storms.is_empty()); // only 6 calls, need 7
    }
}
