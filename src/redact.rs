pub const REDACTED: &str = "<redacted>";

const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "set-cookie",
    "proxy-authorization",
    "x-api-key",
    "x-auth-token",
    "x-amz-security-token",
];

const SENSITIVE_QUERY_KEYS: &[&str] = &[
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "key",
    "api_key",
    "apikey",
    "sig",
    "signature",
    "password",
    "secret",
];

pub fn redact_header_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_HEADERS.iter().any(|h| *h == lname) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

pub fn redact_query_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_QUERY_KEYS.iter().any(|k| *k == lname) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

const SENSITIVE_BODY_KEYS: &[&str] = &[
    "password",
    "token",
    "secret",
    "authorization",
    "access_token",
    "refresh_token",
    "id_token",
    "api_key",
    "apikey",
    "client_secret",
];

/// Redact and truncate a request/response body for safe display. JSON bodies
/// have sensitive keys recursively replaced; non-JSON bodies are truncated as-is.
/// `max` bounds the character count of the returned string.
pub fn redact_body(body: &str, unsafe_include: bool, max: usize) -> String {
    if unsafe_include {
        return truncate(body, max);
    }
    if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) {
        redact_json(&mut v);
        let s = serde_json::to_string(&v).unwrap_or_default();
        return truncate(&s, max);
    }
    truncate(body, max)
}

fn redact_json(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                let lk = k.to_ascii_lowercase();
                if SENSITIVE_BODY_KEYS.iter().any(|s| lk.contains(s)) {
                    *val = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redact_json(val);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for e in arr.iter_mut() {
                redact_json(e);
            }
        }
        _ => {}
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::{redact_header_value, redact_query_value};

    #[test]
    fn redacts_authorization_header() {
        assert_eq!(redact_header_value("Authorization", "Bearer abc", false), "<redacted>");
    }

    #[test]
    fn passes_through_safe_header() {
        assert_eq!(redact_header_value("Accept", "application/json", false), "application/json");
    }

    #[test]
    fn unsafe_flag_disables_redaction() {
        assert_eq!(redact_header_value("Authorization", "Bearer abc", true), "Bearer abc");
    }

    #[test]
    fn redacts_token_query_param() {
        assert_eq!(redact_query_value("access_token", "xyz", false), "<redacted>");
        assert_eq!(redact_query_value("page", "2", false), "2");
    }

    #[test]
    fn redacts_sensitive_json_keys() {
        let body = r#"{"user":"bob","access_token":"abc","nested":{"password":"x"}}"#;
        let out = super::redact_body(body, false, 1000);
        assert!(out.contains("bob"));
        assert!(!out.contains("abc"));
        assert!(!out.contains("\"x\""));
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn unsafe_body_passthrough() {
        let body = r#"{"access_token":"abc"}"#;
        let out = super::redact_body(body, true, 1000);
        assert!(out.contains("abc"));
    }

    #[test]
    fn truncates_long_body() {
        let body = "x".repeat(500);
        let out = super::redact_body(&body, false, 10);
        assert!(out.chars().count() <= 11); // 10 + ellipsis
    }
}
