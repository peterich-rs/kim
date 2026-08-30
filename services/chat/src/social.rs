//! Friend requests, accepted friendships, and blocks.

use std::collections::HashSet;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum SocialError {
    #[error("self")]
    SelfOp,
    #[error("not found")]
    NotFound,
    #[error("blocked")]
    Blocked,
    #[error("{0}")]
    Backend(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendRequestOutcome {
    Sent,
    AutoAccepted,
    AlreadyFriends,
}

pub(crate) fn ordered_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[async_trait]
pub trait SocialDirectory: Send + Sync {
    async fn request(
        &self,
        app: &str,
        from: &str,
        to: &str,
    ) -> Result<FriendRequestOutcome, SocialError>;
    async fn accept(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError>;
    async fn reject(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError>;
    async fn remove(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError>;
    async fn list_friends(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError>;
    async fn incoming(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError>;
    async fn is_friend(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError>;
    async fn block(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError>;
    async fn unblock(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError>;
    async fn list_blocked(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError>;
    async fn is_blocked_either(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError>;
}

#[derive(Default)]
struct Inner {
    requests: HashSet<(String, String, String)>,
    friends: HashSet<(String, String, String)>,
    blocks: HashSet<(String, String, String)>,
}

pub struct MemorySocialDirectory {
    inner: RwLock<Inner>,
}

impl MemorySocialDirectory {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner::default()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for MemorySocialDirectory {
    fn default() -> Self {
        Self::new()
    }
}

fn friend_key(app: &str, a: &str, b: &str) -> (String, String, String) {
    let (x, y) = ordered_pair(a, b);
    (app.to_string(), x.to_string(), y.to_string())
}

#[async_trait]
impl SocialDirectory for MemorySocialDirectory {
    async fn request(
        &self,
        app: &str,
        from: &str,
        to: &str,
    ) -> Result<FriendRequestOutcome, SocialError> {
        if from.is_empty() || to.is_empty() || from == to {
            return Err(SocialError::SelfOp);
        }
        let mut inner = self.write();
        if inner.blocks.contains(&(app.into(), from.into(), to.into()))
            || inner.blocks.contains(&(app.into(), to.into(), from.into()))
        {
            return Err(SocialError::Blocked);
        }
        if inner.friends.contains(&friend_key(app, from, to)) {
            return Ok(FriendRequestOutcome::AlreadyFriends);
        }
        let reverse = (app.to_string(), to.to_string(), from.to_string());
        if inner.requests.contains(&reverse) {
            inner.requests.remove(&reverse);
            inner
                .requests
                .remove(&(app.to_string(), from.to_string(), to.to_string()));
            inner.friends.insert(friend_key(app, from, to));
            return Ok(FriendRequestOutcome::AutoAccepted);
        }
        inner
            .requests
            .insert((app.to_string(), from.to_string(), to.to_string()));
        Ok(FriendRequestOutcome::Sent)
    }

    async fn accept(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        if account == from {
            return Err(SocialError::SelfOp);
        }
        let mut inner = self.write();
        if inner
            .blocks
            .contains(&(app.into(), account.into(), from.into()))
            || inner
                .blocks
                .contains(&(app.into(), from.into(), account.into()))
        {
            return Err(SocialError::Blocked);
        }
        let pending = (app.to_string(), from.to_string(), account.to_string());
        if !inner.requests.remove(&pending)
            && !inner.friends.contains(&friend_key(app, account, from))
        {
            return Err(SocialError::NotFound);
        }
        inner.friends.insert(friend_key(app, account, from));
        Ok(())
    }

    async fn reject(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        self.write()
            .requests
            .remove(&(app.to_string(), from.to_string(), account.to_string()));
        Ok(())
    }

    async fn remove(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.write().friends.remove(&friend_key(app, account, peer));
        Ok(())
    }

    async fn list_friends(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let inner = self.read();
        let mut out: Vec<String> = inner
            .friends
            .iter()
            .filter(|(row_app, a, b)| row_app == app && (a == account || b == account))
            .map(|(_, a, b)| if a == account { b.clone() } else { a.clone() })
            .collect();
        out.sort();
        Ok(out)
    }

    async fn incoming(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let inner = self.read();
        let mut out: Vec<String> = inner
            .requests
            .iter()
            .filter(|(row_app, _, to)| row_app == app && to == account)
            .map(|(_, from, _)| from.clone())
            .collect();
        out.sort();
        Ok(out)
    }

    async fn is_friend(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        if a == b {
            return Ok(true);
        }
        Ok(self.read().friends.contains(&friend_key(app, a, b)))
    }

    async fn block(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        if account == peer {
            return Err(SocialError::SelfOp);
        }
        let mut inner = self.write();
        inner
            .blocks
            .insert((app.to_string(), account.to_string(), peer.to_string()));
        inner.friends.remove(&friend_key(app, account, peer));
        inner
            .requests
            .remove(&(app.to_string(), account.to_string(), peer.to_string()));
        inner
            .requests
            .remove(&(app.to_string(), peer.to_string(), account.to_string()));
        Ok(())
    }

    async fn unblock(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.write()
            .blocks
            .remove(&(app.to_string(), account.to_string(), peer.to_string()));
        Ok(())
    }

    async fn list_blocked(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let inner = self.read();
        let mut out: Vec<String> = inner
            .blocks
            .iter()
            .filter(|(row_app, acc, _)| row_app == app && acc == account)
            .map(|(_, _, blocked)| blocked.clone())
            .collect();
        out.sort();
        Ok(out)
    }

    async fn is_blocked_either(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let inner = self.read();
        Ok(inner.blocks.contains(&(app.into(), a.into(), b.into()))
            || inner.blocks.contains(&(app.into(), b.into(), a.into())))
    }
}

#[cfg(feature = "postgres")]
pub struct PostgresSocialDirectory {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresSocialDirectory {
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
fn pg_err(e: sqlx::Error) -> SocialError {
    SocialError::Backend(e.to_string())
}

#[cfg(feature = "postgres")]
#[async_trait]
impl SocialDirectory for PostgresSocialDirectory {
    async fn request(
        &self,
        app: &str,
        from: &str,
        to: &str,
    ) -> Result<FriendRequestOutcome, SocialError> {
        if from.is_empty() || to.is_empty() || from == to {
            return Err(SocialError::SelfOp);
        }
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        let blocked: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM blocks
             WHERE app = $1 AND (
               (account = $2 AND blocked = $3) OR (account = $3 AND blocked = $2)
             )",
        )
        .bind(app)
        .bind(from)
        .bind(to)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if blocked.is_some() {
            return Err(SocialError::Blocked);
        }
        let (a, b) = ordered_pair(from, to);
        let friends: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3",
        )
        .bind(app)
        .bind(a)
        .bind(b)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if friends.is_some() {
            tx.commit().await.map_err(pg_err)?;
            return Ok(FriendRequestOutcome::AlreadyFriends);
        }
        let reverse: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM friend_requests
             WHERE app = $1 AND from_account = $2 AND to_account = $3",
        )
        .bind(app)
        .bind(to)
        .bind(from)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if reverse.is_some() {
            sqlx::query(
                "DELETE FROM friend_requests
                 WHERE app = $1 AND (
                   (from_account = $2 AND to_account = $3)
                   OR (from_account = $3 AND to_account = $2)
                 )",
            )
            .bind(app)
            .bind(from)
            .bind(to)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
            sqlx::query(
                "INSERT INTO friendships (app, account_a, account_b)
                 VALUES ($1, $2, $3)
                 ON CONFLICT DO NOTHING",
            )
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
            tx.commit().await.map_err(pg_err)?;
            return Ok(FriendRequestOutcome::AutoAccepted);
        }
        sqlx::query(
            "INSERT INTO friend_requests (app, from_account, to_account)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(from)
        .bind(to)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        tx.commit().await.map_err(pg_err)?;
        Ok(FriendRequestOutcome::Sent)
    }

    async fn accept(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        if account == from {
            return Err(SocialError::SelfOp);
        }
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        let blocked: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM blocks
             WHERE app = $1 AND (
               (account = $2 AND blocked = $3) OR (account = $3 AND blocked = $2)
             )",
        )
        .bind(app)
        .bind(account)
        .bind(from)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if blocked.is_some() {
            return Err(SocialError::Blocked);
        }
        let deleted = sqlx::query(
            "DELETE FROM friend_requests
             WHERE app = $1 AND from_account = $2 AND to_account = $3",
        )
        .bind(app)
        .bind(from)
        .bind(account)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        let (a, b) = ordered_pair(account, from);
        let already: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3",
        )
        .bind(app)
        .bind(a)
        .bind(b)
        .fetch_optional(&mut *tx)
        .await
        .map_err(pg_err)?;
        if deleted.rows_affected() == 0 && already.is_none() {
            return Err(SocialError::NotFound);
        }
        sqlx::query(
            "INSERT INTO friendships (app, account_a, account_b)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(a)
        .bind(b)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        tx.commit().await.map_err(pg_err)?;
        Ok(())
    }

    async fn reject(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        sqlx::query(
            "DELETE FROM friend_requests
             WHERE app = $1 AND from_account = $2 AND to_account = $3",
        )
        .bind(app)
        .bind(from)
        .bind(account)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(())
    }

    async fn remove(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        let (a, b) = ordered_pair(account, peer);
        sqlx::query("DELETE FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3")
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&self.pool)
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn list_friends(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT account_a, account_b FROM friendships
             WHERE app = $1 AND (account_a = $2 OR account_b = $2)
             ORDER BY account_a, account_b",
        )
        .bind(app)
        .bind(account)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows
            .into_iter()
            .map(|(a, b)| if a == account { b } else { a })
            .collect())
    }

    async fn incoming(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT from_account FROM friend_requests
             WHERE app = $1 AND to_account = $2
             ORDER BY created_at ASC",
        )
        .bind(app)
        .bind(account)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn is_friend(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        if a == b {
            return Ok(true);
        }
        let (x, y) = ordered_pair(a, b);
        let found: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3",
        )
        .bind(app)
        .bind(x)
        .bind(y)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(found.is_some())
    }

    async fn block(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        if account == peer {
            return Err(SocialError::SelfOp);
        }
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        sqlx::query(
            "INSERT INTO blocks (app, account, blocked) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(app)
        .bind(account)
        .bind(peer)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        let (a, b) = ordered_pair(account, peer);
        sqlx::query("DELETE FROM friendships WHERE app = $1 AND account_a = $2 AND account_b = $3")
            .bind(app)
            .bind(a)
            .bind(b)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        sqlx::query(
            "DELETE FROM friend_requests
             WHERE app = $1 AND (
               (from_account = $2 AND to_account = $3)
               OR (from_account = $3 AND to_account = $2)
             )",
        )
        .bind(app)
        .bind(account)
        .bind(peer)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        tx.commit().await.map_err(pg_err)?;
        Ok(())
    }

    async fn unblock(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        sqlx::query("DELETE FROM blocks WHERE app = $1 AND account = $2 AND blocked = $3")
            .bind(app)
            .bind(account)
            .bind(peer)
            .execute(&self.pool)
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn list_blocked(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT blocked FROM blocks WHERE app = $1 AND account = $2 ORDER BY blocked",
        )
        .bind(app)
        .bind(account)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn is_blocked_either(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let found: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM blocks
             WHERE app = $1 AND (
               (account = $2 AND blocked = $3) OR (account = $3 AND blocked = $2)
             )",
        )
        .bind(app)
        .bind(a)
        .bind(b)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(found.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_accept_and_talk_gate() {
        let dir = MemorySocialDirectory::new();
        assert_eq!(
            dir.request("kim", "alice", "bob").await.unwrap(),
            FriendRequestOutcome::Sent
        );
        assert!(!dir.is_friend("kim", "alice", "bob").await.unwrap());
        dir.accept("kim", "bob", "alice").await.unwrap();
        assert!(dir.is_friend("kim", "alice", "bob").await.unwrap());
        assert_eq!(
            dir.list_friends("kim", "alice").await.unwrap(),
            vec!["bob".to_string()]
        );
    }

    #[tokio::test]
    async fn reverse_request_auto_accepts() {
        let dir = MemorySocialDirectory::new();
        dir.request("kim", "alice", "bob").await.unwrap();
        assert_eq!(
            dir.request("kim", "bob", "alice").await.unwrap(),
            FriendRequestOutcome::AutoAccepted
        );
        assert!(dir.is_friend("kim", "alice", "bob").await.unwrap());
        assert!(dir.incoming("kim", "bob").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn block_removes_friend_and_pending() {
        let dir = MemorySocialDirectory::new();
        dir.request("kim", "alice", "bob").await.unwrap();
        dir.accept("kim", "bob", "alice").await.unwrap();
        dir.block("kim", "bob", "alice").await.unwrap();
        assert!(!dir.is_friend("kim", "alice", "bob").await.unwrap());
        assert!(dir.is_blocked_either("kim", "alice", "bob").await.unwrap());
        assert!(matches!(
            dir.request("kim", "alice", "bob").await,
            Err(SocialError::Blocked)
        ));
        dir.unblock("kim", "bob", "alice").await.unwrap();
        assert!(!dir.is_blocked_either("kim", "alice", "bob").await.unwrap());
    }

    #[tokio::test]
    async fn self_request_is_rejected() {
        let dir = MemorySocialDirectory::new();
        assert!(matches!(
            dir.request("kim", "alice", "alice").await,
            Err(SocialError::SelfOp)
        ));
    }
}
