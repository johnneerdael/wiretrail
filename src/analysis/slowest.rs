use crate::filter::Filter;
use crate::model::Capture;
use crate::render::human_ms;
use crate::timing::{PhaseBreakdown, classify_bottleneck};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SlowestResult {
    pub entries: Vec<SlowRow>,
}

#[derive(Debug, Serialize)]
pub struct SlowRow {
    pub id: String,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub duration_ms: f64,
    pub phases: PhaseBreakdown,
    pub bottleneck: String,
}

/// Top-N slowest requests globally, with timing breakdown and bottleneck label.
pub fn compute_slowest(cap: &Capture, filter: &Filter, top: usize) -> SlowestResult {
    let mut entries: Vec<SlowRow> = cap
        .entries
        .iter()
        .filter(|e| filter.matches(e))
        .map(|e| SlowRow {
            id: e.id.clone(),
            method: e.method.to_ascii_uppercase(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            duration_ms: e.duration_ms,
            phases: PhaseBreakdown::from_phases(&e.timings),
            bottleneck: classify_bottleneck(&e.timings).to_string(),
        })
        .collect();

    entries.sort_by(|a, b| {
        b.duration_ms
            .partial_cmp(&a.duration_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    entries.truncate(top);
    SlowestResult { entries }
}

/// Render slowest requests as deterministic terminal text.
pub fn render_slowest_text(r: &SlowestResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail slowest ==\n");
    for e in &r.entries {
        out.push_str(&format!(
            "\n{:>8}  {} {} {}{}  [{}]\n",
            human_ms(e.duration_ms),
            e.id,
            e.method,
            e.host,
            e.norm_path,
            e.status
        ));
        out.push_str(&format!("  bottleneck: {}\n", e.bottleneck));
        out.push_str(&format!(
            "  phases: wait {} / receive {} / send {} / connect {} / dns {} / ssl {} / blocked {}\n",
            human_ms(e.phases.wait),
            human_ms(e.phases.receive),
            human_ms(e.phases.send),
            human_ms(e.phases.connect.unwrap_or(0.0)),
            human_ms(e.phases.dns.unwrap_or(0.0)),
            human_ms(e.phases.ssl.unwrap_or(0.0)),
            human_ms(e.phases.blocked.unwrap_or(0.0)),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_slowest;
    use crate::filter::Filter;
    use crate::model::{Phases, sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut fast = sample_entry(0, "h", "GET", "/fast", 200);
        fast.duration_ms = 5.0;
        let mut slow = sample_entry(1, "h", "GET", "/slow", 200);
        slow.duration_ms = 900.0;
        slow.timings = Phases {
            wait: 850.0,
            receive: 40.0,
            ..Phases::default()
        };
        sample_capture(vec![fast, slow])
    }

    #[test]
    fn orders_by_duration_desc_with_bottleneck() {
        let r = compute_slowest(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.entries[0].norm_path, "/slow");
        assert_eq!(r.entries[0].duration_ms, 900.0);
        assert_eq!(r.entries[0].bottleneck, "server wait/TTFB");
    }

    #[test]
    fn top_bounds_list() {
        let r = compute_slowest(&cap(), &Filter::parse(&[]).unwrap(), 1);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].norm_path, "/slow");
    }
}
