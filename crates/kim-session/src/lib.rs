//! Session storage: in-memory default and optional Redis adapter (`--features redis`).
//!
//! Tests construct [`MemorySessionStore::new`] or [`open_session_store`]`(None)` and
//! never read `REDIS_URL`. `env -u REDIS_URL cargo test --workspace` does not need a
//! Redis daemon.

mod keys;
mod memory;
#[cfg(feature = "redis")]
mod redis;

use std::sync::Arc;
use std::time::Duration;

use kim_router::{SessionError, SessionStorage};

pub use keys::{key_location, key_session};
pub use memory::MemorySessionStore;

/// Session and location key TTL used on the Redis path (`SET EX`).
pub const SESSION_TTL: Duration = Duration::from_secs(48 * 3600);

/// Open a session store.
///
/// * `None` / `Some("")` → [`MemorySessionStore`].
/// * Non-empty URL without `--features redis` → `Err("rebuild with --features redis")`.
/// * Non-empty URL with the feature → Redis; invalid URL is `Err`, never silent Memory.
pub async fn open_session_store(
    redis_url: Option<&str>,
) -> Result<Arc<dyn SessionStorage>, SessionError> {
    match redis_url {
        None | Some("") => Ok(Arc::new(MemorySessionStore::new())),
        Some(url) => open_redis_store(url).await,
    }
}

#[cfg(feature = "redis")]
async fn open_redis_store(url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    let store = redis::RedisSessionStore::open(url).await?;
    Ok(Arc::new(store))
}

#[cfg(not(feature = "redis"))]
async fn open_redis_store(_url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    Err(SessionError::Other("rebuild with --features redis".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::pkt::Session;
    use kim_router::SessionError;

    fn session(channel_id: &str, account: &str, gate_id: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: gate_id.into(),
            account: account.into(),
            ..Session::default()
        }
    }

    #[test]
    fn session_ttl_is_48h() {
        assert_eq!(SESSION_TTL, Duration::from_secs(48 * 3600));
    }

    #[tokio::test]
    async fn open_none_is_memory() {
        let store = open_session_store(None).await.unwrap();
        store.add(&session("id1", "alice", "wg-1")).await.unwrap();
        store.add(&session("id2", "alice", "wg-1")).await.unwrap();
        store.delete("alice", "id1").await.unwrap();

        let loc = store.get_location("alice", "").await.unwrap();
        assert_eq!(loc.channel_id, "id2");
        assert!(matches!(
            store.get("id1").await,
            Err(SessionError::NotFound)
        ));
        let s2 = store.get("id2").await.unwrap();
        assert_eq!(s2.channel_id, "id2");
    }

    #[tokio::test]
    async fn open_empty_string_is_memory() {
        let store = open_session_store(Some("")).await.unwrap();
        store.add(&session("id1", "bob", "g")).await.unwrap();
        assert_eq!(store.get("id1").await.unwrap().account, "bob");
    }

    #[cfg(not(feature = "redis"))]
    #[tokio::test]
    async fn non_empty_url_requires_redis_feature() {
        match open_session_store(Some("redis://127.0.0.1:6379")).await {
            Err(e) => assert_eq!(e.to_string(), "rebuild with --features redis"),
            Ok(_) => panic!("expected rebuild-with-features error"),
        }
    }
}
