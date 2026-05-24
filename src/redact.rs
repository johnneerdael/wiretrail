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
}
