use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ExportRecord {
    pub id: String,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub method: String,
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub bytes: i64,
    pub content_type: Option<String>,
    pub resource_type: String,
    pub correlation: Option<String>,
}

/// Flatten the filtered capture into one normalized record per entry (redacted
/// by construction — metadata only, no raw bodies/headers).
pub fn export_records(cap: &Capture, filter: &Filter) -> Vec<ExportRecord> {
    cap.entries
        .iter()
        .filter(|e| filter.matches(e))
        .map(|e| ExportRecord {
            id: e.id.clone(),
            offset_ms: e.started_offset_ms,
            duration_ms: e.duration_ms,
            method: e.method.to_ascii_uppercase(),
            host: e.host.clone(),
            norm_path: e.norm_path.clone(),
            status: e.status,
            bytes: e.sizes.resp_content.max(e.sizes.resp_body).max(0),
            content_type: e.content_type.clone(),
            resource_type: format!("{:?}", e.resource_type).to_ascii_lowercase(),
            correlation: e.correlation.first().map(|(_, v)| v.clone()),
        })
        .collect()
}

/// One JSON object per line.
pub fn render_ndjson(records: &[ExportRecord]) -> String {
    records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// RFC4180-ish CSV: header + one row per record.
pub fn render_csv(records: &[ExportRecord]) -> String {
    let mut out = String::new();
    out.push_str(
        "id,offset_ms,duration_ms,method,host,norm_path,status,bytes,content_type,resource_type,correlation\n",
    );
    for r in records {
        let row = [
            r.id.clone(),
            (r.offset_ms as i64).to_string(),
            (r.duration_ms as i64).to_string(),
            r.method.clone(),
            r.host.clone(),
            r.norm_path.clone(),
            r.status.to_string(),
            r.bytes.to_string(),
            r.content_type.clone().unwrap_or_default(),
            r.resource_type.clone(),
            r.correlation.clone().unwrap_or_default(),
        ];
        out.push_str(
            &row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{export_records, render_csv, render_ndjson};
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "api.x", "GET", "/a", 200),
            sample_entry(1, "api.x", "POST", "/b", 500),
        ])
    }

    #[test]
    fn ndjson_one_line_per_entry() {
        let recs = export_records(&cap(), &Filter::parse(&[]).unwrap());
        let s = render_ndjson(&recs);
        assert_eq!(s.lines().count(), 2);
        assert!(s.lines().all(|l| l.starts_with('{') && l.contains("\"id\"")));
    }

    #[test]
    fn csv_has_header_and_rows() {
        let recs = export_records(&cap(), &Filter::parse(&[]).unwrap());
        let s = render_csv(&recs);
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[0].starts_with("id,offset_ms,"));
        assert_eq!(lines.len(), 3); // header + 2 rows
    }

    #[test]
    fn csv_quotes_fields_with_commas() {
        let mut e = sample_entry(0, "api.x", "GET", "/a,b", 200);
        e.content_type = Some("text/html; charset=utf-8".into());
        let recs = export_records(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap());
        let s = render_csv(&recs);
        assert!(s.contains("\"/a,b\""));
    }
}
