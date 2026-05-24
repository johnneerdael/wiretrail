use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuthResult {
    pub failures: Vec<AuthFailure>,
    pub missing_auth_hosts: Vec<String>,
    pub token_changes: Vec<TokenChange>,
    pub refreshes: Vec<RefreshEvent>,
}

#[derive(Debug, Serialize)]
pub struct AuthFailure {
    pub host: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenChange {
    pub host: String,
    pub distinct_tokens: usize,
}

#[derive(Debug, Serialize)]
pub struct RefreshEvent {
    pub id: String,
    pub host: String,
    pub status: i64,
    pub success: bool,
    pub concurrent: bool,
    pub old_token_reused: bool,
    pub reusing_ids: Vec<String>,
}

fn auth_value(e: &Entry) -> Option<&str> {
    e.req_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str())
}

fn is_refresh(e: &Entry) -> bool {
    let p = e.norm_path.to_ascii_lowercase();
    let path_hit = p.contains("/token") || p.contains("/oauth") || p.contains("/auth/refresh");
    let query_hit = e
        .query
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("grant_type") && v == "refresh_token");
    path_hit || query_hit
}

/// Analyze auth failures, missing/rotating auth, and token-refresh flows.
pub fn compute_auth(cap: &Capture, filter: &Filter, top: usize) -> AuthResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    // --- failures (401/403) ---
    let mut fail_map: AHashMap<(String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        if e.status == 401 || e.status == 403 {
            fail_map
                .entry((e.host.clone(), e.norm_path.clone(), e.status))
                .or_default()
                .push(e);
        }
    }
    let mut failures: Vec<AuthFailure> = fail_map
        .into_iter()
        .map(|((host, norm_path, status), g)| AuthFailure {
            host,
            norm_path,
            status,
            count: g.len(),
            entry_ids: g.iter().map(|e| e.id.clone()).collect(),
        })
        .collect();
    failures.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    failures.truncate(top);

    // --- per-host auth presence + distinct tokens ---
    let mut host_has_auth: AHashMap<String, bool> = AHashMap::new();
    let mut host_no_auth: AHashMap<String, bool> = AHashMap::new();
    let mut host_tokens: AHashMap<String, AHashSet<String>> = AHashMap::new();
    for e in &entries {
        match auth_value(e) {
            Some(a) => {
                *host_has_auth.entry(e.host.clone()).or_default() = true;
                host_tokens
                    .entry(e.host.clone())
                    .or_default()
                    .insert(a.to_string());
            }
            None => {
                *host_no_auth.entry(e.host.clone()).or_default() = true;
            }
        }
    }
    let mut missing_auth_hosts: Vec<String> = host_has_auth
        .keys()
        .filter(|h| host_no_auth.get(*h).copied().unwrap_or(false))
        .cloned()
        .collect();
    missing_auth_hosts.sort();

    let mut token_changes: Vec<TokenChange> = host_tokens
        .into_iter()
        .filter(|(_, set)| set.len() > 1)
        .map(|(host, set)| TokenChange {
            host,
            distinct_tokens: set.len(),
        })
        .collect();
    token_changes.sort_by(|a, b| {
        b.distinct_tokens
            .cmp(&a.distinct_tokens)
            .then(a.host.cmp(&b.host))
    });

    // --- token refresh flows ---
    // Global timeline of (offset, auth value, id) for reuse analysis.
    let mut auth_timeline: Vec<(f64, String, String)> = entries
        .iter()
        .filter_map(|e| auth_value(e).map(|a| (e.started_offset_ms, a.to_string(), e.id.clone())))
        .collect();
    auth_timeline.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let refresh_entries: Vec<&&Entry> = entries.iter().filter(|e| is_refresh(e)).collect();
    let mut refreshes: Vec<RefreshEvent> = Vec::new();
    for rf in &refresh_entries {
        let t = rf.started_offset_ms;
        let success = (200..300).contains(&rf.status);

        let pre: AHashSet<&String> = auth_timeline
            .iter()
            .filter(|(o, _, _)| *o < t)
            .map(|(_, a, _)| a)
            .collect();
        let new_token_seen = auth_timeline
            .iter()
            .any(|(o, a, _)| *o > t && !pre.contains(a));
        let reusing_ids: Vec<String> = auth_timeline
            .iter()
            .filter(|(o, a, _)| *o > t && pre.contains(a))
            .map(|(_, _, id)| id.clone())
            .collect();
        let old_token_reused = success && new_token_seen && !reusing_ids.is_empty();

        let concurrent = refresh_entries.iter().any(|other| {
            other.id != rf.id && (other.started_offset_ms - t).abs() < rf.duration_ms.max(1.0)
        });

        refreshes.push(RefreshEvent {
            id: rf.id.clone(),
            host: rf.host.clone(),
            status: rf.status,
            success,
            concurrent,
            old_token_reused,
            reusing_ids,
        });
    }
    refreshes.sort_by(|a, b| a.id.cmp(&b.id));

    AuthResult {
        failures,
        missing_auth_hosts,
        token_changes,
        refreshes,
    }
}

