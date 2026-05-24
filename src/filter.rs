use crate::model::Entry;

#[derive(Debug)]
enum Cmp {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

#[derive(Debug)]
enum Clause {
    Host(String),
    Method(String),
    Path(String),
    Status(Cmp, i64),
    Time(Cmp, f64),
    Has(String),
}

#[derive(Debug)]
pub struct Filter {
    clauses: Vec<Clause>,
}

impl Filter {
    /// Parse clauses like `host:api.foo.com status:>=400 path:*login* time:>5ms`.
    pub fn parse(exprs: &[String]) -> Result<Filter, String> {
        let mut clauses = Vec::new();
        for raw in exprs {
            for token in raw.split_whitespace() {
                clauses.push(parse_clause(token)?);
            }
        }
        Ok(Filter { clauses })
    }

    pub fn matches(&self, e: &Entry) -> bool {
        self.clauses.iter().all(|c| clause_matches(c, e))
    }
}

fn parse_clause(token: &str) -> Result<Clause, String> {
    let (key, val) = token
        .split_once(':')
        .ok_or_else(|| format!("invalid filter clause: {token}"))?;
    match key {
        "host" => Ok(Clause::Host(val.to_string())),
        "method" => Ok(Clause::Method(val.to_ascii_uppercase())),
        "path" => Ok(Clause::Path(val.to_string())),
        "status" => {
            let (cmp, n) = parse_cmp_int(val)?;
            Ok(Clause::Status(cmp, n))
        }
        "time" => {
            let v = val.trim_end_matches("ms");
            let (cmp, n) = parse_cmp_float(v)?;
            Ok(Clause::Time(cmp, n))
        }
        "has" => Ok(Clause::Has(val.to_ascii_lowercase())),
        other => Err(format!("unknown filter key: {other}")),
    }
}

fn parse_cmp_int(s: &str) -> Result<(Cmp, i64), String> {
    let (cmp, rest) = split_cmp(s);
    let n = rest.parse::<i64>().map_err(|_| format!("invalid number: {rest}"))?;
    Ok((cmp, n))
}

fn parse_cmp_float(s: &str) -> Result<(Cmp, f64), String> {
    let (cmp, rest) = split_cmp(s);
    let n = rest.parse::<f64>().map_err(|_| format!("invalid number: {rest}"))?;
    Ok((cmp, n))
}

fn split_cmp(s: &str) -> (Cmp, &str) {
    if let Some(rest) = s.strip_prefix(">=") {
        (Cmp::Ge, rest)
    } else if let Some(rest) = s.strip_prefix("<=") {
        (Cmp::Le, rest)
    } else if let Some(rest) = s.strip_prefix('>') {
        (Cmp::Gt, rest)
    } else if let Some(rest) = s.strip_prefix('<') {
        (Cmp::Lt, rest)
    } else if let Some(rest) = s.strip_prefix('=') {
        (Cmp::Eq, rest)
    } else {
        (Cmp::Eq, s)
    }
}

fn cmp_i(cmp: &Cmp, a: i64, b: i64) -> bool {
    match cmp {
        Cmp::Ge => a >= b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Lt => a < b,
        Cmp::Eq => a == b,
    }
}

fn cmp_f(cmp: &Cmp, a: f64, b: f64) -> bool {
    match cmp {
        Cmp::Ge => a >= b,
        Cmp::Le => a <= b,
        Cmp::Gt => a > b,
        Cmp::Lt => a < b,
        Cmp::Eq => a == b,
    }
}

fn clause_matches(c: &Clause, e: &Entry) -> bool {
    match c {
        Clause::Host(h) => glob_match(h, &e.host),
        Clause::Method(m) => e.method.eq_ignore_ascii_case(m),
        Clause::Path(p) => glob_match(p, &e.path),
        Clause::Status(cmp, n) => cmp_i(cmp, e.status, *n),
        Clause::Time(cmp, n) => cmp_f(cmp, e.duration_ms, *n),
        Clause::Has(field) => has_field(field, e),
    }
}

fn has_field(field: &str, e: &Entry) -> bool {
    // Supported forms: req.header.<name>, resp.header.<name>, req.body, resp.body
    if let Some(name) = field.strip_prefix("req.header.") {
        return e.req_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    }
    if let Some(name) = field.strip_prefix("resp.header.") {
        return e.resp_headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
    }
    match field {
        "req.body" => e.req_body.as_ref().is_some_and(|b| !b.is_empty()),
        "resp.body" => e.resp_body.as_ref().is_some_and(|b| !b.is_empty()),
        _ => false,
    }
}

/// Minimal glob: `*` matches any run of characters. Case-insensitive.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.to_ascii_lowercase();
    let t = text.to_ascii_lowercase();
    if !p.contains('*') {
        return p == t;
    }
    let parts: Vec<&str> = p.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !t[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            return t[pos..].ends_with(part);
        } else if let Some(found) = t[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::Filter;
    use crate::classify::ResourceType;
    use crate::model::{Entry, Phases, Sizes};

    fn entry(host: &str, status: i64, method: &str, path: &str, dur: f64) -> Entry {
        Entry {
            id: "e000000".into(), index: 0, started_offset_ms: 0.0, duration_ms: dur,
            method: method.into(), url: format!("https://{host}{path}"), host: host.into(),
            path: path.into(), norm_path: path.into(), query: vec![], status,
            status_text: String::new(), resource_type: ResourceType::Api, content_type: None,
            req_headers: vec![("authorization".into(), "x".into())], resp_headers: vec![],
            req_body: None, resp_body: None, timings: Phases::default(), sizes: Sizes::default(),
            server_ip: None, http_version: "HTTP/2".into(), redirect_url: None, correlation: vec![],
        }
    }

    #[test]
    fn matches_host_and_status() {
        let f = Filter::parse(&["host:api.foo.com".into(), "status:>=400".into()]).unwrap();
        assert!(f.matches(&entry("api.foo.com", 500, "GET", "/x", 10.0)));
        assert!(!f.matches(&entry("api.foo.com", 200, "GET", "/x", 10.0)));
        assert!(!f.matches(&entry("other.com", 500, "GET", "/x", 10.0)));
    }

    #[test]
    fn matches_method_and_path_glob_and_time() {
        let f = Filter::parse(&["method:POST".into(), "path:*login*".into(), "time:>5ms".into()]).unwrap();
        assert!(f.matches(&entry("h", 200, "POST", "/v1/login/start", 10.0)));
        assert!(!f.matches(&entry("h", 200, "POST", "/v1/login/start", 1.0)));
        assert!(!f.matches(&entry("h", 200, "GET", "/v1/login/start", 10.0)));
    }

    #[test]
    fn matches_has_header() {
        let f = Filter::parse(&["has:req.header.authorization".into()]).unwrap();
        assert!(f.matches(&entry("h", 200, "GET", "/x", 1.0)));
    }

    #[test]
    fn empty_filter_matches_all() {
        let f = Filter::parse(&[]).unwrap();
        assert!(f.matches(&entry("h", 200, "GET", "/x", 1.0)));
    }
}
