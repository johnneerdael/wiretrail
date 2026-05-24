use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct RateLimitResult {
    pub groups: Vec<RateLimitGroup>,
}

#[derive(Debug, Serialize)]
pub struct RateLimitGroup {
    pub host: String,
    pub norm_path: String,
    pub count_429: usize,
    pub retry_after_secs: Vec<f64>,
    pub ratelimit_headers: BTreeMap<String, String>,
    pub cooldown_violated: bool,
    pub violating_ids: Vec<String>,
    pub entry_ids: Vec<String>,
}

fn header<'a>(e: &'a Entry, name: &str) -> Option<&'a str> {
    e.resp_headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn is_limited(e: &Entry) -> bool {
    e.status == 429 || header(e, "x-ratelimit-remaining") == Some("0")
}

/// Detect rate-limit events and cooldown violations.
pub fn compute_rate_limit(cap: &Capture, filter: &Filter, top: usize) -> RateLimitResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    // Index entries by route for cooldown follow-up lookups.
    let mut by_route: AHashMap<(String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((e.host.clone(), e.norm_path.clone()))
            .or_default()
            .push(e);
    }

    let mut groups: Vec<RateLimitGroup> = Vec::new();
    for ((host, np), members) in &by_route {
        let limited: Vec<&&Entry> = members.iter().filter(|e| is_limited(e)).collect();
        if limited.is_empty() {
            continue;
        }

        let count_429 = limited.iter().filter(|e| e.status == 429).count();
        let mut retry_after_secs: Vec<f64> = Vec::new();
        let mut ratelimit_headers: BTreeMap<String, String> = BTreeMap::new();
        let mut violating_ids: Vec<String> = Vec::new();

        for lim in &limited {
            if let Some(ra) = header(lim, "retry-after").and_then(|v| v.trim().parse::<f64>().ok())
            {
                retry_after_secs.push(ra);
                let cooldown_end = lim.started_offset_ms + ra * 1000.0;
                for e in members.iter() {
                    if e.started_offset_ms > lim.started_offset_ms
                        && e.started_offset_ms < cooldown_end
                        && !violating_ids.contains(&e.id)
                    {
                        violating_ids.push(e.id.clone());
                    }
                }
            }
            for (n, v) in &lim.resp_headers {
                let ln = n.to_ascii_lowercase();
                if ln.starts_with("x-ratelimit") {
                    ratelimit_headers.entry(ln).or_insert_with(|| v.clone());
                }
            }
        }
        violating_ids.sort();

        groups.push(RateLimitGroup {
            host: host.clone(),
            norm_path: np.clone(),
            count_429,
            retry_after_secs,
            ratelimit_headers,
            cooldown_violated: !violating_ids.is_empty(),
            violating_ids,
            entry_ids: limited.iter().map(|e| e.id.clone()).collect(),
        });
    }

    groups.sort_by(|a, b| {
        b.count_429
            .cmp(&a.count_429)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    RateLimitResult { groups }
}

/// Render rate-limit findings as deterministic terminal text.
pub fn render_rate_limit_text(r: &RateLimitResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail rate-limit ==\n");
    for g in &r.groups {
        let tag = if g.cooldown_violated {
            " [cooldown violated]"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n{} {}  ({}x 429){}\n",
            g.host, g.norm_path, g.count_429, tag
        ));
        if !g.retry_after_secs.is_empty() {
            let ras: Vec<String> = g.retry_after_secs.iter().map(|s| format!("{s}s")).collect();
            out.push_str(&format!("  retry-after: {}\n", ras.join(", ")));
        }
        for (k, v) in &g.ratelimit_headers {
            out.push_str(&format!("  {k}: {v}\n"));
        }
        if !g.violating_ids.is_empty() {
            out.push_str(&format!(
                "  called during cooldown: {}\n",
                g.violating_ids.join(", ")
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_rate_limit;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn limited(index: usize, offset_ms: f64, retry_after: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/data", 429);
        e.started_offset_ms = offset_ms;
        e.resp_headers = vec![
            ("Retry-After".to_string(), retry_after.to_string()),
            ("X-RateLimit-Remaining".to_string(), "0".to_string()),
        ];
        e
    }

    fn ok(index: usize, offset_ms: f64) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/data", 200);
        e.started_offset_ms = offset_ms;
        e
    }

    #[test]
    fn groups_429_and_parses_retry_after() {
        let cap = sample_capture(vec![limited(0, 0.0, "30")]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert_eq!(r.groups.len(), 1);
        let g = &r.groups[0];
        assert_eq!(g.count_429, 1);
        assert_eq!(g.retry_after_secs, vec![30.0]);
        assert_eq!(
            g.ratelimit_headers
                .get("x-ratelimit-remaining")
                .map(String::as_str),
            Some("0")
        );
    }

    #[test]
    fn flags_cooldown_violation() {
        // 429 at t=0 with Retry-After 10s; a follow-up call at t=2s violates cooldown
        let cap = sample_capture(vec![limited(0, 0.0, "10"), ok(1, 2000.0)]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups[0].cooldown_violated);
        assert_eq!(r.groups[0].violating_ids, vec!["e000001"]);
    }

    #[test]
    fn respected_cooldown_not_flagged() {
        // follow-up at t=20s is after the 10s cooldown
        let cap = sample_capture(vec![limited(0, 0.0, "10"), ok(1, 20000.0)]);
        let r = compute_rate_limit(&cap, &Filter::parse(&[]).unwrap(), 10);
        assert!(!r.groups[0].cooldown_violated);
    }
}
