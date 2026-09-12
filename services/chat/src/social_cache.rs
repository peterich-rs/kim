//! Short-TTL friend / block / exists cache in front of directory traits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::social::{ordered_pair, FriendRequestOutcome, SocialDirectory, SocialError};
use crate::users::{CreateBot, ProfilePatch, UserDirectory, UserError, UserPresence, UserProfile};

const DEFAULT_TTL: Duration = Duration::from_secs(30);
const DEFAULT_CAP: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SocialQueryKind {
    Friend,
    BlockedEither,
}

type SocialKey = (SocialQueryKind, String, String, String);
type InflightWaiters = Vec<oneshot::Sender<Result<bool, String>>>;

fn social_ttl() -> Duration {
    match std::env::var("KIM_SOCIAL_CACHE_TTL_MS") {
        Ok(s) if s.trim() == "0" => Duration::ZERO,
        Ok(s) => s
            .trim()
            .parse::<u64>()
            .ok()
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_TTL),
        _ => DEFAULT_TTL,
    }
}

fn jittered_ttl(base: Duration) -> Duration {
    if base.is_zero() {
        return Duration::ZERO;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let span = (base.as_millis().saturating_mul(20) / 100).max(1);
    let delta = u128::from(nanos) % (span.saturating_mul(2).saturating_add(1));
    let signed = i128::try_from(delta).unwrap_or(0) - i128::try_from(span).unwrap_or(0);
    let ms = (i128::try_from(base.as_millis()).unwrap_or(0) + signed).max(1);
    Duration::from_millis(u64::try_from(ms).unwrap_or(1))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn make_key(kind: SocialQueryKind, app: &str, a: &str, b: &str) -> SocialKey {
    let (x, y) = ordered_pair(a, b);
    (kind, app.to_string(), x.to_string(), y.to_string())
}

struct Entry {
    value: bool,
    expire: Instant,
}

pub struct CachedSocial {
    inner: Arc<dyn SocialDirectory>,
    entries: Mutex<HashMap<SocialKey, Entry>>,
    inflight: Mutex<HashMap<SocialKey, InflightWaiters>>,
    ttl: Duration,
    cap: usize,
}

impl CachedSocial {
    pub fn wrap(inner: Arc<dyn SocialDirectory>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            entries: Mutex::new(HashMap::new()),
            inflight: Mutex::new(HashMap::new()),
            ttl: social_ttl(),
            cap: DEFAULT_CAP,
        })
    }

    fn lookup(&self, key: &SocialKey) -> Option<bool> {
        if self.ttl.is_zero() {
            return None;
        }
        let now = Instant::now();
        let mut map = lock(&self.entries);
        match map.get(key) {
            Some(e) if e.expire > now => Some(e.value),
            Some(_) => {
                map.remove(key);
                None
            }
            None => None,
        }
    }

    fn store(&self, key: SocialKey, value: bool) {
        if self.ttl.is_zero() {
            return;
        }
        let mut map = lock(&self.entries);
        if map.len() >= self.cap {
            evict_oldest_tenth(&mut map);
        }
        map.insert(
            key,
            Entry {
                value,
                expire: Instant::now() + jittered_ttl(self.ttl),
            },
        );
    }

    fn evict_pair(&self, app: &str, a: &str, b: &str) {
        let friend = make_key(SocialQueryKind::Friend, app, a, b);
        let blocked = make_key(SocialQueryKind::BlockedEither, app, a, b);
        let mut map = lock(&self.entries);
        map.remove(&friend);
        map.remove(&blocked);
    }

    async fn cached_bool<F, Fut>(&self, key: SocialKey, fetch: F) -> Result<bool, SocialError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<bool, SocialError>>,
    {
        if let Some(v) = self.lookup(&key) {
            return Ok(v);
        }
        if self.ttl.is_zero() {
            return fetch().await;
        }
        let rx = {
            let mut inflight = lock(&self.inflight);
            if let Some(waiters) = inflight.get_mut(&key) {
                let (tx, rx) = oneshot::channel();
                waiters.push(tx);
                Some(rx)
            } else {
                inflight.insert(key.clone(), Vec::new());
                None
            }
        };
        if let Some(rx) = rx {
            return match rx.await {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(s)) => Err(SocialError::Backend(s)),
                Err(_) => Err(SocialError::Backend("inflight dropped".into())),
            };
        }
        let result = fetch().await;
        let waiters = lock(&self.inflight).remove(&key).unwrap_or_default();
        match &result {
            Ok(v) => {
                self.store(key, *v);
                for tx in waiters {
                    let _ = tx.send(Ok(*v));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                for tx in waiters {
                    let _ = tx.send(Err(msg.clone()));
                }
            }
        }
        result
    }
}

