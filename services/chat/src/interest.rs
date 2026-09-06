//! Room interest indexes: who is viewing a conversation dest.
//!
//! Keys (Redis / memory):
//! - `room:interest:{app}:{dest}:{kind}` → SET of `viewer#channel`
//! - `room:viewing:{app}:{account}:{channel}` → SET of `dest:kind`

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use async_trait::async_trait;
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
    interest: Mutex<HashMap<String, HashSet<String>>>,
    // viewing_key → dest tokens
    viewing: Mutex<HashMap<String, HashSet<String>>>,
}

impl MemoryRoomInterest {
    pub fn new() -> Self {
        Self::default()
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
        self.interest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(ik)
            .or_default()
            .insert(member);
        self.viewing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(vk)
            .or_default()
            .insert(token);
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
        {
            let mut interest = self.interest.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(set) = interest.get_mut(&ik) {
                set.remove(&member);
                if set.is_empty() {
                    interest.remove(&ik);
                }
            }
        }
        {
            let mut viewing = self.viewing.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(set) = viewing.get_mut(&vk) {
                set.remove(&token);
                if set.is_empty() {
                    viewing.remove(&vk);
                }
            }
        }
        Ok(())
    }

    async fn clear_channel(
        &self,
        app: &str,
        viewer: &str,
        channel: &str,
    ) -> Result<(), InterestError> {
        let vk = viewing_key(app, viewer, channel);
        let tokens = {
            let mut viewing = self.viewing.lock().unwrap_or_else(|e| e.into_inner());
            viewing.remove(&vk).unwrap_or_default()
        };
        if tokens.is_empty() {
            return Ok(());
        }
        let member = ViewerRef {
            account: viewer.to_string(),
            channel_id: channel.to_string(),
        }
        .encode();
        let mut interest = self.interest.lock().unwrap_or_else(|e| e.into_inner());
        for token in tokens {
            let Some((dest, kind)) = parse_dest_token(&token) else {
                continue;
            };
            let ik = interest_key(app, &dest, kind);
            if let Some(set) = interest.get_mut(&ik) {
                set.remove(&member);
                if set.is_empty() {
                    interest.remove(&ik);
                }
            }
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
        let interest = self.interest.lock().unwrap_or_else(|e| e.into_inner());
        let Some(set) = interest.get(&ik) else {
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
}
