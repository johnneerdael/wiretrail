#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

/// Nearest-rank percentiles over a set of values. Deterministic; does not mutate input.
pub fn percentiles(values: &[f64]) -> Percentiles {
    if values.is_empty() {
        return Percentiles {
            p50: 0.0,
            p95: 0.0,
            max: 0.0,
        };
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    let pick = |p: f64| -> f64 {
        let rank = ((p / 100.0) * n as f64).ceil() as usize;
        let idx = rank.saturating_sub(1).min(n - 1);
        v[idx]
    };
    Percentiles {
        p50: pick(50.0),
        p95: pick(95.0),
        max: *v.last().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::percentiles;

    #[test]
    fn empty_is_zero() {
        let p = percentiles(&[]);
        assert_eq!(p.p50, 0.0);
        assert_eq!(p.p95, 0.0);
        assert_eq!(p.max, 0.0);
    }

    #[test]
    fn single_value() {
        let p = percentiles(&[42.0]);
        assert_eq!(p.p50, 42.0);
        assert_eq!(p.p95, 42.0);
        assert_eq!(p.max, 42.0);
    }

    #[test]
    fn nearest_rank_five_values() {
        // sorted: 10,20,30,40,50 ; p50 -> rank ceil(2.5)=3 -> 30 ; p95 -> rank ceil(4.75)=5 -> 50
        let p = percentiles(&[50.0, 10.0, 40.0, 20.0, 30.0]);
        assert_eq!(p.p50, 30.0);
        assert_eq!(p.p95, 50.0);
        assert_eq!(p.max, 50.0);
    }
}