fn evict_oldest_tenth(map: &mut HashMap<SocialKey, Entry>) {
    if map.is_empty() {
        return;
    }
    let n = (map.len() / 10).max(1);
    let mut keys: Vec<(SocialKey, Instant)> =
        map.iter().map(|(k, e)| (k.clone(), e.expire)).collect();
    keys.sort_by_key(|(_, exp)| *exp);
    for (k, _) in keys.into_iter().take(n) {
        map.remove(&k);
    }
}

#[async_trait]
impl SocialDirectory for CachedSocial {
    async fn request(
        &self,
        app: &str,
        from: &str,
        to: &str,
    ) -> Result<FriendRequestOutcome, SocialError> {
        let out = self.inner.request(app, from, to).await?;
        self.evict_pair(app, from, to);
        Ok(out)
    }

    async fn accept(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        self.inner.accept(app, account, from).await?;
        self.evict_pair(app, account, from);
        Ok(())
    }

    async fn reject(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
        self.inner.reject(app, account, from).await?;
        self.evict_pair(app, account, from);
        Ok(())
    }

    async fn remove(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.inner.remove(app, account, peer).await?;
        self.evict_pair(app, account, peer);
        Ok(())
    }

    async fn list_friends(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        self.inner.list_friends(app, account).await
    }

    async fn incoming(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        self.inner.incoming(app, account).await
    }

    async fn is_friend(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let key = make_key(SocialQueryKind::Friend, app, a, b);
        self.cached_bool(key, || self.inner.is_friend(app, a, b))
            .await
    }

    async fn block(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.inner.block(app, account, peer).await?;
        self.evict_pair(app, account, peer);
        Ok(())
    }

    async fn unblock(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
        self.inner.unblock(app, account, peer).await?;
        self.evict_pair(app, account, peer);
        Ok(())
    }

    async fn list_blocked(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
        self.inner.list_blocked(app, account).await
    }

    async fn is_blocked_either(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
        let key = make_key(SocialQueryKind::BlockedEither, app, a, b);
        self.cached_bool(key, || self.inner.is_blocked_either(app, a, b))
            .await
    }

    async fn ensure_friends(&self, app: &str, a: &str, b: &str) -> Result<(), SocialError> {
        self.inner.ensure_friends(app, a, b).await?;
        self.evict_pair(app, a, b);
        Ok(())
    }
}

type UserKey = (String, String);
type UserInflightWaiters = Vec<oneshot::Sender<Result<UserPresence, String>>>;

struct UserEntry {
    value: UserPresence,
    expire: Instant,
}

pub struct CachedUserDirectory {
    inner: Arc<dyn UserDirectory>,
    entries: Mutex<HashMap<UserKey, UserEntry>>,
    inflight: Mutex<HashMap<UserKey, UserInflightWaiters>>,
    ttl: Duration,
    cap: usize,
}

