/// Minimal glob: `*` matches any run of characters. Case-insensitive.
pub fn glob_match(pattern: &str, text: &str) -> bool {
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
    use super::glob_match;

    #[test]
    fn exact_match_no_star() {
        assert!(glob_match("api.foo.com", "API.FOO.COM"));
        assert!(!glob_match("api.foo.com", "api.bar.com"));
    }

    #[test]
    fn star_matches_substring() {
        assert!(glob_match("*login*", "/v1/login/start"));
        assert!(glob_match("api.*.com", "api.foo.com"));
        assert!(!glob_match("api.*.com", "cdn.foo.net"));
    }

    #[test]
    fn leading_and_trailing_star() {
        assert!(glob_match("*.nexioapp.org", "torii.nexioapp.org"));
        assert!(glob_match("torii.*", "torii.nexioapp.org"));
    }
}
