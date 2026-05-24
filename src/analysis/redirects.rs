use crate::filter::Filter;
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

const STORM_THRESHOLD: usize = 5;
const REDIRECT_STATUSES: &[i64] = &[301, 302, 303, 307, 308];

#[derive(Debug, Serialize)]
pub struct RedirectsResult {
    pub groups: Vec<RedirectGroup>,
}

#[derive(Debug, Serialize)]
pub struct RedirectGroup {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub status: i64,
    pub count: usize,
    pub target_host: Option<String>,
    pub cross_host: bool,
    pub is_storm: bool,
    pub entry_ids: Vec<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

fn is_redirect(e: &Entry) -> bool {
    REDIRECT_STATUSES.contains(&e.status)
        || e.redirect_url.as_deref().is_some_and(|u| !u.is_empty())
}

fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_string()))
}

/// Group redirect responses by (host, method, norm_path, status); flag storms
/// (count >= 5) and cross-host hops. `top` bounds the list.
pub fn compute_redirects(cap: &Capture, filter: &Filter, top: usize) -> RedirectsResult {
    let mut by_key: AHashMap<(String, String, String, i64), Vec<&Entry>> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e) && is_redirect(e)) {
        let key = (e.host.clone(), e.method.to_ascii_uppercase(), e.norm_path.clone(), e.status);
        by_key.entry(key).or_default().push(e);
    }

    let mut groups: Vec<RedirectGroup> = by_key
        .into_iter()
        .map(|((host, method, norm_path, status), mut g)| {
            g.sort_by(|a, b| {
                a.started_offset_ms
                    .partial_cmp(&b.started_offset_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.index.cmp(&b.index))
            });
            let target_host = g
                .iter()
                .find_map(|e| e.redirect_url.as_deref())
                .and_then(host_of);
            let cross_host = target_host.as_deref().is_some_and(|t| !t.is_empty() && t != host);
            RedirectGroup {
                count: g.len(),
                is_storm: g.len() >= STORM_THRESHOLD,
                cross_host,
                target_host,
                entry_ids: g.iter().map(|e| e.id.clone()).collect(),
                first_offset_ms: g.first().map(|e| e.started_offset_ms).unwrap_or(0.0),
                last_offset_ms: g.last().map(|e| e.started_offset_ms).unwrap_or(0.0),
                host,
                method,
                norm_path,
                status,
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.host.cmp(&b.host))
            .then(a.norm_path.cmp(&b.norm_path))
    });
    groups.truncate(top);
    RedirectsResult { groups }
}

/// Render redirects as deterministic terminal text.
pub fn render_redirects_text(r: &RedirectsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail redirects ==\n");
    for g in &r.groups {
        let mut tags = Vec::new();
        if g.is_storm {
            tags.push("storm");
        }
        if g.cross_host {
            tags.push("cross-host");
        }
        let tagstr = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) };
        out.push_str(&format!(
            "\n{:>4}x  [{}] {} {}{}{}\n",
            g.count, g.status, g.method, g.host, g.norm_path, tagstr
        ));
        if let Some(t) = &g.target_host {
            out.push_str(&format!("  -> {t}\n"));
        }
        out.push_str(&format!("  entries: {}\n", g.entry_ids.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_redirects;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn redirect(index: usize, host: &str, path: &str, status: i64, target: &str) -> crate::model::Entry {
        let mut e = sample_entry(index, host, "GET", path, status);
        e.redirect_url = Some(target.to_string());
        e
    }

    fn cap() -> crate::model::Capture {
        let mut entries = Vec::new();
        // 6 x 308 to torii manifest -> storm
        for i in 0..6 {
            entries.push(redirect(i, "torii.app", "/manifest.json", 308, "https://torii.app/v2/manifest.json"));
        }
        // one cross-host 302
        entries.push(redirect(6, "a.com", "/go", 302, "https://b.com/landing"));
        // a normal 200 (ignored)
        entries.push(sample_entry(7, "a.com", "GET", "/ok", 200));
        sample_capture(entries)
    }

    #[test]
    fn groups_redirects_and_flags_storm() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let storm = r.groups.iter().find(|g| g.norm_path == "/manifest.json").unwrap();
        assert_eq!(storm.count, 6);
        assert_eq!(storm.status, 308);
        assert!(storm.is_storm);
        assert!(!storm.cross_host);
    }

    #[test]
    fn flags_cross_host() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        let x = r.groups.iter().find(|g| g.norm_path == "/go").unwrap();
        assert!(x.cross_host);
        assert_eq!(x.target_host.as_deref(), Some("b.com"));
    }

    #[test]
    fn ignores_non_redirects() {
        let r = compute_redirects(&cap(), &Filter::parse(&[]).unwrap(), 10);
        assert!(r.groups.iter().all(|g| g.norm_path != "/ok"));
    }
}