impl CachedUserDirectory {
    pub fn wrap(inner: Arc<dyn UserDirectory>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            entries: Mutex::new(HashMap::new()),
            inflight: Mutex::new(HashMap::new()),
            ttl: social_ttl(),
            cap: DEFAULT_CAP,
        })
    }

    fn evict(&self, app: &str, account: &str) {
        lock(&self.entries).remove(&(app.to_string(), account.to_string()));
    }

    fn lookup_entry(&self, key: &UserKey) -> Option<UserPresence> {
        if self.ttl.is_zero() {
            return None;
        }
        let now = Instant::now();
        let mut map = lock(&self.entries);
        match map.get(key) {
            Some(e) if e.expire > now => Some(e.value.clone()),
            Some(_) => {
                map.remove(key);
                None
            }
            None => None,
        }
    }

    fn store_presence(&self, key: UserKey, value: UserPresence) {
        if self.ttl.is_zero() {
            return;
        }
        let mut map = lock(&self.entries);
        if map.len() >= self.cap {
            let n = (map.len() / 10).max(1);
            let mut keys: Vec<(UserKey, Instant)> =
                map.iter().map(|(k, e)| (k.clone(), e.expire)).collect();
            keys.sort_by_key(|(_, exp)| *exp);
            for (k, _) in keys.into_iter().take(n) {
                map.remove(&k);
            }
        }
        map.insert(
            key,
            UserEntry {
                value,
                expire: Instant::now() + jittered_ttl(self.ttl),
            },
        );
    }

    async fn cached_presence(&self, app: &str, account: &str) -> Result<UserPresence, UserError> {
        let key = (app.to_string(), account.to_string());
        if let Some(v) = self.lookup_entry(&key) {
            return Ok(v);
        }
        if self.ttl.is_zero() {
            return match self.inner.lookup(app, account).await? {
                Some(p) => Ok(p),
                None => Ok(UserPresence {
                    exists: false,
                    kind: 0,
                    owner_account: String::new(),
                }),
            };
        }
        let rx = {
            let mut inflight = lock(&self.inflight);
            if let Some(waiters) = inflight.get_mut(&key) {
                let (tx, rx) = oneshot::channel();
                waiters.push(tx);
                Some(rx)
            } else {
                inflight.insert(key.clone(), Vec::new());
                None
            }
        };
        if let Some(rx) = rx {
            return match rx.await {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(s)) => Err(UserError::Backend(s)),
                Err(_) => Err(UserError::Backend("inflight dropped".into())),
            };
        }
        let fetched = match self.inner.lookup(app, account).await {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Ok(UserPresence {
                exists: false,
                kind: 0,
                owner_account: String::new(),
            }),
            Err(e) => Err(e),
        };
        let waiters = lock(&self.inflight).remove(&key).unwrap_or_default();
        match &fetched {
            Ok(v) => {
                self.store_presence(key, v.clone());
                for tx in waiters {
                    let _ = tx.send(Ok(v.clone()));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                for tx in waiters {
                    let _ = tx.send(Err(msg.clone()));
                }
            }
        }
        fetched
    }
}

