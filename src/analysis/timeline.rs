use crate::filter::Filter;
use crate::fingerprint::fingerprint;
use crate::grouping::retry_entry_ids;
use crate::model::Capture;
use crate::render::{human_bytes, human_ms};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TimelineResult {
    pub rows: Vec<TimelineRow>,
}

#[derive(Debug, Serialize)]
pub struct TimelineRow {
    pub id: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub bytes: i64,
    pub correlation_id: Option<String>,
    pub marker: Option<String>,
}

/// Chronological per-request timeline. `top` bounds the number of rows (earliest first).
pub fn compute_timeline(cap: &Capture, filter: &Filter, top: usize) -> TimelineResult {
    let entries: Vec<&crate::model::Entry> =
        cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let retries = retry_entry_ids(&entries);
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    for e in &entries {
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }

    let mut rows: Vec<TimelineRow> = entries
        .iter()
        .map(|e| {
            let is_dup = fp_counts.get(&fingerprint(e)).copied().unwrap_or(0) > 1;
            let marker = if retries.contains(&e.id) {
                Some("RETRY".to_string())
            } else if is_dup {
                Some("DUP".to_string())
            } else {
                None
            };
            TimelineRow {
                id: e.id.clone(),
                offset_ms: e.started_offset_ms,
                duration_ms: e.duration_ms,
                method: e.method.to_ascii_uppercase(),
                host: e.host.clone(),
                norm_path: e.norm_path.clone(),
                status: e.status,
                bytes: e.sizes.resp_content.max(e.sizes.resp_body).max(0),
                correlation_id: e.correlation.first().map(|(_, v)| v.clone()),
                marker,
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        a.offset_ms
            .partial_cmp(&b.offset_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(&b.id))
    });
    rows.truncate(top);
    TimelineResult { rows }
}

/// Render the timeline as deterministic terminal text.
pub fn render_timeline_text(r: &TimelineResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail timeline ==\n");
    for row in &r.rows {
        let marker = row
            .marker
            .as_deref()
            .map(|m| format!(" {m}"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{:>8}  {:>7}  {} {} {}{}  [{}] {}{}\n",
            human_ms(row.offset_ms),
            human_ms(row.duration_ms),
            row.id,
            row.method,
            row.host,
            row.norm_path,
            row.status,
            human_bytes(row.bytes),
            marker,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_timeline;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let mut entries = vec![
            sample_entry(0, "h", "POST", "/x", 500), // retry trigger
            sample_entry(1, "h", "POST", "/x", 200), // retry
            sample_entry(2, "h", "GET", "/y", 200),  // unique
        ];
        entries[1].started_offset_ms = 50.0;
        entries[2].started_offset_ms = 20.0;
        sample_capture(entries)
    }

    #[test]
    fn ordered_by_offset_with_markers() {
        let r = compute_timeline(&cap(), &Filter::parse(&[]).unwrap(), 100);
        // offsets: e0=0, e2=20, e1=50 -> chronological order
        assert_eq!(r.rows[0].id, "e000000");
        assert_eq!(r.rows[1].id, "e000002");
        assert_eq!(r.rows[2].id, "e000001");
        // e0 and e1 are duplicates; e1 follows a 500 -> RETRY
        assert_eq!(r.rows[2].marker.as_deref(), Some("RETRY"));
        assert_eq!(r.rows[0].marker.as_deref(), Some("DUP"));
        assert!(r.rows[1].marker.is_none());
    }
}
