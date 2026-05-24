use crate::model::CaptureMeta;
use serde::Serialize;

#[derive(Serialize)]
pub struct Envelope<T: Serialize> {
    pub tool: &'static str,
    pub schema_version: u32,
    pub command: &'static str,
    pub capture: CaptureMeta,
    pub result: T,
    pub warnings: Vec<String>,
    pub next_commands: Vec<String>,
}

impl<T: Serialize> Envelope<T> {
    pub fn new(command: &'static str, capture: CaptureMeta, result: T) -> Self {
        Envelope {
            tool: "wiretrail",
            schema_version: 1,
            command,
            capture,
            result,
            warnings: Vec::new(),
            next_commands: Vec::new(),
        }
    }

    pub fn with_next_commands(mut self, cmds: Vec<String>) -> Self {
        self.next_commands = cmds;
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExitCode {
    Clean = 0,
    Findings = 1,
    InvalidHar = 2,
    UnsafeBlocked = 3,
}

/// Human-readable byte size, e.g. 1.2 MiB.
pub fn human_bytes(n: i64) -> String {
    if n < 0 {
        return "?".to_string();
    }
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// Human-readable milliseconds, e.g. 75.4s or 312ms.
pub fn human_ms(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.1}s", ms / 1000.0)
    } else {
        format!("{ms:.0}ms")
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, ExitCode};
    use crate::model::CaptureMeta;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Dummy {
        n: u32,
    }

    fn meta() -> CaptureMeta {
        CaptureMeta {
            har_version: "1.2".into(),
            creator: "x".into(),
            creator_version: "1".into(),
            browser: None,
            entry_count: 0,
            start_ms: None,
            end_ms: None,
            duration_ms: 0.0,
        }
    }

    #[test]
    fn serializes_stable_envelope() {
        let env = Envelope::new("summary", meta(), Dummy { n: 7 });
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"tool\":\"wiretrail\""));
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"command\":\"summary\""));
        assert!(json.contains("\"n\":7"));
    }

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(ExitCode::Clean as i32, 0);
        assert_eq!(ExitCode::Findings as i32, 1);
        assert_eq!(ExitCode::InvalidHar as i32, 2);
        assert_eq!(ExitCode::UnsafeBlocked as i32, 3);
    }
}
