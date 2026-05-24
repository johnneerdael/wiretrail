use crate::analysis::{auth, duplicates, errors, rate_limit, redirects, retries, slowest, storms};
use crate::filter::Filter;
use crate::model::Capture;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub severity: String, // "critical" | "high" | "medium" | "low"
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub command: String,        // drill-down subcommand
    pub filter: Option<String>, // scoping filter expression, if any
}

impl Recommendation {
    /// The reproducing command tail, e.g. `errors --filter "host:api.x"` or `auth`.
    pub fn command_line(&self) -> String {
        match &self.filter {
            Some(f) => format!("{} --filter \"{}\"", self.command, f),
            None => self.command.clone(),
        }
    }
}

/// Severity ordering shared across the recommender, diagnose, summary, and auto.
pub fn sev_rank(s: &str) -> u8 {
    match s {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

/// Rank actionable recommendations by composing the existing analyses.
pub fn recommend(cap: &Capture, filter: &Filter, top: usize) -> Vec<Recommendation> {
    let mut f: Vec<Recommendation> = Vec::new();

    // 5xx clusters / 4xx groups
    for g in errors::compute_errors(cap, filter, top, false).groups {
        if (500..600).contains(&g.status) && g.count >= 3 {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "5xx-cluster".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "server error cluster".into()),
                evidence_ids: g.entry_ids.clone(),
                command: "errors".into(),
                filter: Some(format!("host:{}", g.host)),
            });
        } else if (400..500).contains(&g.status) {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "4xx".into(),
                title: format!("{}x {} on {} {}", g.count, g.status, g.method, g.norm_path),
                detail: g
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "client error".into()),
                evidence_ids: g.entry_ids.clone(),
                command: "errors".into(),
                filter: None,
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
            f.push(Recommendation {
                severity: "high".into(),
                kind: "token-refresh-race".into(),
                title: format!("suspicious token refresh on {}", rf.host),
                detail: why.into(),
                evidence_ids: ids,
                command: "auth".into(),
                filter: None,
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
        f.push(Recommendation {
            severity: "medium".into(),
            kind: "auth-failures".into(),
            title: format!("{total} auth failures (401/403)"),
            detail: "requests rejected for authentication/authorization".into(),
            evidence_ids: ids,
            command: "auth".into(),
            filter: None,
        });
    }

    // rate-limit without backoff
    for g in rate_limit::compute_rate_limit(cap, filter, top).groups {
        if g.cooldown_violated {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "rate-limit-no-backoff".into(),
                title: format!("calls during 429 cooldown on {} {}", g.host, g.norm_path),
                detail: format!(
                    "{} 429s, follow-ups before Retry-After elapsed",
                    g.count_429
                ),
                evidence_ids: g.entry_ids.clone(),
                command: "rate-limit".into(),
                filter: None,
            });
        }
    }

    // retry exhaustion
    for g in retries::compute_retries(cap, filter, top).groups {
        if g.retry_count >= 3 && !(200..300).contains(&g.final_status) {
            f.push(Recommendation {
                severity: "high".into(),
                kind: "retry-exhaustion".into(),
                title: format!(
                    "{} retries, final {} on {} {}",
                    g.retry_count, g.final_status, g.method, g.norm_path
                ),
                detail: "repeated retries did not recover".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "retries".into(),
                filter: None,
            });
        }
    }

    // request storms
    for s in storms::compute_storms(cap, filter, 1000, 5, top).storms {
        if s.peak_count >= 10 {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "request-storm".into(),
                title: format!(
                    "{} {} calls/s burst to {}",
                    s.peak_count, s.scope_kind, s.scope
                ),
                detail: "burst of calls in a 1s window".into(),
                evidence_ids: s.entry_ids.clone(),
                command: "storms".into(),
                filter: None,
            });
        }
    }

    // wasteful duplicates (not retries)
    for g in duplicates::compute_duplicates(cap, filter, top).groups {
        if g.count >= 10 && !g.is_retry_pattern {
            f.push(Recommendation {
                severity: "medium".into(),
                kind: "wasteful-duplicates".into(),
                title: format!("{}x identical {} {}", g.count, g.method, g.norm_path),
                detail: "repeated identical calls (not retries)".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "diff".into(),
                filter: None,
            });
        }
    }

    // redirect storms
    for g in redirects::compute_redirects(cap, filter, top).groups {
        if g.is_storm {
            f.push(Recommendation {
                severity: "low".into(),
                kind: "redirect-storm".into(),
                title: format!(
                    "{}x [{}] redirect on {} {}",
                    g.count, g.status, g.host, g.norm_path
                ),
                detail: "repeated redirects".into(),
                evidence_ids: g.entry_ids.clone(),
                command: "redirects".into(),
                filter: None,
            });
        }
    }

    // slow backend
    if let Some(s) = slowest::compute_slowest(cap, filter, top).entries.first()
        && s.duration_ms > 1000.0
        && s.bottleneck == "server wait/TTFB"
    {
        f.push(Recommendation {
            severity: "low".into(),
            kind: "slow-backend".into(),
            title: format!(
                "slowest call {}ms on {} {}",
                s.duration_ms as i64, s.host, s.norm_path
            ),
            detail: "dominated by server wait (TTFB)".into(),
            evidence_ids: vec![s.id.clone()],
            command: "slowest".into(),
            filter: None,
        });
    }

    f.sort_by(|a, b| {
        sev_rank(&b.severity)
            .cmp(&sev_rank(&a.severity))
            .then(b.evidence_ids.len().cmp(&a.evidence_ids.len()))
            .then(a.kind.cmp(&b.kind))
    });
    f.truncate(top);
    f
}

#[cfg(test)]
mod tests {
    use super::recommend;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn err(index: usize, path: &str, status: i64, off: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "POST", path, status);
        e.started_offset_ms = off;
        e
    }

    #[test]
    fn surfaces_5xx_cluster_as_high_with_host_filter() {
        let cap = sample_capture(vec![
            err(0, "/bulk", 500, 0.0),
            err(1, "/bulk", 500, 10.0),
            err(2, "/bulk", 500, 20.0),
        ]);
        let recs = recommend(&cap, &Filter::parse(&[]).unwrap(), 20);
        let top = &recs[0];
        assert_eq!(top.severity, "high");
        assert_eq!(top.kind, "5xx-cluster");
        assert_eq!(top.command, "errors");
        assert_eq!(top.filter.as_deref(), Some("host:api.x"));
        assert_eq!(top.command_line(), "errors --filter \"host:api.x\"");
    }

    #[test]
    fn clean_capture_yields_no_recommendations() {
        let cap = sample_capture(vec![sample_entry(0, "api.x", "GET", "/ok", 200)]);
        assert!(recommend(&cap, &Filter::parse(&[]).unwrap(), 20).is_empty());
    }
}
