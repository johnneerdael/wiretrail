use crate::config::Config;
use crate::filter::Filter;
use crate::fingerprint::fingerprint;
use crate::model::{Capture, Entry};
use crate::render::human_ms;
use ahash::{AHashMap, AHashSet};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SubsystemsResult {
    pub subsystems: Vec<SubsystemStat>,
}

#[derive(Debug, Serialize)]
pub struct SubsystemStat {
    pub name: String,
    pub owner: Option<String>,
    pub criticality: Option<String>,
    pub count: usize,
    pub hosts: Vec<String>,
    pub error_count: usize,
    pub duplicate_count: usize,
    pub first_offset_ms: f64,
    pub last_offset_ms: f64,
}

struct Acc<'a> {
    owner: Option<String>,
    criticality: Option<String>,
    entries: Vec<&'a Entry>,
    hosts: AHashSet<String>,
}

/// Aggregate the filtered capture per resolved subsystem. `top` bounds the list.
pub fn compute_subsystems(
    cap: &Capture,
    filter: &Filter,
    config: &Config,
    top: usize,
) -> SubsystemsResult {
    let mut by_name: AHashMap<String, Acc> = AHashMap::new();
    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let sub = config.subsystem_for(e);
        let acc = by_name.entry(sub.name.clone()).or_insert_with(|| Acc {
            owner: sub.owner.clone(),
            criticality: sub.criticality.clone(),
            entries: Vec::new(),
            hosts: AHashSet::new(),
        });
        acc.entries.push(e);
        acc.hosts.insert(e.host.clone());
    }

    let mut subsystems: Vec<SubsystemStat> = by_name
        .into_iter()
        .map(|(name, acc)| subsystem_stat(name, acc))
        .collect();

    subsystems.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    subsystems.truncate(top);
    SubsystemsResult { subsystems }
}

fn subsystem_stat(name: String, acc: Acc) -> SubsystemStat {
    let mut fp_counts: AHashMap<String, usize> = AHashMap::new();
    let mut error_count = 0usize;
    let mut first = f64::MAX;
    let mut last = f64::MIN;
    for e in &acc.entries {
        if e.is_error() {
            error_count += 1;
        }
        first = first.min(e.started_offset_ms);
        last = last.max(e.started_offset_ms);
        *fp_counts.entry(fingerprint(e)).or_default() += 1;
    }
    let duplicate_count: usize = fp_counts.values().filter(|c| **c > 1).sum();
    let mut hosts: Vec<String> = acc.hosts.into_iter().collect();
    hosts.sort();

    SubsystemStat {
        name,
        owner: acc.owner,
        criticality: acc.criticality,
        count: acc.entries.len(),
        hosts,
        error_count,
        duplicate_count,
        first_offset_ms: if first == f64::MAX { 0.0 } else { first },
        last_offset_ms: if last == f64::MIN { 0.0 } else { last },
    }
}

/// Render the dossier-style subsystem category table as terminal text.
pub fn render_subsystems_text(r: &SubsystemsResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail subsystems ==\n");
    for s in &r.subsystems {
        let owner = s.owner.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "\n{}  [{}]  ({} req, {} err, {} dup)\n",
            s.name, owner, s.count, s.error_count, s.duplicate_count
        ));
        out.push_str(&format!(
            "  window: {} - {}\n",
            human_ms(s.first_offset_ms),
            human_ms(s.last_offset_ms)
        ));
        out.push_str(&format!("  hosts: {}\n", s.hosts.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_subsystems;
    use crate::config::Config;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry};

    fn cap() -> crate::model::Capture {
        sample_capture(vec![
            sample_entry(0, "api.github.com", "GET", "/repos/x", 200),
            sample_entry(1, "raw.githubusercontent.com", "GET", "/y", 404),
            sample_entry(2, "torii.nexioapp.org", "GET", "/manifest.json", 308),
        ])
    }

    #[test]
    fn groups_by_resolved_subsystem() {
        let r = compute_subsystems(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10);
        let gh = r.subsystems.iter().find(|s| s.name == "GitHub").unwrap();
        // both github hosts collapse into one subsystem
        assert_eq!(gh.count, 2);
        assert_eq!(gh.error_count, 1);
        assert_eq!(gh.hosts.len(), 2);
        // unknown host becomes its own subsystem named after the host
        assert!(r.subsystems.iter().any(|s| s.name == "torii.nexioapp.org"));
    }

    #[test]
    fn sorted_by_count_desc() {
        let r = compute_subsystems(&cap(), &Filter::parse(&[]).unwrap(), &Config::default(), 10);
        assert_eq!(r.subsystems[0].name, "GitHub");
    }
}
