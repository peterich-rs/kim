use std::sync::atomic::{AtomicI64, Ordering};

/// Resolve snowflake node: env `KIM_SNOWFLAKE_NODE` (trim, parse u16) > `cfg` > `1`.
///
/// Missing or empty env does not warn. Non-numeric env warns and falls through
/// to `cfg`. A selected value `> 31` is an error (no silent fallback to 1).
pub fn resolve_snowflake_node(cfg: Option<u16>) -> Result<u16, IdError> {
    let from_env = match std::env::var("KIM_SNOWFLAKE_NODE") {
        Ok(s) if !s.trim().is_empty() => match s.trim().parse::<u16>() {
            Ok(n) => Some(n),
            Err(_) => {
                tracing::warn!(value = %s, "KIM_SNOWFLAKE_NODE not u16; ignoring");
                None
            }
        },
        _ => None,
    };
    let n = from_env.or(cfg).unwrap_or(1);
    if n > 31 {
        Err(IdError::Init(format!("snowflake node {n} out of 0..=31")))
    } else {
        Ok(n)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdError {
    #[error("snowflake init: {0}")]
    Init(String),
    #[error("snowflake next: {0}")]
    Next(String),
    #[error("snowflake id overflows i64")]
    Overflow,
}

pub trait IdGenerator: Send + Sync {
    fn next_id(&self) -> Result<i64, IdError>;
}

pub struct SnowflakeGen {
    inner: snowflake_me::Snowflake,
}

impl SnowflakeGen {
    pub fn try_new(machine_id: u16) -> Result<Self, IdError> {
        let mid = machine_id;
        let sf = snowflake_me::Snowflake::builder()
            .machine_id(&|| Ok::<u16, Box<dyn std::error::Error + Send + Sync>>(mid))
            .data_center_id(&|| Ok::<u16, Box<dyn std::error::Error + Send + Sync>>(0))
            .finalize()
            .map_err(|e| IdError::Init(e.to_string()))?;
        Ok(Self { inner: sf })
    }
}

impl IdGenerator for SnowflakeGen {
    fn next_id(&self) -> Result<i64, IdError> {
        let id = self
            .inner
            .next_id()
            .map_err(|e| IdError::Next(e.to_string()))?;
        i64::try_from(id.as_u64()).map_err(|_| IdError::Overflow)
    }
}

/// Test + snowflake init fallback. Default start is 10_001.
pub struct SequenceIdGen {
    n: AtomicI64,
}

impl SequenceIdGen {
    pub fn new(start: i64) -> Self {
        Self {
            n: AtomicI64::new(start),
        }
    }
}

impl Default for SequenceIdGen {
    fn default() -> Self {
        Self::new(10_001)
    }
}

impl IdGenerator for SequenceIdGen {
    fn next_id(&self) -> Result<i64, IdError> {
        let id = self.n.fetch_add(1, Ordering::Relaxed);
        if id < 0 {
            Err(IdError::Overflow)
        } else {
            Ok(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        prev: Option<String>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn lock() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let prev = std::env::var("KIM_SNOWFLAKE_NODE").ok();
            Self { prev, _lock: lock }
        }

        #[allow(unsafe_code)]
        fn unset(&self) {
            // SAFETY: ENV_LOCK serializes mutation of this process-global var.
            unsafe { std::env::remove_var("KIM_SNOWFLAKE_NODE") }
        }

        #[allow(unsafe_code)]
        fn set(&self, value: &str) {
            // SAFETY: ENV_LOCK serializes mutation of this process-global var.
            unsafe { std::env::set_var("KIM_SNOWFLAKE_NODE", value) }
        }
    }

    impl Drop for EnvGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            // SAFETY: ENV_LOCK is held for the guard's lifetime.
            unsafe {
                match &self.prev {
                    Some(v) => std::env::set_var("KIM_SNOWFLAKE_NODE", v),
                    None => std::env::remove_var("KIM_SNOWFLAKE_NODE"),
                }
            }
        }
    }

    #[test]
    fn sequence_ids_are_consecutive_and_gt_10000() {
        let g = SequenceIdGen::default();
        let a = g.next_id().unwrap();
        let b = g.next_id().unwrap();
        let c = g.next_id().unwrap();
        assert!(a > 10_000);
        assert_eq!(b, a + 1);
        assert_eq!(c, b + 1);
    }

    #[test]
    fn snowflake_next_id_gt_10000() {
        let id = SnowflakeGen::try_new(1).unwrap().next_id().unwrap();
        assert!(id > 10_000);
    }

    #[test]
    fn resolve_missing_env_and_none_is_one() {
        let env = EnvGuard::lock();
        env.unset();
        assert_eq!(resolve_snowflake_node(None).unwrap(), 1);
    }

    #[test]
    fn resolve_missing_env_uses_cfg() {
        let env = EnvGuard::lock();
        env.unset();
        assert_eq!(resolve_snowflake_node(Some(7)).unwrap(), 7);
    }

    #[test]
    fn resolve_cfg_above_31_falls_back_to_one() {
        let env = EnvGuard::lock();
        env.unset();
        assert!(resolve_snowflake_node(Some(99)).is_err());
    }

    #[test]
    fn resolve_non_numeric_env_falls_through_to_cfg() {
        let env = EnvGuard::lock();
        env.set("not-a-number");
        assert_eq!(resolve_snowflake_node(Some(4)).unwrap(), 4);
    }
}
