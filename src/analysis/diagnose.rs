use crate::analysis::{auth, duplicates, errors, rate_limit, redirects, retries, slowest, storms};
use crate::filter::Filter;
use crate::model::Capture;
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

fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

/// Synthesize ranked root-cause findings by composing the existing analyses.
pub fn compute_diagnose(cap: &Capture, filter: &Filter, top: usize) -> DiagnoseResult {
    let mut f: Vec<Diagnosis> = Vec::new();

    // 5xx clusters / 4xx groups
    for g in errors::compute_errors(cap, filter, top, false).groups {
        if (500..600).contains(&g.status) && g.count >= 3 {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "5xx-cluster".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "server error cluster".into()),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: format!("errors --filter \"host:{}\"", g.host),
            });
        } else if (400..500).contains(&g.status) {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "4xx".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "client error".into()),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "errors".into(),
            });
        }
    }

    // auth: refresh races + failures
    let a = auth::compute_auth(cap, filter, top);
    for rf in &a.refreshes {
        if rf.old_token_reused || !rf.success {
            let why = if rf.old_token_reused {
                "refresh succeeded but later calls reused the old token"
            } else {
                "token refresh failed"
            };
            let mut ids = vec![rf.id.clone()];
            ids.extend(rf.reusing_ids.clone());
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "token-refresh-race".into(),
                title: format!("suspicious token refresh on {}", rf.host),
                detail: why.into(),
                evidence_ids: ids,
                suggested_command: "auth".into(),
            });
        }
    }
    if !a.failures.is_empty() {
        let total: usize = a.failures.iter().map(|x| x.count).sum();
        let ids: Vec<String> = a
            .failures
            .iter()
            .flat_map(|x| x.entry_ids.clone())
            .collect();
        f.push(Diagnosis {
            severity: "medium".into(),
            kind: "auth-failures".into(),
            title: format!("{total} auth failures (401/403)"),
            detail: "requests rejected for authentication/authorization".into(),
            evidence_ids: ids,
            suggested_command: "auth".into(),
        });
    }

    // rate-limit without backoff
    for g in rate_limit::compute_rate_limit(cap, filter, top).groups {
        if g.cooldown_violated {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "rate-limit-no-backoff".into(),
                title: format!("calls during 429 cooldown on {} {}", g.host, g.norm_path),
                detail: format!(
                    "{} 429s, follow-ups before Retry-After elapsed",
                    g.count_429
                ),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "rate-limit".into(),
            });
        }
    }

    // retry exhaustion
    for g in retries::compute_retries(cap, filter, top).groups {
        if g.retry_count >= 3 && !(200..300).contains(&g.final_status) {
            f.push(Diagnosis {
                severity: "high".into(),
                kind: "retry-exhaustion".into(),
                title: format!(
                    "{} retries, final {} on {} {}",
                    g.retry_count, g.final_status, g.method, g.norm_path
                ),
                detail: "repeated retries did not recover".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "retries".into(),
            });
        }
    }

    // request storms
    for s in storms::compute_storms(cap, filter, 1000, 5, top).storms {
        if s.peak_count >= 10 {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "request-storm".into(),
                title: format!(
                    "{} {} calls/s burst to {}",
                    s.peak_count, s.scope_kind, s.scope
                ),
                detail: "burst of calls in a 1s window".into(),
                evidence_ids: s.entry_ids.clone(),
                suggested_command: "storms".into(),
            });
        }
    }

    // wasteful duplicates (not retries)
    for g in duplicates::compute_duplicates(cap, filter, top).groups {
        if g.count >= 10 && !g.is_retry_pattern {
            f.push(Diagnosis {
                severity: "medium".into(),
                kind: "wasteful-duplicates".into(),
                title: format!("{}x identical {} {}", g.count, g.method, g.norm_path),
                detail: "repeated identical calls (not retries)".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "diff".into(),
            });
        }
    }

    // redirect storms
    for g in redirects::compute_redirects(cap, filter, top).groups {
        if g.is_storm {
            f.push(Diagnosis {
                severity: "low".into(),
                kind: "redirect-storm".into(),
                title: format!(
                    "{}x [{}] redirect on {} {}",
                    g.count, g.status, g.host, g.norm_path
                ),
                detail: "repeated redirects".into(),
                evidence_ids: g.entry_ids.clone(),
                suggested_command: "redirects".into(),
            });
        }
    }

    // slow backend
    if let Some(s) = slowest::compute_slowest(cap, filter, top).entries.first()
        && s.duration_ms > 1000.0
        && s.bottleneck == "server wait/TTFB"
    {
        f.push(Diagnosis {
            severity: "low".into(),
            kind: "slow-backend".into(),
            title: format!(
                "slowest call {}ms on {} {}",
                s.duration_ms as i64, s.host, s.norm_path
            ),
            detail: "dominated by server wait (TTFB)".into(),
            evidence_ids: vec![s.id.clone()],
            suggested_command: "slowest".into(),
        });
    }

    f.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.evidence_ids.len().cmp(&a.evidence_ids.len()))
            .then(a.kind.cmp(&b.kind))
    });
    f.truncate(top);
    DiagnoseResult { findings: f }
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
