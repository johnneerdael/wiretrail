/// (domain suffix, friendly vendor name). Matched as a dotted-label suffix so
/// `notgithub.com` does not match `github.com`.
const VENDORS: &[(&str, &str)] = &[
    ("github.com", "GitHub"),
    ("githubusercontent.com", "GitHub"),
    ("supabase.co", "Supabase"),
    ("googleapis.com", "Google"),
    ("youtube.com", "Google"),
    ("googlevideo.com", "Google"),
    ("google.com", "Google"),
    ("gstatic.com", "Google"),
    ("themoviedb.org", "TMDB"),
    ("tmdb.org", "TMDB"),
    ("cloudflare.com", "Cloudflare"),
    ("amazonaws.com", "AWS"),
    ("sentry.io", "Sentry"),
];

/// Map a host to a known vendor name, or None when unrecognized.
pub fn vendor_for(host: &str) -> Option<&'static str> {
    if host.is_empty() {
        return None;
    }
    for (suffix, name) in VENDORS {
        if host == *suffix || host.ends_with(&format!(".{suffix}")) {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::vendor_for;

    #[test]
    fn known_vendors() {
        assert_eq!(vendor_for("api.github.com"), Some("GitHub"));
        assert_eq!(vendor_for("raw.githubusercontent.com"), Some("GitHub"));
        assert_eq!(
            vendor_for("yjyuomfgkqwmjvnoxurn.supabase.co"),
            Some("Supabase")
        );
        assert_eq!(vendor_for("youtubei.googleapis.com"), Some("Google"));
        assert_eq!(vendor_for("api.themoviedb.org"), Some("TMDB"));
    }

    #[test]
    fn unknown_host_is_none() {
        assert_eq!(vendor_for("torii.nexioapp.org"), None);
        assert_eq!(vendor_for(""), None);
    }

    #[test]
    fn matches_on_suffix_not_substring() {
        // "notgithub.com" must NOT match "github.com"
        assert_eq!(vendor_for("notgithub.com"), None);
    }
}
