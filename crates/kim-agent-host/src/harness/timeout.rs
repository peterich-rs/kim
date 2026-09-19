use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Sleep that stays pending for multi-day durations so `Duration::MAX` does not overflow `Instant`.
pub async fn sleep_for(d: Duration) {
    const CAP: Duration = Duration::from_secs(24 * 60 * 60);
    if d >= CAP {
        std::future::pending::<()>().await;
    } else if d.is_zero() {
        return;
    } else {
        tokio::time::sleep(d).await;
    }
}

/// Silent-inference watchdog. Only [`IdleClock::reset`] restarts it.
pub struct IdleClock {
    limit: Duration,
    last: Mutex<Instant>,
    /// Set only by [`IdleClock::reset`], never by keepalive.
    activity: Mutex<Option<Instant>>,
}

impl IdleClock {
    pub fn new(limit: Duration) -> Arc<Self> {
        Arc::new(Self {
            limit,
            last: Mutex::new(Instant::now()),
            activity: Mutex::new(None),
        })
    }

    pub fn reset(&self) {
        let now = Instant::now();
        *self.last.lock().unwrap_or_else(|e| e.into_inner()) = now;
        *self.activity.lock().unwrap_or_else(|e| e.into_inner()) = Some(now);
    }

    pub fn recently_active(&self, window: Duration) -> bool {
        self.activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some_and(|at| at.elapsed() <= window)
    }

    pub async fn sleep(&self) {
        loop {
            let wait = {
                let last = self.last.lock().unwrap_or_else(|e| e.into_inner());
                self.limit.saturating_sub(last.elapsed())
            };
            if wait.is_zero() {
                return;
            }
            sleep_for(wait).await;
            let elapsed = self
                .last
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .elapsed();
            if elapsed >= self.limit {
                return;
            }
        }
    }
}

pub struct HardDeadline {
    remaining: Duration,
}

impl HardDeadline {
    pub fn from_remaining(remaining: Duration) -> Self {
        Self { remaining }
    }

    pub async fn sleep(&self) {
        sleep_for(self.remaining).await;
    }
}

/// Pure yield-wait helper. FFI owns the real timer; this is the testable clock.
pub struct YieldWatch {
    limit: Duration,
}

impl YieldWatch {
    pub fn new(limit: Duration) -> Self {
        Self { limit }
    }

    pub async fn fired(&self) {
        sleep_for(self.limit).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn yield_watch_fires_after_limit() {
        let watch = YieldWatch::new(Duration::from_millis(40));
        let started = Instant::now();
        tokio::time::timeout(Duration::from_secs(2), watch.fired())
            .await
            .expect("yield watch");
        assert!(started.elapsed() >= Duration::from_millis(30));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn idle_sleep_respects_reset() {
        let clock = IdleClock::new(Duration::from_millis(80));
        let started = Instant::now();
        tokio::spawn({
            let clock = Arc::clone(&clock);
            async move {
                tokio::time::sleep(Duration::from_millis(40)).await;
                clock.reset();
            }
        });
        clock.sleep().await;
        assert!(started.elapsed() >= Duration::from_millis(100));
    }
}