#[async_trait]
impl UserDirectory for CachedUserDirectory {
    async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
        self.inner.upsert(app, account).await?;
        self.evict(app, account);
        Ok(())
    }
    async fn create(&self, app: &str, account: &str, password_hash: &str) -> Result<(), UserError> {
        self.inner.create(app, account, password_hash).await?;
        self.evict(app, account);
        Ok(())
    }
    async fn password_hash(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        self.inner.password_hash(app, account).await
    }
    async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
        Ok(self.cached_presence(app, account).await?.exists)
    }
    async fn profile(&self, app: &str, account: &str) -> Result<Option<UserProfile>, UserError> {
        self.inner.profile(app, account).await
    }
    async fn update_profile(
        &self,
        app: &str,
        account: &str,
        patch: &ProfilePatch,
    ) -> Result<UserProfile, UserError> {
        self.inner.update_profile(app, account, patch).await
    }
    async fn profiles(
        &self,
        app: &str,
        accounts: &[String],
    ) -> Result<Vec<UserProfile>, UserError> {
        self.inner.profiles(app, accounts).await
    }
    async fn search(
        &self,
        app: &str,
        query: &str,
        exclude: &[String],
        limit: usize,
    ) -> Result<Vec<UserProfile>, UserError> {
        self.inner.search(app, query, exclude, limit).await
    }
    async fn set_password(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<(), UserError> {
        self.inner.set_password(app, account, password_hash).await
    }
    async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        self.inner.token_epoch(app, account).await
    }
    async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
        self.inner.bump_token_epoch(app, account).await
    }
    async fn set_password_and_bump_epoch(
        &self,
        app: &str,
        account: &str,
        password_hash: &str,
    ) -> Result<u32, UserError> {
        self.inner
            .set_password_and_bump_epoch(app, account, password_hash)
            .await
    }

    async fn create_bot(&self, app: &str, req: &CreateBot) -> Result<UserProfile, UserError> {
        let profile = self.inner.create_bot(app, req).await?;
        self.evict(app, &profile.account);
        Ok(profile)
    }

    async fn bot_owner(&self, app: &str, account: &str) -> Result<Option<String>, UserError> {
        let p = self.cached_presence(app, account).await?;
        if p.exists && p.kind == kim_protocol::PROFILE_KIND_BOT && !p.owner_account.is_empty() {
            Ok(Some(p.owner_account))
        } else {
            Ok(None)
        }
    }

    async fn delete_bot(&self, app: &str, owner: &str, account: &str) -> Result<(), UserError> {
        self.inner.delete_bot(app, owner, account).await?;
        self.evict(app, account);
        Ok(())
    }

    async fn count_bots(&self, app: &str, owner: &str) -> Result<u32, UserError> {
        self.inner.count_bots(app, owner).await
    }

    async fn lookup(&self, app: &str, account: &str) -> Result<Option<UserPresence>, UserError> {
        let p = self.cached_presence(app, account).await?;
        if p.exists {
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    use crate::social::MemorySocialDirectory;
    use crate::users::MemoryUserDirectory;

    struct CountingSocial {
        inner: MemorySocialDirectory,
        friend: AtomicU32,
        blocked: AtomicU32,
    }

    impl CountingSocial {
        fn new() -> Self {
            Self {
                inner: MemorySocialDirectory::new(),
                friend: AtomicU32::new(0),
                blocked: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl SocialDirectory for CountingSocial {
        async fn request(
            &self,
            app: &str,
            from: &str,
            to: &str,
        ) -> Result<FriendRequestOutcome, SocialError> {
            self.inner.request(app, from, to).await
        }
        async fn accept(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
            self.inner.accept(app, account, from).await
        }
        async fn reject(&self, app: &str, account: &str, from: &str) -> Result<(), SocialError> {
            self.inner.reject(app, account, from).await
        }
        async fn remove(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
            self.inner.remove(app, account, peer).await
        }
        async fn list_friends(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
            self.inner.list_friends(app, account).await
        }
        async fn incoming(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
            self.inner.incoming(app, account).await
        }
        async fn is_friend(&self, app: &str, a: &str, b: &str) -> Result<bool, SocialError> {
            self.friend.fetch_add(1, Ordering::SeqCst);
            self.inner.is_friend(app, a, b).await
        }
        async fn block(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
            self.inner.block(app, account, peer).await
        }
        async fn unblock(&self, app: &str, account: &str, peer: &str) -> Result<(), SocialError> {
            self.inner.unblock(app, account, peer).await
        }
        async fn list_blocked(&self, app: &str, account: &str) -> Result<Vec<String>, SocialError> {
            self.inner.list_blocked(app, account).await
        }
        async fn is_blocked_either(
            &self,
            app: &str,
            a: &str,
            b: &str,
        ) -> Result<bool, SocialError> {
            self.blocked.fetch_add(1, Ordering::SeqCst);
            self.inner.is_blocked_either(app, a, b).await
        }
        async fn ensure_friends(&self, app: &str, a: &str, b: &str) -> Result<(), SocialError> {
            self.inner.ensure_friends(app, a, b).await
        }
    }

    #[tokio::test]
    async fn friend_and_block_keys_do_not_overlap() {
        let inner = Arc::new(CountingSocial::new());
        inner.request("kim", "alice", "bob").await.unwrap();
        inner.accept("kim", "bob", "alice").await.unwrap();
        let cache = CachedSocial::wrap(inner.clone());
        assert!(!cache
            .is_blocked_either("kim", "alice", "bob")
            .await
            .unwrap());
        assert!(cache.is_friend("kim", "alice", "bob").await.unwrap());
        assert_eq!(inner.blocked.load(Ordering::SeqCst), 1);
        assert_eq!(inner.friend.load(Ordering::SeqCst), 1);
        assert!(!cache
            .is_blocked_either("kim", "alice", "bob")
            .await
            .unwrap());
        assert!(cache.is_friend("kim", "alice", "bob").await.unwrap());
        assert_eq!(inner.blocked.load(Ordering::SeqCst), 1);
        assert_eq!(inner.friend.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ordered_pair_hits_same_entry() {
        let inner = Arc::new(CountingSocial::new());
        inner.request("kim", "alice", "bob").await.unwrap();
        inner.accept("kim", "bob", "alice").await.unwrap();
        let cache = CachedSocial::wrap(inner.clone());
        assert!(cache.is_friend("kim", "alice", "bob").await.unwrap());
        assert!(cache.is_friend("kim", "bob", "alice").await.unwrap());
        assert_eq!(inner.friend.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn write_evicts_both_kinds() {
        let inner = Arc::new(CountingSocial::new());
        inner.request("kim", "alice", "bob").await.unwrap();
        inner.accept("kim", "bob", "alice").await.unwrap();
        let cache = CachedSocial::wrap(inner.clone());
        assert!(cache.is_friend("kim", "alice", "bob").await.unwrap());
        cache.remove("kim", "alice", "bob").await.unwrap();
        assert!(!cache.is_friend("kim", "alice", "bob").await.unwrap());
        assert_eq!(inner.friend.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn concurrent_miss_single_flights() {
        let inner = Arc::new(CountingSocial::new());
        inner.request("kim", "alice", "bob").await.unwrap();
        inner.accept("kim", "bob", "alice").await.unwrap();
        let cache = CachedSocial::wrap(inner.clone());
        let a = cache.clone();
        let b = cache.clone();
        let (ra, rb) = tokio::join!(
            a.is_friend("kim", "alice", "bob"),
            b.is_friend("kim", "bob", "alice"),
        );
        assert!(ra.unwrap());
        assert!(rb.unwrap());
        assert_eq!(inner.friend.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn expired_entry_refetches_and_is_not_stale_if_error() {
        let inner = Arc::new(CountingSocial::new());
        inner.request("kim", "alice", "bob").await.unwrap();
        inner.accept("kim", "bob", "alice").await.unwrap();
        let cache = CachedSocial::wrap(inner.clone());
        let key = make_key(SocialQueryKind::Friend, "kim", "alice", "bob");
        {
            let mut map = lock(&cache.entries);
            map.insert(
                key,
                Entry {
                    value: true,
                    expire: Instant::now() - Duration::from_secs(1),
                },
            );
        }
        assert!(cache.is_friend("kim", "alice", "bob").await.unwrap());
        assert_eq!(inner.friend.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn evicts_oldest_tenth_at_cap() {
        let cache = CachedSocial::wrap(Arc::new(MemorySocialDirectory::new()));
        {
            let mut map = lock(&cache.entries);
            for i in 0..20 {
                let key = make_key(SocialQueryKind::Friend, "kim", "a", &i.to_string());
                map.insert(
                    key,
                    Entry {
                        value: false,
                        expire: Instant::now() + Duration::from_secs(u64::try_from(i).unwrap_or(0)),
                    },
                );
            }
            evict_oldest_tenth(&mut map);
            assert_eq!(map.len(), 18);
        }
    }

    #[tokio::test]
    async fn exists_cache_hits() {
        let users = Arc::new(MemoryUserDirectory::new());
        users.upsert("kim", "alice").await.unwrap();
        let hits = Arc::new(AtomicU32::new(0));
        struct CountExists {
            inner: Arc<MemoryUserDirectory>,
            hits: Arc<AtomicU32>,
        }
        #[async_trait]
        impl UserDirectory for CountExists {
            async fn upsert(&self, app: &str, account: &str) -> Result<(), UserError> {
                self.inner.upsert(app, account).await
            }
            async fn create(
                &self,
                app: &str,
                account: &str,
                password_hash: &str,
            ) -> Result<(), UserError> {
                self.inner.create(app, account, password_hash).await
            }
            async fn password_hash(
                &self,
                app: &str,
                account: &str,
            ) -> Result<Option<String>, UserError> {
                self.inner.password_hash(app, account).await
            }
            async fn exists(&self, app: &str, account: &str) -> Result<bool, UserError> {
                self.hits.fetch_add(1, Ordering::SeqCst);
                self.inner.exists(app, account).await
            }
            async fn profile(
                &self,
                app: &str,
                account: &str,
            ) -> Result<Option<UserProfile>, UserError> {
                self.inner.profile(app, account).await
            }
            async fn update_profile(
                &self,
                app: &str,
                account: &str,
                patch: &ProfilePatch,
            ) -> Result<UserProfile, UserError> {
                self.inner.update_profile(app, account, patch).await
            }
            async fn profiles(
                &self,
                app: &str,
                accounts: &[String],
            ) -> Result<Vec<UserProfile>, UserError> {
                self.inner.profiles(app, accounts).await
            }
            async fn search(
                &self,
                app: &str,
                query: &str,
                exclude: &[String],
                limit: usize,
            ) -> Result<Vec<UserProfile>, UserError> {
                self.inner.search(app, query, exclude, limit).await
            }
            async fn set_password(
                &self,
                app: &str,
                account: &str,
                password_hash: &str,
            ) -> Result<(), UserError> {
                self.inner.set_password(app, account, password_hash).await
            }
            async fn token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
                self.inner.token_epoch(app, account).await
            }
            async fn bump_token_epoch(&self, app: &str, account: &str) -> Result<u32, UserError> {
                self.inner.bump_token_epoch(app, account).await
            }
            async fn set_password_and_bump_epoch(
                &self,
                app: &str,
                account: &str,
                password_hash: &str,
            ) -> Result<u32, UserError> {
                self.inner
                    .set_password_and_bump_epoch(app, account, password_hash)
                    .await
            }
            async fn create_bot(
                &self,
                app: &str,
                req: &CreateBot,
            ) -> Result<UserProfile, UserError> {
                self.inner.create_bot(app, req).await
            }
            async fn bot_owner(
                &self,
                app: &str,
                account: &str,
            ) -> Result<Option<String>, UserError> {
                self.inner.bot_owner(app, account).await
            }
            async fn delete_bot(
                &self,
                app: &str,
                owner: &str,
                account: &str,
            ) -> Result<(), UserError> {
                self.inner.delete_bot(app, owner, account).await
            }
            async fn count_bots(&self, app: &str, owner: &str) -> Result<u32, UserError> {
                self.inner.count_bots(app, owner).await
            }
            async fn lookup(
                &self,
                app: &str,
                account: &str,
            ) -> Result<Option<UserPresence>, UserError> {
                self.hits.fetch_add(1, Ordering::SeqCst);
                self.inner.lookup(app, account).await
            }
        }
        let inner = Arc::new(CountExists {
            inner: users,
            hits: hits.clone(),
        });
        let cache = CachedUserDirectory::wrap(inner);
        assert!(cache.exists("kim", "alice").await.unwrap());
        assert!(cache.exists("kim", "alice").await.unwrap());
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
