use crate::opaque::{is_opaque, is_uuid};

/// Collapse identifier-like path segments into `{id}` and opaque blobs into
/// `{blob}` so routes group together and secret-bearing config blobs are hidden.
pub fn normalize_path(path: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, seg) in path.split('/').enumerate() {
        match segment_token(seg, i) {
            Some(tok) => parts.push(tok.to_string()),
            None => parts.push(seg.to_string()),
        }
    }
    parts.join("/")
}

fn segment_token(seg: &str, index: usize) -> Option<&'static str> {
    if seg.is_empty() {
        return None;
    }
    // Pure numeric: id unless a single leading digit (keeps `/3/tv/popular`).
    if seg.bytes().all(|b| b.is_ascii_digit()) {
        return if index == 1 && seg.len() == 1 { None } else { Some("{id}") };
    }
    if is_uuid(seg) {
        return Some("{id}");
    }
    if is_long_hex(seg) {
        return Some("{id}");
    }
    if is_opaque(seg) {
        return Some("{blob}");
    }
    None
}

fn is_long_hex(s: &str) -> bool {
    s.len() >= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
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

    #[test]
    fn collapses_opaque_blob_to_blob_token() {
        assert_eq!(
            normalize_path("/cfg/eyJtYXhUb3JyZW50cyI6OCwiZGVicmlkIjp0cnVlfQ==/manifest.json"),
            "/cfg/{blob}/manifest.json"
        );
    }

    #[test]
    fn collapses_percent_encoded_blob() {
        assert_eq!(
            normalize_path("/%7B%22NexioTorii%22%3A%22eyJ1c2VFbmdsaXNo%22%7D/manifest.json"),
            "/{blob}/manifest.json"
        );
    }

    #[test]
    fn numeric_id_still_uses_id_token() {
        assert_eq!(normalize_path("/users/123/orders/456"), "/users/{id}/orders/{id}");
    }
}
