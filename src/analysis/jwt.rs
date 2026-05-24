use crate::filter::Filter;
use crate::jwt::{decode_jwt, summarize, token_hash, JwtSummary};
use crate::model::{Capture, Entry};
use ahash::AHashMap;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct JwtResult {
    pub tokens: Vec<JwtOccurrence>,
}

#[derive(Debug, Serialize)]
pub struct JwtOccurrence {
    pub token_hash: String,
    pub source: String,
    pub summary: JwtSummary,
    pub occurrences: usize,
    pub first_entry_id: String,
    pub last_entry_id: String,
    pub raw_token: Option<String>,
}

struct Acc {
    source: String,
    first_id: String,
    last_id: String,
    count: usize,
    ref_ms: Option<i64>,
}

/// Find and decode JWTs across headers, query, cookies, and bodies.
pub fn compute_jwt(cap: &Capture, filter: &Filter, top: usize, unsafe_include: bool) -> JwtResult {
    let mut map: AHashMap<String, Acc> = AHashMap::new();

    for e in cap.entries.iter().filter(|e| filter.matches(e)) {
        let ref_ms = cap.meta.start_ms.map(|s| s + e.started_offset_ms as i64);
        for (token, source) in scan_entry(e) {
            let acc = map.entry(token).or_insert_with(|| Acc {
                source,
                first_id: e.id.clone(),
                last_id: e.id.clone(),
                count: 0,
                ref_ms,
            });
            acc.count += 1;
            acc.last_id = e.id.clone();
        }
    }

    let mut tokens: Vec<JwtOccurrence> = map
        .into_iter()
        .filter_map(|(token, acc)| {
            let parts = decode_jwt(&token)?;
            let summary = summarize(&parts, acc.ref_ms);
            Some(JwtOccurrence {
                token_hash: token_hash(&token),
                source: acc.source,
                summary,
                occurrences: acc.count,
                first_entry_id: acc.first_id,
                last_entry_id: acc.last_id,
                raw_token: if unsafe_include { Some(token) } else { None },
            })
        })
        .collect();

    tokens.sort_by(|a, b| {
        let ax = a.summary.expired == Some(true);
        let bx = b.summary.expired == Some(true);
        bx.cmp(&ax)
            .then(b.occurrences.cmp(&a.occurrences))
            .then(a.token_hash.cmp(&b.token_hash))
    });
    tokens.truncate(top);
    JwtResult { tokens }
}

/// Scan an entry's headers, query, and bodies for JWTs; returns (token, source).
fn scan_entry(e: &Entry) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (n, v) in &e.req_headers {
        for t in scan_jwts(v) {
            found.push((t, format!("req.header.{}", n.to_ascii_lowercase())));
        }
    }
    for (n, v) in &e.resp_headers {
        for t in scan_jwts(v) {
            found.push((t, format!("resp.header.{}", n.to_ascii_lowercase())));
        }
    }
    for (k, v) in &e.query {
        for t in scan_jwts(v) {
            found.push((t, format!("query.{k}")));
        }
    }
    if let Some(b) = &e.req_body {
        for t in scan_jwts(b) {
            found.push((t, "req.body".to_string()));
        }
    }
    if let Some(b) = &e.resp_body {
        for t in scan_jwts(b) {
            found.push((t, "resp.body".to_string()));
        }
    }
    found
}

/// Extract decodable JWT substrings from free text (tokenize on non-JWT chars).
fn scan_jwts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cand in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')) {
        if cand.matches('.').count() == 2 && cand.len() >= 20 && decode_jwt(cand).is_some() {
            out.push(cand.to_string());
        }
    }
    out
}

/// Render JWTs as deterministic terminal text.
pub fn render_jwt_text(r: &JwtResult) -> String {
    let mut out = String::new();
    out.push_str("== wiretrail jwt ==\n");
    for t in &r.tokens {
        let exp = match t.summary.expired {
            Some(true) => " [EXPIRED]",
            _ => "",
        };
        out.push_str(&format!(
            "\n{} ({}x, {}){}\n",
            t.token_hash, t.occurrences, t.source, exp
        ));
        if let Some(iss) = &t.summary.iss {
            out.push_str(&format!("  iss: {iss}\n"));
        }
        if let Some(aud) = &t.summary.aud {
            out.push_str(&format!("  aud: {aud}\n"));
        }
        if let Some(sub) = &t.summary.sub_hash {
            out.push_str(&format!("  sub (hashed): {sub}\n"));
        }
        if let Some(exp) = t.summary.exp {
            out.push_str(&format!(
                "  exp: {} ({})\n",
                exp,
                match t.summary.seconds_to_expiry {
                    Some(s) if s < 0 => format!("expired {}s ago", -s),
                    Some(s) => format!("{s}s left"),
                    None => "unknown".to_string(),
                }
            ));
        }
        if let Some(hint) = &t.summary.clock_skew_hint {
            out.push_str(&format!("  warning: {hint}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::compute_jwt;
    use crate::filter::Filter;
    use crate::model::{sample_capture, sample_entry, Entry};

    const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    fn with_bearer(index: usize) -> Entry {
        let mut e = sample_entry(index, "api.x", "GET", "/me", 200);
        e.req_headers = vec![("Authorization".to_string(), format!("Bearer {SAMPLE}"))];
        e
    }

    #[test]
    fn finds_and_decodes_bearer_jwt() {
        let cap = sample_capture(vec![with_bearer(0), with_bearer(1)]);
        let r = compute_jwt(&cap, &Filter::parse(&[]).unwrap(), 10, false);
        assert_eq!(r.tokens.len(), 1);
        let t = &r.tokens[0];
        assert_eq!(t.occurrences, 2);
        assert_eq!(t.source, "req.header.authorization");
        assert_eq!(t.summary.iat, Some(1516239022));
        assert!(t.raw_token.is_none()); // redacted by default
    }

    #[test]
    fn unsafe_includes_raw_token() {
        let cap = sample_capture(vec![with_bearer(0)]);
        let r = compute_jwt(&cap, &Filter::parse(&[]).unwrap(), 10, true);
        assert_eq!(r.tokens[0].raw_token.as_deref(), Some(SAMPLE));
    }

    #[test]
    fn finds_jwt_in_body() {
        let mut e = sample_entry(0, "api.x", "POST", "/login", 200);
        e.resp_body = Some(format!(r#"{{"access_token":"{SAMPLE}"}}"#));
        let r = compute_jwt(&sample_capture(vec![e]), &Filter::parse(&[]).unwrap(), 10, false);
        assert_eq!(r.tokens.len(), 1);
        assert_eq!(r.tokens[0].source, "resp.body");
    }
}
