/// True when a string chunk looks like an opaque blob (base64 / percent-encoded /
/// long high-entropy token) rather than a readable path/identifier. Hex strings
/// and UUIDs are treated as readable ids and excluded.
pub fn is_opaque(s: &str) -> bool {
    let len = s.len();
    if len < 16 {
        return false;
    }
    if s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false; // hex hash/etag -> readable id
    }
    if is_uuid(s) {
        return false; // resource id, not a secret blob
    }

    // Percent-encoded blob (URL-encoded JSON/config).
    if s.contains('%') {
        return true;
    }

    // Must be within the base64 alphabet to be considered a blob.
    let base64_alpha = s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'_' | b'-'));
    if !base64_alpha {
        return false;
    }
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let has_upper = s.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = s.bytes().any(|b| b.is_ascii_lowercase());
    let has_b64_symbol = s.bytes().any(|b| matches!(b, b'+' | b'/' | b'='));

    // Standard base64 markers never appear in slugs/ids -> strong signal.
    if len >= 24 && has_b64_symbol && has_digit {
        return true;
    }
    // Symbol-less base64url token: long, mixed case + digit (slugs are lowercase).
    if len >= 32 && has_digit && has_upper && has_lower {
        return true;
    }
    false
}

/// Canonical 8-4-4-4-12 hex UUID check.
pub fn is_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != groups.len() {
        return false;
    }
    parts
        .iter()
        .zip(groups)
        .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::is_opaque;

    #[test]
    fn flags_standard_base64_blob() {
        // jackettio-style standard base64 with padding
        assert!(is_opaque("eyJtYXhUb3JyZW50cyI6OCwiZGVicmlkIjp0cnVlfQ=="));
    }

    #[test]
    fn flags_percent_encoded_blob() {
        assert!(is_opaque("%7B%22NexioTorii%22%3A%22eyJ1c2VFbmdsaXNo%22%7D"));
    }

    #[test]
    fn flags_base64url_token() {
        // JWT-like base64url, no +/=, mixed case + digits
        assert!(is_opaque("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn does_not_flag_readable_segments() {
        assert!(!is_opaque("manifest.json"));
        assert!(!is_opaque("videoplayback"));
        assert!(!is_opaque("sync_resolve_account_secret"));
        assert!(!is_opaque("v1"));
        assert!(!is_opaque("popular"));
    }

    #[test]
    fn does_not_flag_lowercase_slug_with_digit() {
        // long, has a digit, but lowercase dashed slug -> readable, must stay
        assert!(!is_opaque("my-very-long-feature-slug-2024-edition"));
    }

    #[test]
    fn does_not_flag_hex_or_uuid() {
        assert!(!is_opaque("0123456789abcdef0123")); // hex hash
        assert!(!is_opaque("550e8400-e29b-41d4-a716-446655440000")); // uuid
    }

    #[test]
    fn does_not_flag_short_strings() {
        assert!(!is_opaque("szpwe4fx4ngs8u9q")); // 16-char token, below blob threshold
        assert!(!is_opaque("abc"));
    }
}
