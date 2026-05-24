use crate::model::Phases;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PhaseBreakdown {
    pub blocked: Option<f64>,
    pub dns: Option<f64>,
    pub connect: Option<f64>,
    pub ssl: Option<f64>,
    pub send: f64,
    pub wait: f64,
    pub receive: f64,
}

impl PhaseBreakdown {
    pub fn from_phases(p: &Phases) -> Self {
        PhaseBreakdown {
            blocked: p.blocked,
            dns: p.dns,
            connect: p.connect,
            ssl: p.ssl,
            send: p.send,
            wait: p.wait,
            receive: p.receive,
        }
    }
}

/// Label the dominant timing phase, or "unknown" when nothing is positive.
pub fn classify_bottleneck(p: &Phases) -> &'static str {
    let candidates = [
        ("queueing/blocked", p.blocked.unwrap_or(0.0)),
        ("DNS", p.dns.unwrap_or(0.0)),
        ("TCP connect", p.connect.unwrap_or(0.0)),
        ("TLS handshake", p.ssl.unwrap_or(0.0)),
        ("request upload", p.send),
        ("server wait/TTFB", p.wait),
        ("download/receive", p.receive),
    ];
    let mut best: (&'static str, f64) = ("unknown", 0.0);
    for (label, v) in candidates {
        if v > best.1 {
            best = (label, v);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::{PhaseBreakdown, classify_bottleneck};
    use crate::model::Phases;

    #[test]
    fn picks_dominant_phase() {
        let p = Phases {
            wait: 500.0,
            receive: 10.0,
            send: 1.0,
            ..Phases::default()
        };
        assert_eq!(classify_bottleneck(&p), "server wait/TTFB");
    }

    #[test]
    fn dns_dominant() {
        let p = Phases {
            dns: Some(300.0),
            wait: 5.0,
            ..Phases::default()
        };
        assert_eq!(classify_bottleneck(&p), "DNS");
    }

    #[test]
    fn all_zero_is_unknown() {
        assert_eq!(classify_bottleneck(&Phases::default()), "unknown");
    }

    #[test]
    fn breakdown_copies_phases() {
        let p = Phases {
            dns: Some(3.0),
            wait: 9.0,
            ..Phases::default()
        };
        let b = PhaseBreakdown::from_phases(&p);
        assert_eq!(b.dns, Some(3.0));
        assert_eq!(b.wait, 9.0);
    }
}
