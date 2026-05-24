use crate::filter::Filter;
use crate::model::Capture;
use crate::recommender::{Recommendation, recommend};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DiagnoseResult {
    pub findings: Vec<Diagnosis>,
}

#[derive(Debug, Serialize)]
pub struct Diagnosis {
    pub severity: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub suggested_command: String,
}

/// Synthesize ranked root-cause findings (renders over the shared recommender).
pub fn compute_diagnose(cap: &Capture, filter: &Filter, top: usize) -> DiagnoseResult {
    let findings = recommend(cap, filter, top)
        .into_iter()
        .map(|r: Recommendation| {
            let suggested_command = r.command_line();
            Diagnosis {
                severity: r.severity,
                kind: r.kind,
                title: r.title,
                detail: r.detail,
                evidence_ids: r.evidence_ids,
                suggested_command,
            }
        })
        .collect();
    DiagnoseResult { findings }
}

/// Render the diagnosis as deterministic terminal text.
pub fn render_diagnose_text(r: &DiagnoseResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail diagnose ==\n");
    for d in &r.findings {
        out.push_str(&format!(
            "\n[{}] {} — {}\n",
            d.severity.to_ascii_uppercase(),
            d.kind,
            d.title
        ));
        out.push_str(&format!("  {}\n", d.detail));
        out.push_str(&format!(
            "  evidence: {}\n",
            d.evidence_ids
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!("  -> wiretrail <file> {}\n", d.suggested_command));
    }
    if r.findings.is_empty() {
        out.push_str("\nno notable findings\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_diagnose;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn err(index: usize, path: &str, status: i64, off: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", path, status);
        e.started_offset_ms = off;
        e
    }

    #[test]
    fn surfaces_5xx_cluster_as_high() {
        let cap = sample_capture(vec![
            err(0, "/bulk", 500, 0.0),
            err(1, "/bulk", 500, 10.0),
            err(2, "/bulk", 500, 20.0),
        ]);
        let r = compute_diagnose(&cap, &Filter::parse(&[]).unwrap(), 20);
        assert!(
            r.findings
                .iter()
                .any(|f| f.kind == "5xx-cluster" && f.severity == "high")
        );
        assert_eq!(r.findings[0].severity, "high");
    }

    #[test]
    fn clean_capture_has_no_findings() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/ok", 200)]);
        let r = compute_diagnose(&cap, &Filter::parse(&[]).unwrap(), 20);
        assert!(r.findings.is_empty());
    }
}
