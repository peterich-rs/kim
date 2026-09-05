use std::time::Duration;

pub(crate) fn next_backoff(current: Duration) -> Duration {
    current.saturating_mul(2).min(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_caps_at_60s() {
        let mut d = Duration::from_secs(1);
        for _ in 0..20 {
            d = next_backoff(d);
            assert!(d <= Duration::from_secs(60));
        }
        assert_eq!(d, Duration::from_secs(60));
    }
}
