use crate::opaque::is_opaque;

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

const URL_VALUED_HEADERS: &[&str] = &["location", "referer", "content-location"];

const VALUE_DELIMS: &[char] = &[
    ' ', '\t', '\n', '\r', ';', ',', '&', '=', '/', '?', '"', '{', '}', '[', ']', ':',
];

pub fn redact_header_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_HEADERS.iter().any(|h| *h == lname) {
        return REDACTED.to_string();
    }
    if URL_VALUED_HEADERS.iter().any(|h| *h == lname) {
        return redact_url(value, false);
    }
    redact_value(value, false)
}

pub fn redact_query_value(name: &str, value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let lname = name.to_ascii_lowercase();
    if SENSITIVE_QUERY_KEYS.iter().any(|k| *k == lname) || is_opaque(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

/// Redact secret-bearing chunks from a free-form value: split on common
/// delimiters and replace any opaque chunk with the redaction marker.
pub fn redact_value(value: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut chunk = String::new();
    for ch in value.chars() {
        if VALUE_DELIMS.contains(&ch) {
            flush_chunk(&mut out, &mut chunk);
            out.push(ch);
        } else {
            chunk.push(ch);
        }
    }
    flush_chunk(&mut out, &mut chunk);
    out
}

fn flush_chunk(out: &mut String, chunk: &mut String) {
    if chunk.is_empty() {
        return;
    }
    if is_opaque(chunk) {
        out.push_str(REDACTED);
    } else {
        out.push_str(chunk);
    }
    chunk.clear();
}

/// Rebuild a URL with opaque path segments and sensitive/opaque query values
/// redacted. `unsafe_include` returns the raw URL. Falls back to `redact_value`
/// on parse failure.
pub fn redact_url(url: &str, unsafe_include: bool) -> String {
    if unsafe_include {
        return url.to_string();
    }
    let Ok(u) = url::Url::parse(url) else {
        return redact_value(url, false);
    };

    let path: String = u
        .path()
        .split('/')
        .map(|seg| if is_opaque(seg) { REDACTED } else { seg })
        .collect::<Vec<_>>()
        .join("/");

    let pairs: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| {
            let rv = redact_query_value(k.as_ref(), v.as_ref(), false);
            (k.into_owned(), rv)
        })
        .collect();

    let mut out = String::new();
    out.push_str(u.scheme());
    out.push_str("://");
    if let Some(host) = u.host_str() {
        out.push_str(host);
    }
    if let Some(port) = u.port() {
        out.push_str(&format!(":{port}"));
    }
    out.push_str(&path);
    if !pairs.is_empty() {
        out.push('?');
        let q: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        out.push_str(&q.join("&"));
    }
    out
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

/// Redact and truncate a request/response body for safe single-line display.
/// JSON bodies have sensitive keys recursively replaced; newlines/tabs are
/// collapsed to spaces so snippets stay on one line. `max` bounds the char count.
pub fn redact_body(body: &str, unsafe_include: bool, max: usize) -> String {
    let scrubbed = if unsafe_include {
        body.to_string()
    } else if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(body) {
        redact_json(&mut v);
        serde_json::to_string(&v).unwrap_or_default()
    } else {
        body.to_string()
    };
    truncate(&collapse_newlines(&scrubbed), max)
}

fn collapse_newlines(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect()
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
        assert_eq!(
            redact_header_value("Authorization", "Bearer abc", false),
            "<redacted>"
        );
    }

    #[test]
    fn passes_through_safe_header() {
        assert_eq!(
            redact_header_value("Accept", "application/json", false),
            "application/json"
        );
    }

    #[test]
    fn unsafe_flag_disables_redaction() {
        assert_eq!(
            redact_header_value("Authorization", "Bearer abc", true),
            "Bearer abc"
        );
    }

    #[test]
    fn redacts_token_query_param() {
        assert_eq!(
            redact_query_value("access_token", "xyz", false),
            "<redacted>"
        );
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

    #[test]
    fn redact_url_masks_opaque_path_keeps_numeric() {
        let url = "https://h.example.com/cfg/eyJrZXkiOiJzZWNyZXQiLCJuIjoxMjN9==/users/123";
        let out = super::redact_url(url, false);
        assert!(out.contains("/cfg/<redacted>/users/123"));
        assert!(!out.contains("eyJrZXki"));
    }

    #[test]
    fn redact_url_masks_opaque_query_keeps_safe() {
        let url = "https://h.example.com/x?token=eyJhbGciOiJIUzI1NiJ9abc123XYZ&page=2";
        let out = super::redact_url(url, false);
        assert!(out.contains("page=2"));
        assert!(out.contains("token=<redacted>"));
    }

    #[test]
    fn redact_url_unsafe_is_raw() {
        let url = "https://h.example.com/cfg/eyJrZXkiOiJzZWNyZXQiLCJuIjoxMjN9==/x";
        assert_eq!(super::redact_url(url, true), url);
    }

    #[test]
    fn header_location_value_is_url_redacted() {
        let v = "https://h.example.com/%7B%22k%22%3A%22eyJzZWNyZXQiOnRydWV9%22%7D/manifest.json";
        let out = super::redact_header_value("Location", v, false);
        assert!(out.contains("<redacted>"));
        assert!(!out.contains("eyJzZWNyZXQi"));
    }

    #[test]
    fn header_value_opaque_substring_redacted() {
        let v = "report-to; s=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abcXYZ123";
        let out = super::redact_header_value("Report-To", v, false);
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn header_accept_untouched() {
        assert_eq!(
            super::redact_header_value("Accept", "application/json", false),
            "application/json"
        );
    }

    #[test]
    fn query_value_redacted_when_opaque() {
        // benign name, but opaque base64url value (>=32, mixed case + digit) -> redacted
        assert_eq!(
            super::redact_query_value("d", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", false),
            "<redacted>"
        );
    }

    #[test]
    fn body_snippet_is_single_line() {
        let body = "line one\nline two\tindented\r\nline three";
        let out = super::redact_body(body, false, 1000);
        assert!(!out.contains('\n'));
        assert!(!out.contains('\t'));
        assert!(!out.contains('\r'));
    }
}
