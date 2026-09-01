//! Session storage: in-memory default and optional Redis adapter (`--features redis`).
//!
//! Tests construct [`MemorySessionStore::new`] or [`open_session_store`]`(None)` and
//! never read `REDIS_URL`. `env -u REDIS_URL cargo test --workspace` does not need a
//! Redis daemon.

mod cache;
mod dual;
mod keys;
mod memory;
#[cfg(feature = "redis")]
mod redis;

use std::sync::Arc;
use std::time::Duration;

use kim_router::{SessionError, SessionStorage};

pub use cache::CachedSessionStore;
pub use dual::DualWriteStore;
pub use keys::{key_location, key_session};
pub use memory::MemorySessionStore;
#[cfg(feature = "redis")]
pub use redis::RedisSessionStore;

/// Mobile-class devices keep a single live session. Web/desktop/cli may overlap.
pub fn exclusive_device(device: &str) -> bool {
    matches!(
        device.trim().to_ascii_lowercase().as_str(),
        "mobile" | "android" | "ios" | "iphone" | "ipad" | "phone"
    )
}

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

/// Redis session store that **never** wraps [`CachedSessionStore`].
/// Royal insert lists locations inside a transaction; a cache would hide
/// another machine's delete until TTL.
pub async fn open_uncached_session_store(
    url: &str,
) -> Result<Arc<dyn SessionStorage>, SessionError> {
    open_uncached_redis_store(url).await
}

#[cfg(any(test, feature = "redis"))]
fn loc_cache_flag(val: Option<&str>) -> bool {
    matches!(val, Some(s) if s == "1" || s.eq_ignore_ascii_case("true"))
}

#[cfg(feature = "redis")]
fn loc_cache_enabled() -> bool {
    loc_cache_flag(std::env::var("KIM_LOC_CACHE").ok().as_deref())
}

#[cfg(feature = "redis")]
fn wrap_cache(store: Arc<dyn SessionStorage>) -> Arc<dyn SessionStorage> {
    if loc_cache_enabled() {
        CachedSessionStore::wrap(store)
    } else {
        store
    }
}

#[cfg(feature = "redis")]
async fn open_redis_primary(url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    let primary = Arc::new(redis::RedisSessionStore::open(url).await?);
    let store: Arc<dyn SessionStorage> = match std::env::var("REDIS_MIRROR_URL") {
        Ok(m) if !m.trim().is_empty() => {
            let mirror = Arc::new(redis::RedisSessionStore::open(m.trim()).await?);
            DualWriteStore::new(primary, mirror)
        }
        _ => primary,
    };
    Ok(store)
}

#[cfg(feature = "redis")]
async fn open_redis_store(url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    Ok(wrap_cache(open_redis_primary(url).await?))
}

#[cfg(feature = "redis")]
async fn open_uncached_redis_store(url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    open_redis_primary(url).await
}

#[cfg(not(feature = "redis"))]
async fn open_redis_store(_url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    Err(SessionError::Other("rebuild with --features redis".into()))
}

#[cfg(not(feature = "redis"))]
async fn open_uncached_redis_store(_url: &str) -> Result<Arc<dyn SessionStorage>, SessionError> {
    Err(SessionError::Other("rebuild with --features redis".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::pkt::Session;
    use kim_router::SessionError;
    use std::sync::Arc;

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

    #[test]
    fn loc_cache_is_opt_in() {
        assert!(!loc_cache_flag(None));
        assert!(!loc_cache_flag(Some("")));
        assert!(!loc_cache_flag(Some("0")));
        assert!(!loc_cache_flag(Some("false")));
        assert!(!loc_cache_flag(Some("yes")));
        assert!(loc_cache_flag(Some("1")));
        assert!(loc_cache_flag(Some("true")));
        assert!(loc_cache_flag(Some("TRUE")));
    }

    #[test]
    fn exclusive_device_classifies_mobile() {
        assert!(exclusive_device("mobile"));
        assert!(exclusive_device("iOS"));
        assert!(exclusive_device("Android"));
        assert!(!exclusive_device("web"));
        assert!(!exclusive_device(""));
        assert!(!exclusive_device("cli"));
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

    #[cfg(not(feature = "redis"))]
    #[tokio::test]
    async fn uncached_url_requires_redis_feature() {
        match open_uncached_session_store("redis://127.0.0.1:6379").await {
            Err(e) => assert_eq!(e.to_string(), "rebuild with --features redis"),
            Ok(_) => panic!("expected rebuild-with-features error"),
        }
    }

    #[tokio::test]
    async fn cached_store_hides_delete_that_uncached_inner_sees() {
        let inner = Arc::new(MemorySessionStore::new());
        inner.add(&session("c1", "alice", "g")).await.unwrap();
        let cached = CachedSessionStore::wrap(inner.clone());
        assert_eq!(
            cached.get_locations(&["alice".into()]).await.unwrap().len(),
            1
        );
        inner.delete("alice", "c1").await.unwrap();
        assert_eq!(
            cached.get_locations(&["alice".into()]).await.unwrap().len(),
            1
        );
        assert!(matches!(
            inner.get_locations(&["alice".into()]).await,
            Err(SessionError::NotFound)
        ));
    }
}
