use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};
use moka::sync::Cache as MokaCache;
use std::time::Duration;

const SESSION_CAP: u64 = 200_000;
const LOC_CAP: u64 = 500_000;
const NEG_CAP: u64 = 100_000;
const POSITIVE_TTL: Duration = Duration::from_secs(60);
const NEGATIVE_TTL: Duration = Duration::from_secs(5);

/// Write-through location/session cache. Miss fill uses one inner
/// `get_locations` (Redis pipelines multi-account HVALS). Per-account loc
/// entries are filled from that call when there is a single miss; multiple
/// misses re-list so each account key stays correct (`Location` has no account).
///
/// Cross-instance writers (other Chat, Royal) `PUBLISH kim:loc:inv {account}`.
/// [`Self::invalidate_account`] is the receive side.
pub struct CachedSessionStore {
    inner: Arc<dyn SessionStorage>,
    sessions: MokaCache<String, Session>,
    locs: MokaCache<String, Arc<Vec<Location>>>,
    neg: MokaCache<String, ()>,
    /// account → channel_ids currently in `sessions`, so a loc-inv message
    /// can drop session rows without scanning the whole cache.
    channels: Mutex<HashMap<String, HashSet<String>>>,
    /// Bumped on every local write and invalidation. In-flight miss fills
    /// capture a stamp and refuse to install if it no longer matches.
    fill_epoch: Mutex<u64>,
}

impl CachedSessionStore {
    pub fn wrap(inner: Arc<dyn SessionStorage>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            sessions: MokaCache::builder()
                .max_capacity(SESSION_CAP)
                .time_to_live(POSITIVE_TTL)
                .eviction_listener(|key, _v, cause| {
                    tracing::debug!(key = %key, ?cause, "session cache eviction");
                })
                .build(),
            locs: MokaCache::builder()
                .max_capacity(LOC_CAP)
                .time_to_live(POSITIVE_TTL)
                .eviction_listener(|key, _v, cause| {
                    tracing::debug!(key = %key, ?cause, "loc cache eviction");
                })
                .build(),
            neg: MokaCache::builder()
                .max_capacity(NEG_CAP)
                .time_to_live(NEGATIVE_TTL)
                .build(),
            channels: Mutex::new(HashMap::new()),
            fill_epoch: Mutex::new(0),
        })
    }

    fn channels_lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, HashSet<String>>> {
        self.channels.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn track_channel(&self, account: &str, channel_id: &str) {
        self.channels_lock()
            .entry(account.to_string())
            .or_default()
            .insert(channel_id.to_string());
    }

    fn untrack_channel(&self, account: &str, channel_id: &str) {
        let mut idx = self.channels_lock();
        if let Some(set) = idx.get_mut(account) {
            set.remove(channel_id);
            if set.is_empty() {
                idx.remove(account);
            }
        }
    }

    fn fill_epoch_lock(&self) -> std::sync::MutexGuard<'_, u64> {
        self.fill_epoch.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fill_stamp(&self) -> u64 {
        *self.fill_epoch_lock()
    }

    fn bump_fill_epoch(&self) -> std::sync::MutexGuard<'_, u64> {
        let mut g = self.fill_epoch_lock();
        *g = g.wrapping_add(1);
        g
    }

    fn remember_locs(&self, account: String, slots: Vec<Location>, stamp: u64) {
        let _epoch = self.fill_epoch_lock();
        if *_epoch != stamp {
            return;
        }
        let chans: HashSet<String> = slots.iter().map(|l| l.channel_id.clone()).collect();
        self.channels_lock().insert(account.clone(), chans);
        self.neg.invalidate(&account);
        self.locs.insert(account, Arc::new(slots));
    }

    fn remember_empty(&self, account: String, stamp: u64) {
        let _epoch = self.fill_epoch_lock();
        if *_epoch != stamp {
            return;
        }
        self.channels_lock().remove(&account);
        self.locs.invalidate(&account);
        self.neg.insert(account, ());
    }

    /// Drop loc, negative, and session rows for `account`. Safe to call from
    /// the Redis subscriber and from tests.
    pub fn invalidate_account(&self, account: &str) {
        if account.is_empty() {
            return;
        }
        let _epoch = self.bump_fill_epoch();
        let channels = self.channels_lock().remove(account).unwrap_or_default();
        for ch in &channels {
            self.sessions.invalidate(ch);
        }
        if let Some(slots) = self.locs.get(account) {
            for loc in slots.iter() {
                self.sessions.invalidate(&loc.channel_id);
            }
        }
        self.locs.invalidate(account);
        self.neg.invalidate(account);
    }

    /// Subscriber reconnect: missed publishes, drop everything.
    pub fn invalidate_all(&self) {
        let _epoch = self.bump_fill_epoch();
        self.sessions.invalidate_all();
        self.locs.invalidate_all();
        self.neg.invalidate_all();
        self.channels_lock().clear();
    }
}

