use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Api,
    Media,
    Static,
    Analytics,
    Document,
    Other,
}

const ANALYTICS_HOSTS: &[&str] = &[
    "google-analytics.com",
    "analytics.google.com",
    "doubleclick.net",
    "googletagmanager.com",
    "segment.io",
    "mixpanel.com",
    "amplitude.com",
    "sentry.io",
    "crashlytics.com",
];

/// Classify an entry by content-type, then URL extension, then host.
pub fn classify(content_type: Option<&str>, url: &str) -> ResourceType {
    let host = host_of(url);
    if ANALYTICS_HOSTS.iter().any(|h| host.ends_with(h)) {
        return ResourceType::Analytics;
    }
    if let Some(ct) = content_type {
        let ct = ct.split(';').next().unwrap_or(ct).trim().to_ascii_lowercase();
        if let Some(rt) = by_mime(&ct) {
            return rt;
        }
    }
    by_extension(url)
}

fn by_mime(ct: &str) -> Option<ResourceType> {
    if ct.contains("json") || ct.contains("graphql") || ct.contains("grpc") || ct.contains("protobuf") {
        return Some(ResourceType::Api);
    }
    if ct.contains("xml") && !ct.contains("html") {
        return Some(ResourceType::Api);
    }
    if ct.starts_with("image/") || ct.starts_with("video/") || ct.starts_with("audio/") {
        return Some(ResourceType::Media);
    }
    if ct.contains("javascript") || ct.contains("css") || ct.contains("font") || ct.contains("ecmascript") {
        return Some(ResourceType::Static);
    }
    if ct.contains("html") {
        return Some(ResourceType::Document);
    }
    None
}

fn by_extension(url: &str) -> ResourceType {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" | "mp4" | "webm" | "ts" | "m4s"
        | "mp3" | "aac" | "m3u8" => ResourceType::Media,
        "js" | "mjs" | "css" | "woff" | "woff2" | "ttf" | "otf" | "eot" => ResourceType::Static,
        "json" => ResourceType::Api,
        "html" | "htm" => ResourceType::Document,
        _ => ResourceType::Other,
    }
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{classify, ResourceType};

    #[test]
    fn json_is_api() {
        assert_eq!(classify(Some("application/json"), "https://api.x/v1/y"), ResourceType::Api);
    }

    #[test]
    fn image_is_media() {
        assert_eq!(classify(Some("image/png"), "https://x/a.png"), ResourceType::Media);
    }

    #[test]
    fn video_is_media() {
        assert_eq!(classify(Some("video/mp4"), "https://x/a.mp4"), ResourceType::Media);
    }

    #[test]
    fn javascript_is_static() {
        assert_eq!(classify(Some("application/javascript"), "https://x/a.js"), ResourceType::Static);
    }

    #[test]
    fn falls_back_to_extension() {
        assert_eq!(classify(None, "https://x/styles.css"), ResourceType::Static);
    }

    #[test]
    fn analytics_host() {
        assert_eq!(classify(Some("application/json"), "https://www.google-analytics.com/collect"), ResourceType::Analytics);
    }
}
