use crate::fingerprint::fingerprint;
use crate::model::Entry;
use ahash::{AHashMap, AHashSet};

/// Group entries by duplicate fingerprint. Each group's entries are sorted by
/// (started_offset_ms, index). Groups are returned sorted by descending size,
/// then fingerprint, for determinism.
pub fn group_by_fingerprint<'a>(entries: &[&'a Entry]) -> Vec<(String, Vec<&'a Entry>)> {
    let mut map: AHashMap<String, Vec<&'a Entry>> = AHashMap::new();
    for e in entries {
        map.entry(fingerprint(e)).or_default().push(e);
    }
    let mut groups: Vec<(String, Vec<&'a Entry>)> = map.into_iter().collect();
    for (_, g) in groups.iter_mut() {
        g.sort_by(|a, b| {
            a.started_offset_ms
                .partial_cmp(&b.started_offset_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.index.cmp(&b.index))
        });
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    groups
}

/// A request whose status indicates a transient failure worth retrying.
pub fn is_retry_trigger(e: &Entry) -> bool {
    e.status == 0 || e.status == 429 || e.status_class() == 5
}

/// True if a time-ordered fingerprint group contains an attempt that follows a
/// failed earlier attempt (i.e. the group exhibits retry behavior).
pub fn group_has_retry(group: &[&Entry]) -> bool {
    let mut seen_failure = false;
    for e in group {
        if seen_failure {
            return true;
        }
        if is_retry_trigger(e) {
            seen_failure = true;
        }
    }
    false
}

/// IDs of entries classified as retries across all fingerprint groups.
pub fn retry_entry_ids(entries: &[&Entry]) -> AHashSet<String> {
    let mut out = AHashSet::new();
    for (_, group) in group_by_fingerprint(entries) {
        let mut seen_failure = false;
        for e in &group {
            if seen_failure {
                out.insert(e.id.clone());
            }
            if is_retry_trigger(e) {
                seen_failure = true;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{group_by_fingerprint, group_has_retry, retry_entry_ids};
    use crate::model::{sample_capture, sample_entry, Entry};

    fn refs(cap: &crate::model::Capture) -> Vec<&Entry> {
        cap.entries.iter().collect()
    }

    #[test]
    fn groups_and_sorts_by_size() {
        let cap = sample_capture(vec![
            sample_entry(0, "h", "GET", "/a", 200),
            sample_entry(1, "h", "GET", "/a", 200),
            sample_entry(2, "h", "GET", "/b", 200),
        ]);
        let groups = group_by_fingerprint(&refs(&cap));
        // /a group (2) sorts before /b group (1)
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn retries_need_a_prior_failure() {
        // three identical calls: 500, 500, 200 -> 2nd and 3rd are retries
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 500),
            sample_entry(1, "h", "POST", "/x", 500),
            sample_entry(2, "h", "POST", "/x", 200),
        ]);
        let ids = retry_entry_ids(&refs(&cap));
        assert!(ids.contains("e000001"));
        assert!(ids.contains("e000002"));
        assert!(!ids.contains("e000000"));
    }

    #[test]
    fn pure_duplicates_are_not_retries() {
        // all 200 -> wasteful duplicates, no retries
        let cap = sample_capture(vec![
            sample_entry(0, "h", "POST", "/x", 200),
            sample_entry(1, "h", "POST", "/x", 200),
        ]);
        assert!(retry_entry_ids(&refs(&cap)).is_empty());
        let groups = group_by_fingerprint(&refs(&cap));
        assert!(!group_has_retry(&groups[0].1));
    }
}
