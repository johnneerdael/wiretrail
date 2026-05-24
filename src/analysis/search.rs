use crate::filter::Filter;
use crate::model::Capture;
use crate::redact::redact_value;
use serde::Serialize;

const CONTEXT: usize = 40;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub id: String,
    pub location: String,
    pub snippet: String,
}

enum Matcher {
    Regex(regex::Regex),
    Substr { needle: String, ignore_case: bool },
}

impl Matcher {
    /// Byte offset of the first match, if any.
    fn find(&self, hay: &str) -> Option<usize> {
        match self {
            Matcher::Regex(re) => re.find(hay).map(|m| m.start()),
            Matcher::Substr {
                needle,
                ignore_case,
            } => {
                if *ignore_case {
                    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
                } else {
                    hay.find(needle)
                }
            }
        }
    }
}

fn nearest_boundary(s: &str, mut i: usize, forward: bool) -> usize {
    i = i.min(s.len());
    while i > 0 && i < s.len() && !s.is_char_boundary(i) {
        if forward {
            i += 1;
        } else {
            i -= 1;
        }
    }
    i
}

fn snippet(body: &str, at: usize, unsafe_include: bool) -> String {
    let start = nearest_boundary(body, at.saturating_sub(CONTEXT), false);
    let end = nearest_boundary(body, (at + CONTEXT).min(body.len()), true);
    redact_value(&body[start..end], unsafe_include)
}

/// Search request/response bodies for a pattern; redaction-safe snippets.
pub fn compute_search(
    cap: &Capture,
    filter: &Filter,
    pattern: &str,
    regex: bool,
    ignore_case: bool,
    top: usize,
    unsafe_include: bool,
) -> Result<SearchResult, String> {
    let matcher = if regex {
        let re = regex::RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|e| format!("invalid regex: {e}"))?;
        Matcher::Regex(re)
    } else {
        Matcher::Substr {
            needle: pattern.to_string(),
            ignore_case,
        }
    };

    let mut matches = Vec::new();
    'outer: for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        for (loc, body) in [("req.body", &e.req_body), ("resp.body", &e.resp_body)] {
            if let Some(b) = body.as_deref().filter(|s| !s.is_empty())
                && let Some(at) = matcher.find(b)
            {
                matches.push(SearchMatch {
                    id: e.id.clone(),
                    location: loc.to_string(),
                    snippet: snippet(b, at, unsafe_include),
                });
                if matches.len() >= top {
                    break 'outer;
                }
            }
        }
    }
    Ok(SearchResult { matches })
}

/// Render search matches as deterministic terminal text.
pub fn render_search_text(r: &SearchResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail search ==\n");
    for m in &r.matches {
        out.push_str(&format!("\n{} ({})\n  …{}…\n", m.id, m.location, m.snippet));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_search;
    use crate::filter::Filter;
    use crate::model::{Entry, sample_capture, sample_entry};

    fn with_resp(index: usize, body: &str) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/a", 200);
        e.resp_body = Some(body.to_string());
        e
    }

    #[test]
    fn substring_match_with_snippet() {
        let cap = sample_capture(vec![with_resp(0, r#"{"message":"internal error here"}"#)]);
        let r = compute_search(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "internal error",
            false,
            false,
            10,
            false,
        )
        .unwrap();
        assert_eq!(r.matches.len(), 1);
        assert_eq!(r.matches[0].location, "resp.body");
        assert!(r.matches[0].snippet.contains("internal error"));
    }

    #[test]
    fn ignore_case() {
        let cap = sample_capture(vec![with_resp(0, "Fatal Boom")]);
        let hit = compute_search(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "fatal",
            false,
            true,
            10,
            false,
        )
        .unwrap();
        assert_eq!(hit.matches.len(), 1);
        let miss = compute_search(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "fatal",
            false,
            false,
            10,
            false,
        )
        .unwrap();
        assert!(miss.matches.is_empty());
    }

    #[test]
    fn regex_match() {
        let cap = sample_capture(vec![with_resp(0, r#"{"code":"E1234"}"#)]);
        let r = compute_search(
            &cap,
            &Filter::parse(&[]).unwrap(),
            r"E\d{4}",
            true,
            false,
            10,
            false,
        )
        .unwrap();
        assert_eq!(r.matches.len(), 1);
    }

    #[test]
    fn invalid_regex_errors() {
        let cap = sample_capture(vec![with_resp(0, "x")]);
        assert!(
            compute_search(
                &cap,
                &Filter::parse(&[]).unwrap(),
                "(",
                true,
                false,
                10,
                false
            )
            .is_err()
        );
    }

    #[test]
    fn secret_in_snippet_is_redacted() {
        let cap = sample_capture(vec![with_resp(
            0,
            "token eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ123 end",
        )]);
        let r = compute_search(
            &cap,
            &Filter::parse(&[]).unwrap(),
            "token",
            false,
            false,
            10,
            false,
        )
        .unwrap();
        assert!(!r.matches[0].snippet.contains("eyJhbGci"));
    }
}