fn pick<'a>(slots: &'a [Location], device: &str) -> Option<&'a Location> {
    if device.is_empty() {
        slots.first()
    } else {
        slots.iter().find(|l| l.device == device)
    }
}

#[async_trait]
impl SessionStorage for CachedSessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        self.inner.add(session).await?;
        let _epoch = self.bump_fill_epoch();
        self.sessions
            .insert(session.channel_id.clone(), session.clone());
        self.track_channel(&session.account, &session.channel_id);
        self.locs.invalidate(&session.account);
        self.neg.invalidate(&session.account);
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        self.inner.delete(account, channel_id).await?;
        let _epoch = self.bump_fill_epoch();
        self.sessions.invalidate(channel_id);
        self.untrack_channel(account, channel_id);
        self.locs.invalidate(account);
        self.neg.invalidate(account);
        Ok(())
    }

    async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
        if let Some(s) = self.sessions.get(channel_id) {
            return Ok(s);
        }
        match self.inner.get(channel_id).await {
            Ok(s) => {
                self.track_channel(&s.account, channel_id);
                self.sessions.insert(channel_id.to_string(), s.clone());
                Ok(s)
            }
            Err(SessionError::NotFound) => Err(SessionError::NotFound),
            Err(e) => Err(e),
        }
    }

    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError> {
        let mut hits = Vec::new();
        let mut misses = Vec::new();
        for acc in accounts {
            if self.neg.get(acc).is_some() {
                continue;
            }
            match self.locs.get(acc) {
                Some(slots) => hits.extend(slots.iter().cloned()),
                None => misses.push(acc.clone()),
            }
        }
        if !misses.is_empty() {
            let stamp = self.fill_stamp();
            match self.inner.get_locations(&misses).await {
                Ok(slots) => {
                    hits.extend(slots.iter().cloned());
                    if misses.len() == 1 {
                        self.remember_locs(misses[0].clone(), slots, stamp);
                    } else {
                        for acc in misses {
                            match self.inner.list_locations(&acc).await {
                                Ok(v) => self.remember_locs(acc, v, stamp),
                                Err(SessionError::NotFound) => self.remember_empty(acc, stamp),
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Err(SessionError::NotFound) => {
                    for acc in misses {
                        self.remember_empty(acc, stamp);
                    }
                }
                Err(e) => return Err(e),
            }
        }
        if hits.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(hits)
        }
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        if let Some(slots) = self.locs.get(account) {
            if let Some(l) = pick(&slots, device) {
                return Ok(l.clone());
            }
            if !device.is_empty() {
                return Err(SessionError::NotFound);
            }
        }
        if self.neg.get(account).is_some() {
            return Err(SessionError::NotFound);
        }
        let stamp = self.fill_stamp();
        let slots = match self.inner.list_locations(account).await {
            Ok(s) => s,
            Err(SessionError::NotFound) => {
                self.remember_empty(account.to_string(), stamp);
                return Err(SessionError::NotFound);
            }
            Err(e) => return Err(e),
        };
        let loc = pick(&slots, device)
            .cloned()
            .ok_or(SessionError::NotFound)?;
        self.remember_locs(account.to_string(), slots, stamp);
        Ok(loc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySessionStore;

    fn session(channel_id: &str, account: &str, gate_id: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: gate_id.into(),
            account: account.into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn miss_fill_and_partial_hits() {
        let inner = Arc::new(MemorySessionStore::new());
        inner.add(&session("c1", "alice", "g")).await.unwrap();
        inner.add(&session("c2", "bob", "g")).await.unwrap();
        let cache = CachedSessionStore::wrap(inner);
        let locs = cache
            .get_locations(&["alice".into(), "bob".into(), "carol".into()])
            .await
            .unwrap();
        assert_eq!(locs.len(), 2);
        let again = cache.get_location("alice", "").await.unwrap();
        assert_eq!(again.channel_id, "c1");
    }

    #[tokio::test]
    async fn caches_two_locs_for_one_account() {
        let inner = Arc::new(MemorySessionStore::new());
        inner.add(&session("c1", "alice", "g")).await.unwrap();
        inner.add(&session("c2", "alice", "g")).await.unwrap();
        let cache = CachedSessionStore::wrap(inner);
        let locs = cache.list_locations("alice").await.unwrap();
        assert_eq!(locs.len(), 2);
    }

    #[tokio::test]
    async fn add_invalidates_neg_and_locs() {
        let inner = Arc::new(MemorySessionStore::new());
        let cache = CachedSessionStore::wrap(inner.clone());
        let err = cache.get_locations(&["alice".into()]).await.unwrap_err();
        assert!(matches!(err, SessionError::NotFound));
        cache.add(&session("c1", "alice", "g")).await.unwrap();
        let locs = cache.get_locations(&["alice".into()]).await.unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].channel_id, "c1");
    }

    #[tokio::test]
    async fn invalidate_account_drops_locs_and_sessions() {
        let inner = Arc::new(MemorySessionStore::new());
        inner.add(&session("c1", "alice", "g")).await.unwrap();
        let cache = CachedSessionStore::wrap(inner.clone());
        assert_eq!(cache.get("c1").await.unwrap().account, "alice");
        assert_eq!(
            cache.get_locations(&["alice".into()]).await.unwrap().len(),
            1
        );
        inner.delete("alice", "c1").await.unwrap();
        cache.invalidate_account("alice");
        assert!(matches!(
            cache.get_locations(&["alice".into()]).await,
            Err(SessionError::NotFound)
        ));
        assert!(matches!(cache.get("c1").await, Err(SessionError::NotFound)));
    }

    #[tokio::test]
    async fn other_fails_whole_call() {
        struct Boom;
        #[async_trait]
        impl SessionStorage for Boom {
            async fn add(&self, _: &Session) -> Result<(), SessionError> {
                Ok(())
            }
            async fn delete(&self, _: &str, _: &str) -> Result<(), SessionError> {
                Ok(())
            }
            async fn get(&self, _: &str) -> Result<Session, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
            async fn get_locations(&self, _: &[String]) -> Result<Vec<Location>, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
            async fn get_location(&self, _: &str, _: &str) -> Result<Location, SessionError> {
                Err(SessionError::Other("boom".into()))
            }
        }
        let cache = CachedSessionStore::wrap(Arc::new(Boom));
        let err = cache.get_locations(&["x".into()]).await.unwrap_err();
        assert!(matches!(err, SessionError::Other(_)));
    }

    #[tokio::test]
    async fn delayed_not_found_does_not_clobber_add() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use tokio::sync::oneshot;

        struct Gated {
            inner: MemorySessionStore,
            started: Mutex<Option<oneshot::Sender<()>>>,
            release: Mutex<Option<oneshot::Receiver<()>>>,
            first: AtomicBool,
        }

        #[async_trait]
        impl SessionStorage for Gated {
            async fn add(&self, session: &Session) -> Result<(), SessionError> {
                self.inner.add(session).await
            }
            async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
                self.inner.delete(account, channel_id).await
            }
            async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
                self.inner.get(channel_id).await
            }
            async fn get_locations(
                &self,
                accounts: &[String],
            ) -> Result<Vec<Location>, SessionError> {
                if self.first.swap(false, Ordering::SeqCst) {
                    let started = self
                        .started
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    if let Some(tx) = started {
                        let _ = tx.send(());
                    }
                    let release = self
                        .release
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    if let Some(rx) = release {
                        let _ = rx.await;
                    }
                    return Err(SessionError::NotFound);
                }
                self.inner.get_locations(accounts).await
            }
            async fn get_location(
                &self,
                account: &str,
                device: &str,
            ) -> Result<Location, SessionError> {
                self.inner.get_location(account, device).await
            }
        }

        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let inner = Gated {
            inner: MemorySessionStore::new(),
            started: Mutex::new(Some(started_tx)),
            release: Mutex::new(Some(release_rx)),
            first: AtomicBool::new(true),
        };
        let cache = CachedSessionStore::wrap(Arc::new(inner));
        let lookup = {
            let cache = cache.clone();
            tokio::spawn(async move { cache.get_locations(&["alice".into()]).await })
        };
        started_rx.await.unwrap();
        cache.add(&session("c1", "alice", "g")).await.unwrap();
        let _ = release_tx.send(());
        let _ = lookup.await.unwrap();
        let locs = cache.get_locations(&["alice".into()]).await.unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].channel_id, "c1");
    }
}
