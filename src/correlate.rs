const CORRELATION_HEADERS: &[&str] = &[
    "x-request-id",
    "x-correlation-id",
    "traceparent",
    "x-amzn-trace-id",
    "cf-ray",
    "x-datadog-trace-id",
];

/// Pull known correlation headers (lowercased name, original value) from a
/// header list. Order follows `CORRELATION_HEADERS`.
pub fn extract_correlation(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for known in CORRELATION_HEADERS {
        if let Some((_, v)) = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(known))
        {
            out.push((known.to_string(), v.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::extract_correlation;

    #[test]
    fn extracts_known_headers_case_insensitively() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Request-Id".to_string(), "abc-123".to_string()),
            ("cf-ray".to_string(), "7d-DFW".to_string()),
        ];
        let got = extract_correlation(&headers);
        assert!(got.contains(&("x-request-id".to_string(), "abc-123".to_string())));
        assert!(got.contains(&("cf-ray".to_string(), "7d-DFW".to_string())));
        assert_eq!(got.len(), 2);
    }
}
