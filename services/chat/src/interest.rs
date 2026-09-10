//! Room interest indexes: who is viewing a conversation dest.
//!
//! Keys (Redis / memory):
//! - `room:interest:{app}:{dest}:{kind}` → SET of `viewer#channel`
//! - `room:viewing:{app}:{account}:{channel}` → SET of `dest:kind`

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::{DashMap, Entry};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterestError {
    #[error("{0}")]
    Backend(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ViewerRef {
    pub account: String,
    pub channel_id: String,
}

impl ViewerRef {
    pub fn encode(&self) -> String {
        format!("{}#{}", self.account, self.channel_id)
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let (account, channel_id) = raw.split_once('#')?;
        if account.is_empty() || channel_id.is_empty() {
            return None;
        }
        Some(Self {
            account: account.to_string(),
            channel_id: channel_id.to_string(),
        })
    }
}

fn interest_key(app: &str, dest: &str, kind: i32) -> String {
    format!("room:interest:{app}:{dest}:{kind}")
}

fn viewing_key(app: &str, account: &str, channel: &str) -> String {
    format!("room:viewing:{app}:{account}:{channel}")
}

fn dest_token(dest: &str, kind: i32) -> String {
    format!("{dest}:{kind}")
}

fn parse_dest_token(raw: &str) -> Option<(String, i32)> {
    let (dest, kind) = raw.rsplit_once(':')?;
    let kind: i32 = kind.parse().ok()?;
    if dest.is_empty() {
        return None;
    }
    Some((dest.to_string(), kind))
}

#[async_trait]
pub trait RoomInterestStore: Send + Sync {
    /// Idempotent: refresh membership for `(viewer, channel) → dest`.
    async fn enter(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
        dest: &str,
        kind: i32,
    ) -> Result<(), InterestError>;

    async fn leave(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
        dest: &str,
        kind: i32,
    ) -> Result<(), InterestError>;

    /// Drop every interest owned by this connection.
    async fn clear_channel(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
    ) -> Result<(), InterestError>;

    async fn viewers(
        &self,
        app: &str,
        dest: &str,
        kind: i32,
    ) -> Result<Vec<ViewerRef>, InterestError>;
}

#[derive(Default)]
pub struct MemoryRoomInterest {
    // interest_key → members
    interest: DashMap<String, HashSet<String>>,
    // viewing_key → dest tokens
    viewing: DashMap<String, HashSet<String>>,
}

impl MemoryRoomInterest {
    pub fn new() -> Self {
        Self::default()
    }

    fn remove_member(map: &DashMap<String, HashSet<String>>, key: String, member: &str) {
        if let Entry::Occupied(mut occ) = map.entry(key) {
            occ.get_mut().remove(member);
            if occ.get().is_empty() {
                occ.remove();
            }
        }
    }
}

#[async_trait]
impl RoomInterestStore for MemoryRoomInterest {
    async fn enter(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
        dest: &str,
        kind: i32,
    ) -> Result<(), InterestError> {
        let member = ViewerRef {
            account: viewer.to_string(),
            channel_id: channel.to_string(),
        }
        .encode();
        let ik = interest_key(app, dest, kind);
        let vk = viewing_key(app, viewer, channel);
        let token = dest_token(dest, kind);
        self.interest.entry(ik).or_default().insert(member);
        self.viewing.entry(vk).or_default().insert(token);
        Ok(())
    }

    async fn leave(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
        dest: &str,
        kind: i32,
    ) -> Result<(), InterestError> {
        let member = ViewerRef {
            account: viewer.to_string(),
            channel_id: channel.to_string(),
        }
        .encode();
        let ik = interest_key(app, dest, kind);
        let vk = viewing_key(app, viewer, channel);
        let token = dest_token(dest, kind);
        Self::remove_member(&self.interest, ik, &member);
        Self::remove_member(&self.viewing, vk, &token);
        Ok(())
    }

    async fn clear_channel(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
    ) -> Result<(), InterestError> {
        let vk = viewing_key(app, viewer, channel);
        let tokens = self.viewing.remove(&vk).map(|(_, v)| v).unwrap_or_default();
        if tokens.is_empty() {
            return Ok(());
        }
        let member = ViewerRef {
            account: viewer.to_string(),
            channel_id: channel.to_string(),
        }
        .encode();
        for token in tokens {
            let Some((dest, kind)) = parse_dest_token(&token) else {
                continue;
            };
            let ik = interest_key(app, &dest, kind);
            Self::remove_member(&self.interest, ik, &member);
        }
        Ok(())
    }

    async fn viewers(
        &self,
        app: &str,
        dest: &str,
        kind: i32,
    ) -> Result<Vec<ViewerRef>, InterestError> {
        let ik = interest_key(app, dest, kind);
        let Some(set) = self.interest.get(&ik) else {
            return Ok(Vec::new());
        };
        Ok(set.iter().filter_map(|m| ViewerRef::decode(m)).collect())
    }
}

#[cfg(feature = "redis")]
mod redis_store {
    use super::*;
    use ::redis::aio::ConnectionManager;

    pub struct RedisRoomInterest {
        conn: ConnectionManager,
    }

    impl RedisRoomInterest {
        pub async fn open(url: &str) -> Result<Self, InterestError> {
            let conn = kim_session::open_connection_manager(url)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            Ok(Self { conn })
        }
    }

    #[async_trait]
    impl RoomInterestStore for RedisRoomInterest {
        async fn enter(
            &self,
            app: &str,
            viewer: &str,
            channel: &str,
            dest: &str,
            kind: i32,
        ) -> Result<(), InterestError> {
            let member = ViewerRef {
                account: viewer.to_string(),
                channel_id: channel.to_string(),
            }
            .encode();
            let ik = interest_key(app, dest, kind);
            let vk = viewing_key(app, viewer, channel);
            let token = dest_token(dest, kind);
            let mut conn = self.conn.clone();
            ::redis::pipe()
                .atomic()
                .cmd("SADD")
                .arg(&ik)
                .arg(&member)
                .ignore()
                .cmd("SADD")
                .arg(&vk)
                .arg(&token)
                .ignore()
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            Ok(())
        }

        async fn leave(
            &self,
            app: &str,
            viewer: &str,
            channel: &str,
            dest: &str,
            kind: i32,
        ) -> Result<(), InterestError> {
            let member = ViewerRef {
                account: viewer.to_string(),
                channel_id: channel.to_string(),
            }
            .encode();
            let ik = interest_key(app, dest, kind);
            let vk = viewing_key(app, viewer, channel);
            let token = dest_token(dest, kind);
            let mut conn = self.conn.clone();
            ::redis::pipe()
                .atomic()
                .cmd("SREM")
                .arg(&ik)
                .arg(&member)
                .ignore()
                .cmd("SREM")
                .arg(&vk)
                .arg(&token)
                .ignore()
                .query_async::<()>(&mut conn)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            Ok(())
        }

        async fn clear_channel(
            &self,
            app: &str,
            viewer: &str,
            channel: &str,
        ) -> Result<(), InterestError> {
            let vk = viewing_key(app, viewer, channel);
            let mut conn = self.conn.clone();
            let tokens: Vec<String> = ::redis::cmd("SMEMBERS")
                .arg(&vk)
                .query_async(&mut conn)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            if tokens.is_empty() {
                let _: () = ::redis::cmd("DEL")
                    .arg(&vk)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| InterestError::Backend(e.to_string()))?;
                return Ok(());
            }
            let member = ViewerRef {
                account: viewer.to_string(),
                channel_id: channel.to_string(),
            }
            .encode();
            let mut pipe = ::redis::pipe();
            pipe.atomic();
            for token in &tokens {
                let Some((dest, kind)) = parse_dest_token(token) else {
                    continue;
                };
                let ik = interest_key(app, &dest, kind);
                pipe.cmd("SREM").arg(ik).arg(&member).ignore();
            }
            pipe.cmd("DEL").arg(&vk).ignore();
            pipe.query_async::<()>(&mut conn)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            Ok(())
        }

        async fn viewers(
            &self,
            app: &str,
            dest: &str,
            kind: i32,
        ) -> Result<Vec<ViewerRef>, InterestError> {
            let ik = interest_key(app, dest, kind);
            let mut conn = self.conn.clone();
            let members: Vec<String> = ::redis::cmd("SMEMBERS")
                .arg(&ik)
                .query_async(&mut conn)
                .await
                .map_err(|e| InterestError::Backend(e.to_string()))?;
            Ok(members
                .iter()
                .filter_map(|m| ViewerRef::decode(m))
                .collect())
        }
    }
}

#[cfg(feature = "redis")]
pub use redis_store::RedisRoomInterest;

/// Open room-interest store. Empty/`None` → in-memory (tests / local default).
/// Non-empty URL requires `--features redis` (same URL as session / ack).
pub async fn open_room_interest(
    redis_url: Option<&str>,
) -> Result<Arc<dyn RoomInterestStore>, InterestError> {
    match redis_url {
        None | Some("") => Ok(Arc::new(MemoryRoomInterest::new())),
        Some(url) => open_redis_room_interest(url).await,
    }
}

#[cfg(feature = "redis")]
async fn open_redis_room_interest(url: &str) -> Result<Arc<dyn RoomInterestStore>, InterestError> {
    Ok(Arc::new(RedisRoomInterest::open(url).await?))
}

#[cfg(not(feature = "redis"))]
async fn open_redis_room_interest(_url: &str) -> Result<Arc<dyn RoomInterestStore>, InterestError> {
    Err(InterestError::Backend(
        "rebuild chat with --features redis".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enter_leave_and_clear() {
        let store = MemoryRoomInterest::new();
        store.enter("kim", "alice", "ch-a", "bob", 0).await.unwrap();
        store
            .enter("kim", "alice", "ch-a", "carol", 0)
            .await
            .unwrap();
        let v = store.viewers("kim", "bob", 0).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].account, "alice");

        store.leave("kim", "alice", "ch-a", "bob", 0).await.unwrap();
        assert!(store.viewers("kim", "bob", 0).await.unwrap().is_empty());

        store.clear_channel("kim", "alice", "ch-a").await.unwrap();
        assert!(store.viewers("kim", "carol", 0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn enter_is_idempotent() {
        let store = MemoryRoomInterest::new();
        store.enter("kim", "alice", "ch-a", "bob", 0).await.unwrap();
        store.enter("kim", "alice", "ch-a", "bob", 0).await.unwrap();
        assert_eq!(store.viewers("kim", "bob", 0).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn leave_last_viewer_does_not_drop_concurrent_enter() {
        for _ in 0..200 {
            let store = Arc::new(MemoryRoomInterest::new());
            store.enter("kim", "alice", "ch-a", "bob", 0).await.unwrap();
            let leaving = store.clone();
            let entering = store.clone();
            let leave = tokio::spawn({
                let store = leaving;
                async move {
                    store.leave("kim", "alice", "ch-a", "bob", 0).await.unwrap();
                }
            });
            let enter = tokio::spawn({
                let store = entering;
                async move {
                    store.enter("kim", "carol", "ch-c", "bob", 0).await.unwrap();
                }
            });
            leave.await.unwrap();
            enter.await.unwrap();
            let viewers = store.viewers("kim", "bob", 0).await.unwrap();
            assert!(
                viewers
                    .iter()
                    .any(|v| v.account == "carol" && v.channel_id == "ch-c"),
                "concurrent enter must survive last-viewer leave: {viewers:?}"
            );
        }
    }
}
