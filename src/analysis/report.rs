use crate::analysis::duplicates::compute_duplicates;
use crate::analysis::errors::compute_errors;
use crate::analysis::redirects::compute_redirects;
use crate::analysis::slowest::compute_slowest;
use crate::analysis::subsystems::compute_subsystems;
use crate::analysis::summary::compute_summary;
use crate::config::Config;
use crate::filter::Filter;
use crate::model::Capture;
use crate::render::human_ms;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReportResult {
    pub markdown: String,
}

/// Compose a dossier-style markdown report from the existing analyses.
pub fn compose_report(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    top: usize,
    unsafe_include: bool,
) -> String {
    let mut md = String::new();

    md.push_str("# wiretrail report\n\n");
    md.push_str(&format!("- Creator: {} {}\n", cap.meta.creator, cap.meta.creator_version));
    md.push_str(&format!("- HAR version: {}\n", cap.meta.har_version));
    md.push_str(&format!("- Entries: {}\n", cap.meta.entry_count));
    md.push_str(&format!("- Window: {}\n\n", human_ms(cap.meta.duration_ms)));

    let summary = compute_summary(cap, filter, top);
    md.push_str("## Executive Summary\n\n");
    md.push_str(&format!(
        "{} requests after filter, {} error responses, {} duplicate groups in the top list.\n\n",
        summary.filtered_entries,
        summary.error_count,
        summary.top_duplicates.len()
    ));

    let subs = compute_subsystems(cap, filter, config, top);
    md.push_str("## Subsystems\n\n");
    md.push_str("| Subsystem | Requests | Window | Errors | Dups |\n");
    md.push_str("|---|---:|---|---:|---:|\n");
    for s in &subs.subsystems {
        md.push_str(&format!(
            "| {} | {} | {} - {} | {} | {} |\n",
            s.name,
            s.count,
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms),
            s.error_count,
            s.duplicate_count
        ));
    }
    md.push('\n');

    let dups = compute_duplicates(cap, filter, top);
    if !dups.groups.is_empty() {
        md.push_str("## Duplicate Index\n\n");
        for g in &dups.groups {
            let tag = if g.is_retry_pattern { " (retry pattern)" } else { "" };
            md.push_str(&format!(
                "- {}x `{} {}{}`{}\n",
                g.count, g.method, g.host, g.norm_path, tag
            ));
        }
        md.push('\n');
    }

    let errs = compute_errors(cap, filter, top, unsafe_include);
    if !errs.groups.is_empty() {
        md.push_str("## Errors\n\n");
        for g in &errs.groups {
            md.push_str(&format!(
                "- {}x [{}] `{} {}{}`",
                g.count, g.status, g.method, g.host, g.norm_path
            ));
            if let Some(m) = &g.error_message {
                md.push_str(&format!(" — {m}"));
            }
            md.push('\n');
        }
        md.push('\n');
    }

    let reds = compute_redirects(cap, filter, top);
    let storms: Vec<_> = reds.groups.iter().filter(|g| g.is_storm).collect();
    if !storms.is_empty() {
        md.push_str("## Redirect Storms\n\n");
        for g in storms {
            md.push_str(&format!(
                "- {}x [{}] `{} {}{}`\n",
                g.count, g.status, g.method, g.host, g.norm_path
            ));
        }
        md.push('\n');
    }

    let slow = compute_slowest(cap, filter, top);
    if !slow.entries.is_empty() {
        md.push_str("## Slowest Requests\n\n");
        for e in &slow.entries {
            md.push_str(&format!(
                "- {} `{} {}{}` [{}] — {}\n",
                human_ms(e.duration_ms),
                e.method,
                e.host,
                e.norm_path,
                e.status,
                e.bottleneck
            ));
        }
        md.push('\n');
    }

    md
}

#[cfg(test)]
mod tests {
    use super::compose_report;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        let d0 = sample_entry(0, "api.x", "POST", "/resolve", 200);
        let d1 = sample_entry(1, "api.x", "POST", "/resolve", 200);
        let mut err = sample_entry(2, "api.x", "GET", "/missing", 404);
        err.resp_body = Some(r#"{"message":"nope"}"#.to_string());
        sample_capture(vec![d0, d1, err])
    }

    #[test]
    fn report_has_expected_sections() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("# wiretrail report"));
        assert!(md.contains("## Executive Summary"));
        assert!(md.contains("## Subsystems"));
        assert!(md.contains("## Duplicate Index"));
        assert!(md.contains("## Errors"));
    }

    #[test]
    fn duplicate_index_lists_the_repeated_call() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("POST api.x/resolve"));
    }

    #[test]
    fn error_message_is_included() {
        let md = compose_report(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10, false);
        assert!(md.contains("nope"));
    }
}
