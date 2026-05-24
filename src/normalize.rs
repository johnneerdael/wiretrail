/// Collapse identifier-like path segments into `{id}` so routes group together.
pub fn normalize_path(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, seg) in path.split('/').enumerate() {
        if is_id_segment(seg, i) {
            parts.push("{id}".to_string());
        } else {
            parts.push(seg.to_string());
        }
    }
    parts.join("/")
}

fn is_id_segment(seg: &str, index: usize) -> bool {
    if seg.is_empty() {
        return false;
    }
    // Pure numeric: treat as id unless it is a single leading digit
    // (keeps versioned roots like `/3/tv/popular`).
    if seg.bytes().all(|b| b.is_ascii_digit()) {
        return !(index == 1 && seg.len() == 1);
    }
    if is_uuid(seg) {
        return true;
    }
    if is_long_hex(seg) {
        return true;
    }
    if is_base64ish(seg) {
        return true;
    }
    false
}

fn is_uuid(s: &str) -> bool {
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

fn is_long_hex(s: &str) -> bool {
    s.len() >= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_base64ish(s: &str) -> bool {
    if s.len() < 20 {
        return false;
    }
    let valid = s
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    let has_upper = s.bytes().any(|b| b.is_ascii_uppercase());
    valid && has_digit && has_upper
}

#[cfg(test)]
mod tests {
    use super::normalize_path;

    #[test]
    fn collapses_numeric_ids() {
        assert_eq!(normalize_path("/users/123/orders/456"), "/users/{id}/orders/{id}");
    }

    #[test]
    fn collapses_uuid() {
        assert_eq!(
            normalize_path("/v1/items/550e8400-e29b-41d4-a716-446655440000"),
            "/v1/items/{id}"
        );
    }

    #[test]
    fn collapses_long_hex() {
        assert_eq!(normalize_path("/blob/0123456789abcdef0123"), "/blob/{id}");
    }

    #[test]
    fn keeps_normal_words() {
        assert_eq!(normalize_path("/3/tv/popular"), "/3/tv/popular");
    }

    #[test]
    fn preserves_leading_and_trailing_slashes() {
        assert_eq!(normalize_path("/a/123/"), "/a/{id}/");
    }
}
