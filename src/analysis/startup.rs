use crate::filter::Filter;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StartupResult {
    pub window_ms: f64,
    pub requests_in_window: usize,
    pub max_concurrency: usize,
    pub critical_path_ms: f64,
    pub critical_path: Vec<StartupCall>,
    pub slowest: Vec<StartupCall>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupCall {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub status: i64,
}

fn call_of(e: &Entry) -> StartupCall {
    StartupCall {
        id: e.id.clone(),
        method: e.method.to_ascii_uppercase(),
        host: e.host.clone(),
        norm_path: e.norm_path.clone(),
        offset_ms: e.started_offset_ms,
        duration_ms: e.duration_ms,
        status: e.status,
    }
}

/// Profile the boot window: concurrency, sequential critical path, slow deps.
/// `window_ms == 0` means "the whole capture".
pub fn compute_startup(
    cap: &Capture,
    filter: &Filter,
    window_ms: u64,
    top: usize,
) -> StartupResult {
    let mut entries: Vec<&Entry> = cap
        .entries
        .iter()
        .filter(|e| {
            filter.matches(e) && (window_ms == 0 || e.started_offset_ms <= window_ms as f64)
        })
        .collect();
    entries.sort_by(|a, b| {
        a.started_offset_ms
            .partial_cmp(&b.started_offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });

    // max concurrency via a sweep line over start/end events.
    let mut events: Vec<(f64, i32)> = Vec::with_capacity(entries.len() * 2);
    for e in &entries {
        events.push((e.started_offset_ms, 1));
        events.push((e.started_offset_ms + e.duration_ms.max(0.0), -1));
    }
    // ends before starts at the same instant, so a touch-point isn't double-counted.
    events.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    let mut cur = 0i32;
    let mut max_concurrency = 0i32;
    for (_, d) in &events {
        cur += d;
        max_concurrency = max_concurrency.max(cur);
    }

    // greedy sequential chain: each next call starts at/after the current one ends.
    let mut chain: Vec<StartupCall> = Vec::new();
    let mut chain_ms = 0.0;
    let mut end = f64::MIN;
    for e in &entries {
        if e.started_offset_ms >= end {
            chain.push(call_of(e));
            chain_ms += e.duration_ms.max(0.0);
            end = e.started_offset_ms + e.duration_ms.max(0.0);
        }
    }
    let critical_path: Vec<StartupCall> = chain.iter().take(top).cloned().collect();

    let mut slow: Vec<&Entry> = entries.clone();
    slow.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let slowest: Vec<StartupCall> = slow.iter().take(top).map(|e| call_of(e)).collect();

    let window_ms_out = if window_ms == 0 {
        entries.last().map(|e| e.started_offset_ms).unwrap_or(0.0)
    } else {
        window_ms as f64
    };

    StartupResult {
        window_ms: window_ms_out,
        requests_in_window: entries.len(),
        max_concurrency: max_concurrency.max(0) as usize,
        critical_path_ms: chain_ms,
        critical_path,
        slowest,
    }
}

/// Render the startup profile as deterministic terminal text.
pub fn render_startup_text(r: &StartupResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail startup ==\n");
    out.push_str(&format!(
        "{} requests in {} · max concurrency {} · critical path {}\n",
        r.requests_in_window,
        human_ms(r.window_ms),
        r.max_concurrency,
        human_ms(r.critical_path_ms)
    ));
    out.push_str("\ncritical path (sequential spine):\n");
    for c in &r.critical_path {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}  [{}]\n",
            human_ms(c.duration_ms),
            c.id,
            c.method,
            c.host,
            c.norm_path,
            c.status
        ));
    }
    out.push_str("\nslowest in window:\n");
    for c in &r.slowest {
        out.push_str(&format!(
            "  {:>8}  {} {} {}{}\n",
            human_ms(c.duration_ms),
            c.id,
            c.method,
            c.host,
            c.norm_path
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_startup;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn at(index: usize, path: &str, offset: f64, dur: f64) -> Entry {
        let mut e = sample_entry(index, "h", "GET", path, 200);
        e.started_offset_ms = offset;
        e.duration_ms = dur;
        e
    }

    #[test]
    fn measures_max_concurrency() {
        let cap = sample_capture(vec![
            at(0, "/a", 0.0, 200.0),
            at(1, "/b", 100.0, 100.0),
            at(2, "/c", 120.0, 50.0),
        ]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 0, 10);
        assert_eq!(r.max_concurrency, 3);
        assert_eq!(r.requests_in_window, 3);
    }

    #[test]
    fn builds_sequential_critical_path() {
        let cap = sample_capture(vec![
            at(0, "/a", 0.0, 100.0),
            at(1, "/b", 100.0, 100.0),
            at(2, "/c", 200.0, 100.0),
        ]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 0, 10);
        assert_eq!(r.critical_path.len(), 3);
        assert_eq!(r.critical_path_ms, 300.0);
        assert_eq!(r.max_concurrency, 1);
    }

    #[test]
    fn window_bounds_entries() {
        let cap = sample_capture(vec![at(0, "/a", 0.0, 10.0), at(1, "/late", 60000.0, 10.0)]);
        let r = compute_startup(&cap, &Filter::parse(&[]).unwrap(), 30000, 10);
        assert_eq!(r.requests_in_window, 1);
    }
}
