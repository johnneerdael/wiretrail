use serde::Serialize;
use serde_json::Value;

/// Decode url-safe base64 (no padding required). Returns None on invalid input.
pub fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    let mut out = Vec::new();
    for c in s.bytes() {
        let v: u32 = match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'-' => 62,
            b'_' => 63,
            b'=' => break, // padding
            _ => return None,
        };
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    Some(out)
}

#[derive(Debug, Clone)]
pub struct JwtParts {
    pub header: Value,
    pub claims: Value,
}

/// Decode a `header.payload.signature` JWT into header + claims JSON.
/// The signature is ignored (never decoded or returned).
pub fn decode_jwt(token: &str) -> Option<JwtParts> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let header: Value = serde_json::from_slice(&base64url_decode(parts[0])?).ok()?;
    let claims: Value = serde_json::from_slice(&base64url_decode(parts[1])?).ok()?;
    Some(JwtParts { header, claims })
}

#[derive(Debug, Clone, Serialize)]
pub struct JwtSummary {
    pub alg: Option<String>,
    pub typ: Option<String>,
    pub iss: Option<String>,
    pub aud: Option<String>,
    pub sub_hash: Option<String>,
    pub iat: Option<i64>,
    pub nbf: Option<i64>,
    pub exp: Option<i64>,
    pub expired: Option<bool>,
    pub seconds_to_expiry: Option<i64>,
    pub clock_skew_hint: Option<String>,
}

/// Summarize a JWT's header/claims, redacting `sub` to a hash and computing
/// expiry/skew against `ref_epoch_ms` (the using request's reconstructed time).
pub fn summarize(parts: &JwtParts, ref_epoch_ms: Option<i64>) -> JwtSummary {
    let h = &parts.header;
    let c = &parts.claims;
    let get_str = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    let get_i64 = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_i64());

    let aud = c.get("aud").map(|a| match a {
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    });
    let sub_hash = c.get("sub").and_then(|s| s.as_str()).map(token_hash);
    let iat = get_i64(c, "iat");
    let nbf = get_i64(c, "nbf");
    let exp = get_i64(c, "exp");

    let (expired, seconds_to_expiry) = match (exp, ref_epoch_ms) {
        (Some(e), Some(r)) => (Some(e * 1000 < r), Some(e - r / 1000)),
        _ => (None, None),
    };
    let clock_skew_hint = match (iat, ref_epoch_ms) {
        (Some(i), Some(r)) if i * 1000 > r + 60_000 => {
            Some("token iat is in the future (clock skew?)".to_string())
        }
        _ => None,
    };

    JwtSummary {
        alg: get_str(h, "alg"),
        typ: get_str(h, "typ"),
        iss: get_str(c, "iss"),
        aud,
        sub_hash,
        iat,
        nbf,
        exp,
        expired,
        seconds_to_expiry,
        clock_skew_hint,
    }
}

/// Stable, non-reversible short hash of a token/value (for grouping + sub redaction).
pub fn token_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::{JwtParts, base64url_decode, decode_jwt, summarize, token_hash};
    use serde_json::json;

    // jwt.io default token: header {"alg":"HS256","typ":"JWT"},
    // payload {"sub":"1234567890","name":"John Doe","iat":1516239022}
    const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    #[test]
    fn base64url_decodes_header() {
        let bytes = base64url_decode("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9").unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"alg":"HS256","typ":"JWT"}"#
        );
    }

    #[test]
    fn decodes_jwt_header_and_claims() {
        let parts = decode_jwt(SAMPLE).unwrap();
        assert_eq!(parts.header.get("alg").unwrap(), "HS256");
        assert_eq!(
            parts.claims.get("iat").unwrap().as_i64().unwrap(),
            1516239022
        );
    }

    #[test]
    fn rejects_non_jwt() {
        assert!(decode_jwt("not.a.jwt").is_none());
        assert!(decode_jwt("only.twoparts").is_none());
    }

    #[test]
    fn summary_redacts_sub_and_flags_expiry() {
        let parts = JwtParts {
            header: json!({"alg": "RS256", "typ": "JWT"}),
            claims: json!({"iss": "acme", "sub": "secret-user", "exp": 1000, "iat": 100}),
        };
        // reference time = 2000s -> exp 1000s is in the past -> expired
        let s = summarize(&parts, Some(2_000_000));
        assert_eq!(s.iss.as_deref(), Some("acme"));
        assert_eq!(s.expired, Some(true));
        assert_eq!(s.seconds_to_expiry, Some(-1000));
        // sub is hashed, never raw
        assert!(s.sub_hash.is_some());
        assert_ne!(s.sub_hash.as_deref(), Some("secret-user"));
    }

    #[test]
    fn summary_detects_future_iat_skew() {
        let parts = JwtParts {
            header: json!({"alg": "HS256"}),
            claims: json!({"iat": 5000}),
        };
        // reference time = 1000s; iat 5000s is far in the future
        let s = summarize(&parts, Some(1_000_000));
        assert!(s.clock_skew_hint.is_some());
    }

    #[test]
    fn token_hash_is_stable_and_not_raw() {
        let h = token_hash(SAMPLE);
        assert_eq!(h, token_hash(SAMPLE));
        assert!(!h.contains("eyJ"));
    }
}
