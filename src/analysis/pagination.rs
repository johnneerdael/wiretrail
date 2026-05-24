use crate::filter::Filter;
use crate::grouping::densest_window;
use crate::model::{Capture, Entry};
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

const PAGE_KEYS: &[&str] = &[
    "page",
    "offset",
    "cursor",
    "page_token",
    "after",
    "before",
    "start",
    "limit",
    "p",
    "pagenumber",
    "page_number",
];

#[derive(Debug, Serialize)]
pub struct PaginationResult {
    pub pages: Vec<PageSeq>,
    pub nplus1: Vec<NPlusOne>,
}

#[derive(Debug, Serialize)]
pub struct PageSeq {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub param_keys: Vec<String>,
    pub page_count: usize,
    pub repeated_cursor: bool,
    pub excessive: bool,
    pub entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct NPlusOne {
    pub host: String,
    pub method: String,
    pub norm_path: String,
    pub fanout: usize,
    pub parent_id: Option<String>,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
    pub entry_ids: Vec<String>,
}

fn is_id_bearing(np: &str) -> bool {
    np.contains("{id}") || np.contains("{blob}")
}

/// Detect pagination sequences and N+1 fan-out clusters.
pub fn compute_pagination(
    cap: &Capture,
    filter: &Filter,
    max_pages: usize,
    fanout_min: usize,
    window_ms: u64,
    top: usize,
) -> PaginationResult {
    let entries: Vec<&Entry> = cap.entries.iter().filter(|e| filter.matches(e)).collect();

    let mut by_route: AHashMap<(String, String, String), Vec<&Entry>> = AHashMap::new();
    for e in &entries {
        by_route
            .entry((
                e.method.to_ascii_uppercase(),
                e.host.clone(),
                e.norm_path.clone(),
            ))
            .or_default()
            .push(e);
    }

    let mut pages = Vec::new();
    let mut nplus1 = Vec::new();

    for ((method, host, np), mut group) in by_route {
        group.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });

        // --- pagination sequence ---
        // A group of repeated calls to the same route that carry a pagination
        // key, where nothing *other* than pagination keys varies. Covers both a
        // normal page walk (cursor varies) and a loop (cursor repeats).
        if group.len() >= 2 {
            let varying = varying_query_keys(&group);
            let non_page_varies = varying
                .iter()
                .any(|k| !PAGE_KEYS.contains(&k.to_ascii_lowercase().as_str()));
            let mut page_keys_present: Vec<String> = page_keys_in(&group);
            page_keys_present.sort();

            if !page_keys_present.is_empty() && !non_page_varies {
                let mut values: Vec<String> = Vec::new();
                for e in &group {
                    for (k, v) in &e.query {
                        if page_keys_present.iter().any(|pk| pk == k) {
                            values.push(v.clone());
                        }
                    }
                }
                let repeated_cursor = has_duplicate(&values);
                pages.push(PageSeq {
                    host: host.clone(),
                    method: method.clone(),
                    norm_path: np.clone(),
                    param_keys: page_keys_present,
                    page_count: group.len(),
                    repeated_cursor,
                    excessive: group.len() > max_pages,
                    entry_ids: group.iter().map(|e| e.id.clone()).collect(),
                });
            }
        }

        // --- N+1 fan-out ---
        if is_id_bearing(&np) && group.len() >= fanout_min {
            let (count, l, r) = densest_window(&group, window_ms as f64);
            if count >= fanout_min {
                let win = &group[l..=r];
                let first = win.first().unwrap().started_offset_ms;
                let parent_id = parent_list_call(&entries, &host, first);
                nplus1.push(NPlusOne {
                    host: host.clone(),
                    method: method.clone(),
                    norm_path: np.clone(),
                    fanout: count,
                    parent_id,
                    first_offset_ms: first,
                    last_offset_ms: win.last().unwrap().started_offset_ms,
                    entry_ids: win.iter().map(|e| e.id.clone()).collect(),
                });
            }
        }
    }

    pages.sort_by(|a, b| {
        b.page_count
            .cmp(&a.page_count)
            .then(a.norm_path.cmp(&b.norm_path))
    });
    nplus1.sort_by(|a, b| b.fanout.cmp(&a.fanout).then(a.norm_path.cmp(&b.norm_path)));
    pages.truncate(top);
    nplus1.truncate(top);
    PaginationResult { pages, nplus1 }
}

