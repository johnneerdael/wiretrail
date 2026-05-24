use crate::glob::glob_match;
use crate::model::Entry;
use crate::vendor::vendor_for;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub ownership: Vec<OwnershipRule>,
    #[serde(default)]
    pub required_headers: Vec<RequiredHeaderRule>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequiredHeaderRule {
    /// Host glob the rule applies to.
    pub host: String,
    /// Header names that must be present on matching requests.
    #[serde(default)]
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Rule {
    /// Human-readable rule name (shown in findings).
    pub name: String,
    /// Host glob the rule applies to (None = any host).
    #[serde(default)]
    pub host: Option<String>,
    /// Path glob the rule applies to (None = any path).
    #[serde(default)]
    pub path: Option<String>,
    /// HTTP method glob (None = any method).
    #[serde(default)]
    pub method: Option<String>,
    /// Status glob, matched against the stringified status (e.g. "2*", "404").
    #[serde(default)]
    pub status: Option<String>,
    /// Header names that must be present on matching requests.
    #[serde(default)]
    pub require_headers: Vec<String>,
    /// Maximum allowed request duration in milliseconds.
    #[serde(default)]
    pub max_latency_ms: Option<f64>,
    /// If true, any matching request is itself a violation.
    #[serde(default)]
    pub forbid: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OwnershipRule {
    pub name: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub criticality: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Subsystem {
    pub name: String,
    pub owner: Option<String>,
    pub criticality: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file")]
    Io(#[source] std::io::Error),
    #[error("failed to parse config YAML")]
    Parse(#[source] yaml_serde::Error),
}

impl Config {
    /// Load config from an explicit path, or discover `wiretrail.yaml` in the
    /// current directory. A missing default file yields an empty config.
    pub fn load(explicit: Option<&Path>) -> Result<Config, ConfigError> {
        match explicit {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(ConfigError::Io)?;
                Config::from_yaml_str(&text)
            }
            None => {
                let default = Path::new("wiretrail.yaml");
                if default.is_file() {
                    let text = std::fs::read_to_string(default).map_err(ConfigError::Io)?;
                    Config::from_yaml_str(&text)
                } else {
                    Ok(Config::default())
                }
            }
        }
    }

    pub fn from_yaml_str(s: &str) -> Result<Config, ConfigError> {
        yaml_serde::from_str(s).map_err(ConfigError::Parse)
    }

    /// Resolve an entry's subsystem: first matching ownership rule, then a
    /// built-in vendor name, then the raw host.
    pub fn subsystem_for(&self, e: &Entry) -> Subsystem {
        for rule in &self.ownership {
            if rule_matches(rule, e) {
                return Subsystem {
                    name: rule.name.clone(),
                    owner: rule.owner.clone(),
                    criticality: rule.criticality.clone(),
                };
            }
        }
        if let Some(v) = vendor_for(&e.host) {
            return Subsystem {
                name: v.to_string(),
                owner: None,
                criticality: None,
            };
        }
        let name = if e.host.is_empty() {
            "(unknown)".to_string()
        } else {
            e.host.clone()
        };
        Subsystem {
            name,
            owner: None,
            criticality: None,
        }
    }
}

fn rule_matches(rule: &OwnershipRule, e: &Entry) -> bool {
    // A rule with neither host nor path never matches (avoids accidental catch-all).
    if rule.host.is_none() && rule.path.is_none() {
        return false;
    }
    if let Some(h) = &rule.host
        && !glob_match(h, &e.host)
    {
        return false;
    }
    if let Some(p) = &rule.path
        && !glob_match(p, &e.path)
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::model::sample_entry;

    #[test]
    fn parses_ownership_rules_from_yaml() {
        let yaml = r#"
ownership:
  - name: Torii Addon
    host: "torii.*"
    owner: Addons
    criticality: high
  - name: GitHub Releases
    host: "api.github.com"
    path: "/repos/*"
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.ownership.len(), 2);
    }

    #[test]
    fn rule_match_wins_over_vendor() {
        let cfg =
            Config::from_yaml_str("ownership:\n  - name: Torii Addon\n    host: \"torii.*\"\n")
                .unwrap();
        let e = sample_entry(0, "torii.nexioapp.org", "GET", "/manifest.json", 308);
        let s = cfg.subsystem_for(&e);
        assert_eq!(s.name, "Torii Addon");
    }

    #[test]
    fn falls_back_to_vendor_then_host() {
        let cfg = Config::default();
        let gh = sample_entry(0, "api.github.com", "GET", "/x", 200);
        assert_eq!(cfg.subsystem_for(&gh).name, "GitHub");
        let unknown = sample_entry(1, "torii.nexioapp.org", "GET", "/x", 200);
        assert_eq!(cfg.subsystem_for(&unknown).name, "torii.nexioapp.org");
    }

    #[test]
    fn path_rule_requires_path_match() {
        let cfg = Config::from_yaml_str(
            "ownership:\n  - name: Repos\n    host: \"api.github.com\"\n    path: \"/repos/*\"\n",
        )
        .unwrap();
        let hit = sample_entry(0, "api.github.com", "GET", "/repos/foo/bar", 200);
        let miss = sample_entry(1, "api.github.com", "GET", "/users/foo", 200);
        assert_eq!(cfg.subsystem_for(&hit).name, "Repos");
        // miss does not match the rule -> vendor fallback
        assert_eq!(cfg.subsystem_for(&miss).name, "GitHub");
    }

    #[test]
    fn parses_required_headers() {
        let yaml = r#"
required_headers:
  - host: "api.company.com"
    headers: ["Authorization", "X-App-Version"]
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.required_headers.len(), 1);
        assert_eq!(cfg.required_headers[0].host, "api.company.com");
        assert_eq!(
            cfg.required_headers[0].headers,
            vec!["Authorization", "X-App-Version"]
        );
    }

    #[test]
    fn required_headers_defaults_empty() {
        let cfg = Config::from_yaml_str("ownership: []").unwrap();
        assert!(cfg.required_headers.is_empty());
    }

    #[test]
    fn parses_rules_from_yaml() {
        let yaml = r#"
rules:
  - name: "API needs auth"
    host: "api.*"
    require_headers: ["Authorization"]
    max_latency_ms: 2000
  - name: "no internal hosts"
    host: "*.internal"
    forbid: true
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.rules.len(), 2);
        assert_eq!(cfg.rules[0].require_headers, vec!["Authorization"]);
        assert_eq!(cfg.rules[0].max_latency_ms, Some(2000.0));
        assert!(cfg.rules[1].forbid);
    }
}