/// Render auth analysis as deterministic terminal text.
pub fn render_auth_text(r: &AuthResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail auth ==\n");
    if !r.failures.is_empty() {
        out.push_str("\nauth failures:\n");
        for f in &r.failures {
            out.push_str(&format!(
                "  {}x [{}] {} {}\n",
                f.count, f.status, f.host, f.norm_path
            ));
        }
    }
    if !r.missing_auth_hosts.is_empty() {
        out.push_str(&format!(
            "\nhosts with inconsistent Authorization: {}\n",
            r.missing_auth_hosts.join(", ")
        ));
    }
    if !r.token_changes.is_empty() {
        out.push_str("\ntoken rotation:\n");
        for t in &r.token_changes {
            out.push_str(&format!(
                "  {} ({} distinct tokens)\n",
                t.host, t.distinct_tokens
            ));
        }
    }
    if !r.refreshes.is_empty() {
        out.push_str("\ntoken refreshes:\n");
        for rf in &r.refreshes {
            let mut tags = Vec::new();
            if !rf.success {
                tags.push("failed".to_string());
            }
            if rf.old_token_reused {
                tags.push("old-token-reused".to_string());
            }
            if rf.concurrent {
                tags.push("concurrent".to_string());
            }
            let tagstr = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags.join(", "))
            };
            out.push_str(&format!(
                "  {} {} [{}]{}\n",
                rf.id, rf.host, rf.status, tagstr
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_auth;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn with_auth(
        index: usize,
        host: &str,
        path: &str,
        status: i64,
        auth: Option<&str>,
        offset: f64,
    ) -> Entry {
        let mut e = sample_entry(index, host, "GET", path, status);
        e.started_offset_ms = offset;
        if let Some(a) = auth {
            e.req_headers = vec![("Authorization".to_string(), a.to_string())];
        } else {
            e.req_headers = vec![];
        }
        e
    }

    #[test]
    fn groups_401_failures() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/me", 401, Some("Bearer a"), 0.0),
            with_auth(1, "api.x", "/me", 401, Some("Bearer a"), 10.0),
            with_auth(2, "api.x", "/ok", 200, Some("Bearer a"), 20.0),
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        let f = r.failures.iter().find(|f| f.norm_path == "/me").unwrap();
        assert_eq!(f.count, 2);
        assert_eq!(f.status, 401);
    }

    #[test]
    fn flags_host_missing_auth_inconsistently() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/a", 200, Some("Bearer a"), 0.0),
            with_auth(1, "api.x", "/b", 200, None, 10.0),
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.missing_auth_hosts.contains(&"api.x".to_string()));
    }

    #[test]
    fn detects_old_token_reuse_after_refresh() {
        let cap = sample_capture(vec![
            with_auth(0, "api.x", "/data", 200, Some("Bearer OLD"), 0.0),
            // refresh call (path contains /token + grant_type query)
            {
                let mut e = sample_entry(1, "auth.x", "POST", "/auth/v1/token", 200);
                e.started_offset_ms = 100.0;
                e.query = vec![("grant_type".to_string(), "refresh_token".to_string())];
                e
            },
            with_auth(2, "api.x", "/data", 200, Some("Bearer OLD"), 200.0), // reuses old
            with_auth(3, "api.x", "/data", 200, Some("Bearer NEW"), 300.0), // new token seen
        ]);
        let r = compute_auth(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.refreshes.len(), 1);
        let rf = &r.refreshes[0];
        assert!(rf.success);
        assert!(rf.old_token_reused);
        assert!(rf.reusing_ids.contains(&"e000002".to_string()));
    }
}