/// Query keys whose value differs across the group (missing counts as a value).
fn varying_query_keys(members: &[&Entry]) -> Vec<String> {
    let all_keys: AHashSet<String> = members
        .iter()
        .flat_map(|e| e.query.iter().map(|(k, _)| k.clone()))
        .collect();
    let mut varying: Vec<String> = Vec::new();
    for k in all_keys {
        let mut vals: AHashSet<String> = AHashSet::new();
        for e in members {
            let v = e
                .query
                .iter()
                .find(|(qk, _)| *qk == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            vals.insert(v);
        }
        if vals.len() > 1 {
            varying.push(k);
        }
    }
    varying.sort();
    varying
}

/// Distinct pagination query keys present anywhere in the group.
fn page_keys_in(members: &[&Entry]) -> Vec<String> {
    let mut set: AHashSet<String> = AHashSet::new();
    for e in members {
        for (k, _) in &e.query {
            if PAGE_KEYS.contains(&k.to_ascii_lowercase().as_str()) {
                set.insert(k.clone());
            }
        }
    }
    set.into_iter().collect()
}

fn has_duplicate(values: &[String]) -> bool {
    let mut seen: AHashSet<&String> = AHashSet::new();
    for v in values {
        if !seen.insert(v) {
            return true;
        }
    }
    false
}

/// The most recent non-id-bearing call to the same host before `before_offset`.
fn parent_list_call(entries: &[&Entry], host: &str, before_offset: f64) -> Option<String> {
    entries
        .iter()
        .filter(|e| {
            e.host == host && e.started_offset_ms < before_offset && !is_id_bearing(&e.norm_path)
        })
        .max_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|e| e.id.clone())
}

/// Render pagination/N+1 as deterministic terminal text.
pub fn render_pagination_text(r: &PaginationResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail pagination ==\n");
    if !r.pages.is_empty() {
        out.push_str("\npagination sequences:\n");
        for p in &r.pages {
            let mut tags = Vec::new();
            if p.repeated_cursor {
                tags.push("repeated-cursor");
            }
            if p.excessive {
                tags.push("excessive");
            }
            let tagstr = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags.join(", "))
            };
            out.push_str(&format!(
                "  {} pages  {} {}{}  (by {}){}\n",
                p.page_count,
                p.method,
                p.host,
                p.norm_path,
                p.param_keys.join(","),
                tagstr
            ));
        }
    }
    if !r.nplus1.is_empty() {
        out.push_str("\nN+1 fan-out:\n");
        for n in &r.nplus1 {
            let parent = n.parent_id.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "  {}x  {} {}{}  (after {})\n",
                n.fanout, n.method, n.host, n.norm_path, parent
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_pagination;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn page(index: usize, page: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/items", 200);
        e.query = vec![("page".to_string(), page.to_string())];
        e.started_offset_ms = index as f64 * 10.0;
        e
    }

    #[test]
    fn detects_pagination_sequence() {
        let cap = sample_capture(vec![page(0, "1"), page(1, "2"), page(2, "3")]);
        let r = compute_pagination(&cap, &Filter::parse(&[]).unwrap(), 20, 5, 2000, 10);
        assert_eq!(r.pages.len(), 1);
        let p = &r.pages[0];
        assert_eq!(p.page_count, 3);
        assert_eq!(p.param_keys, vec!["page".to_string()]);
        assert!(!p.repeated_cursor);
        assert!(!p.excessive);
    }

    #[test]
    fn flags_repeated_cursor() {
        let cap = sample_capture(vec![page(0, "abc"), page(1, "abc")]);
        let r = compute_pagination(&cap, &Filter::parse(&[]).unwrap(), 20, 5, 2000, 10);
        assert!(r.pages[0].repeated_cursor);
    }

    #[test]
    fn detects_nplus1_fanout() {
        // one list call, then 5 detail calls to an {id} endpoint within window
        let mut es = Vec::new();
        let mut list = sample_entry(0, "api.x", "GET", "/items", 200);
        list.started_offset_ms = 0.0;
        es.push(list);
        for i in 1..=5 {
            let mut e = sample_entry(i, "api.x", "GET", "/items/{id}", 200);
            e.started_offset_ms = i as f64 * 20.0;
            es.push(e);
        }
        let r = compute_pagination(
            &sample_capture(es),
            &Filter::parse(&[]).unwrap(),
            20,
            5,
            2000,
            10,
        );
        assert_eq!(r.nplus1.len(), 1);
        let n = &r.nplus1[0];
        assert_eq!(n.fanout, 5);
        assert_eq!(n.norm_path, "/items/{id}");
        assert_eq!(n.parent_id.as_deref(), Some("e000000"));
    }
}
