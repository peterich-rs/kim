//! Presence production: location add/remove → fanout `chat.presence` to room interest.
//! Heartbeat must not call into this module.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kim_protocol::pkt::{Presence, PresencePush, PresenceStatus};
use kim_protocol::{CMD_PRESENCE, INBOX_KIND_USER};
use kim_router::{Dispatcher, Location, SessionError, SessionStorage};
use tracing::warn;

use crate::interest::RoomInterestStore;
use crate::notify::notify_locations;

/// Default offline debounce (45s within the 30–60s design window).
pub const DEFAULT_OFFLINE_DEBOUNCE: Duration = Duration::from_secs(45);

pub fn offline_debounce_from_env() -> Duration {
    match std::env::var("KIM_PRESENCE_OFFLINE_DEBOUNCE_MS") {
        Ok(s) => match s.trim().parse::<u64>() {
            Ok(ms) if ms > 0 => Duration::from_millis(ms),
            _ => DEFAULT_OFFLINE_DEBOUNCE,
        },
        _ => DEFAULT_OFFLINE_DEBOUNCE,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub struct PresenceHub {
    interest: Arc<dyn RoomInterestStore>,
    storage: Arc<dyn SessionStorage>,
    dispatcher: Arc<dyn Dispatcher>,
    debounce: Duration,
    /// `app:account` → generation (bumped to cancel stale offline tasks).
    gens: Arc<Mutex<HashMap<String, u64>>>,
}

impl PresenceHub {
    pub fn new(
        interest: Arc<dyn RoomInterestStore>,
        storage: Arc<dyn SessionStorage>,
        dispatcher: Arc<dyn Dispatcher>,
        debounce: Duration,
    ) -> Self {
        Self {
            interest,
            storage,
            dispatcher,
            debounce,
            gens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn interest(&self) -> &Arc<dyn RoomInterestStore> {
        &self.interest
    }

    pub fn debounce(&self) -> Duration {
        self.debounce
    }

    fn gen_key(app: &str, account: &str) -> String {
        format!("{app}:{account}")
    }

    fn bump_gen(&self, app: &str, account: &str) -> u64 {
        let mut g = self.gens.lock().unwrap_or_else(|e| e.into_inner());
        let e = g.entry(Self::gen_key(app, account)).or_insert(0);
        *e += 1;
        *e
    }

    /// After a location was added: cancel pending OFFLINE; if this was the first
    /// location, fanout ONLINE to interested viewers.
    pub async fn on_location_added(&self, app: &str, account: &str) {
        let _ = self.bump_gen(app, account);
        let locs = match self.storage.list_locations(account).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => Vec::new(),
            Err(err) => {
                warn!(%err, account, "presence list_locations after add");
                return;
            }
        };
        if locs.len() == 1 {
            self.fanout(app, account, PresenceStatus::PresenceOnline, 0)
                .await;
            return;
        }
        if locs.len() < 2 {
            return;
        }
        // Login often wins the race against the dying channel's logout, so we
        // briefly see old+new locations and would skip ONLINE. Spawn a short
        // poll (must not await here — that would block the chat handler and
        // prevent the logout from running). If we settle on one location,
        // announce ONLINE. Keep the window short so a later multi-device leave
        // cannot be mistaken for this reconnect edge.
        let app = app.to_string();
        let account = account.to_string();
        let interest = self.interest.clone();
        let storage = self.storage.clone();
        let dispatcher = self.dispatcher.clone();
        let gens = self.gens.clone();
        let debounce = self.debounce;
        tokio::spawn(async move {
            for _ in 0..10 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                match storage.list_locations(&account).await {
                    Ok(locs) if locs.len() == 1 => {
                        let hub = PresenceHub {
                            interest,
                            storage,
                            dispatcher,
                            debounce,
                            gens,
                        };
                        hub.fanout(&app, &account, PresenceStatus::PresenceOnline, 0)
                            .await;
                        return;
                    }
                    Ok(locs) if locs.len() > 1 => continue,
                    Ok(_) | Err(SessionError::NotFound) => return,
                    Err(err) => {
                        warn!(%err, account, "presence reconnect list_locations");
                        return;
                    }
                }
            }
        });
    }

    /// After a location was removed: clear this channel's room interest, then
    /// start offline debounce if no locations remain.
    pub async fn on_location_removed(&self, app: &str, account: &str, channel_id: &str) {
        if let Err(err) = self.interest.clear_channel(app, account, channel_id).await {
            warn!(%err, account, channel_id, "clear room interest failed");
        }
        let locs = match self.storage.list_locations(account).await {
            Ok(v) => v,
            Err(SessionError::NotFound) => Vec::new(),
            Err(err) => {
                warn!(%err, account, "presence list_locations after remove");
                return;
            }
        };
        if !locs.is_empty() {
            let _ = self.bump_gen(app, account);
            return;
        }
        let gen = self.bump_gen(app, account);
        let app = app.to_string();
        let account = account.to_string();
        let interest = self.interest.clone();
        let storage = self.storage.clone();
        let dispatcher = self.dispatcher.clone();
        let gens = self.gens.clone();
        let debounce = self.debounce;
        tokio::spawn(async move {
            tokio::time::sleep(debounce).await;
            let key = format!("{app}:{account}");
            let still = {
                let g = gens.lock().unwrap_or_else(|e| e.into_inner());
                g.get(&key).copied().unwrap_or(0) == gen
            };
            if still {
                match storage.list_locations(&account).await {
                    Ok(locs) if !locs.is_empty() => {}
                    Ok(_) | Err(SessionError::NotFound) => {
                        let hub = PresenceHub {
                            interest,
                            storage,
                            dispatcher,
                            debounce,
                            gens: gens.clone(),
                        };
                        hub.fanout(&app, &account, PresenceStatus::PresenceOffline, now_ms())
                            .await;
                    }
                    Err(err) => {
                        warn!(%err, account, "presence debounce list_locations");
                    }
                }
            }
            let mut g = gens.lock().unwrap_or_else(|e| e.into_inner());
            if g.get(&key).copied() == Some(gen) {
                g.remove(&key);
            }
        });
    }

    async fn fanout(&self, app: &str, account: &str, status: PresenceStatus, last_seen: i64) {
        let viewers = match self.interest.viewers(app, account, INBOX_KIND_USER).await {
            Ok(v) => v,
            Err(err) => {
                warn!(%err, account, "list room viewers failed");
                return;
            }
        };
        if viewers.is_empty() {
            return;
        }
        let body = PresencePush {
            entries: vec![Presence {
                account: account.to_string(),
                status: status as i32,
                last_seen,
            }],
        };
        let mut by_account: HashMap<String, Vec<String>> = HashMap::new();
        for v in viewers {
            by_account.entry(v.account).or_default().push(v.channel_id);
        }
        for (viewer, channels) in by_account {
            let locs = match self.storage.list_locations(&viewer).await {
                Ok(v) => v,
                Err(SessionError::NotFound) => continue,
                Err(err) => {
                    warn!(%err, viewer, "presence viewer locations");
                    continue;
                }
            };
            let want: HashSet<&str> = channels.iter().map(|c| c.as_str()).collect();
            let recvs: Vec<Location> = locs
                .into_iter()
                .filter(|l| want.contains(l.channel_id.as_str()))
                .collect();
            if recvs.is_empty() {
                continue;
            }
            if let Err(err) =
                notify_locations(self.dispatcher.as_ref(), "", CMD_PRESENCE, &body, &recvs).await
            {
                warn!(%err, viewer, "presence fanout failed");
            }
        }
    }

    pub fn snapshot_status(locs: &[Location]) -> PresenceStatus {
        if locs.is_empty() {
            PresenceStatus::PresenceOffline
        } else {
            PresenceStatus::PresenceOnline
        }
    }

    pub fn presence_of(account: &str, locs: &[Location]) -> Presence {
        Presence {
            account: account.to_string(),
            status: Self::snapshot_status(locs) as i32,
            last_seen: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn gens_len(&self) -> usize {
        self.gens.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interest::MemoryRoomInterest;
    use async_trait::async_trait;
    use kim_protocol::pkt::Session;
    use kim_protocol::LogicPkt;
    use kim_router::{RouterError, SessionStorage};
    use kim_session::MemorySessionStore;

    struct NoopDisp;

    #[async_trait]
    impl Dispatcher for NoopDisp {
        async fn push(
            &self,
            _gateway: &str,
            _channels: &[String],
            _pkt: LogicPkt,
        ) -> Result<(), RouterError> {
            Ok(())
        }
    }

    fn session(channel_id: &str, account: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: "g".into(),
            account: account.into(),
            app: "kim".into(),
            ..Session::default()
        }
    }

    #[tokio::test]
    async fn gens_do_not_grow_across_offline_cycles() {
        let storage = Arc::new(MemorySessionStore::new());
        let hub = PresenceHub::new(
            Arc::new(MemoryRoomInterest::new()),
            storage.clone(),
            Arc::new(NoopDisp),
            Duration::from_millis(30),
        );
        for i in 0..8 {
            let ch = format!("c{i}");
            storage.add(&session(&ch, "alice")).await.unwrap();
            hub.on_location_added("kim", "alice").await;
            storage.delete("alice", &ch).await.unwrap();
            hub.on_location_removed("kim", "alice", &ch).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            hub.gens_len() <= 1,
            "gens leaked across cycles: {}",
            hub.gens_len()
        );
    }

    #[tokio::test]
    async fn reconnect_race_still_announces_online() {
        use kim_protocol::INBOX_KIND_USER;
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        struct RecDisp {
            pushes: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Dispatcher for RecDisp {
            async fn push(
                &self,
                _gateway: &str,
                _channels: &[String],
                _pkt: LogicPkt,
            ) -> Result<(), RouterError> {
                self.pushes.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(())
            }
        }

        let storage = Arc::new(MemorySessionStore::new());
        let interest = Arc::new(MemoryRoomInterest::new());
        interest
            .enter("kim", "viewer", "ch-v", "alice", INBOX_KIND_USER)
            .await
            .unwrap();
        storage.add(&session("ch-v", "viewer")).await.unwrap();
        storage.add(&session("ch-old", "alice")).await.unwrap();

        let pushes = Arc::new(AtomicUsize::new(0));
        let hub = PresenceHub::new(
            interest,
            storage.clone(),
            Arc::new(RecDisp {
                pushes: pushes.clone(),
            }),
            Duration::from_millis(500),
        );

        // Login wins: new location added while old still present.
        storage.add(&session("ch-new", "alice")).await.unwrap();
        hub.on_location_added("kim", "alice").await;
        assert_eq!(pushes.load(AtomicOrdering::SeqCst), 0);

        // Dying channel logout lands shortly after.
        storage.delete("alice", "ch-old").await.unwrap();
        hub.on_location_removed("kim", "alice", "ch-old").await;

        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(
            pushes.load(AtomicOrdering::SeqCst),
            1,
            "reconnect race must still fanout ONLINE once old location is gone"
        );
    }

    #[tokio::test]
    async fn debounce_storage_error_does_not_announce_offline() {
        use kim_protocol::INBOX_KIND_USER;
        use kim_router::SessionError;
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        struct FailOnNth {
            inner: MemorySessionStore,
            calls: AtomicUsize,
            fail_at: usize,
        }

        #[async_trait]
        impl SessionStorage for FailOnNth {
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
                let n = self.calls.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                if n == self.fail_at {
                    return Err(SessionError::Other("transient".into()));
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

        struct RecDisp {
            pushes: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl Dispatcher for RecDisp {
            async fn push(
                &self,
                _gateway: &str,
                _channels: &[String],
                _pkt: LogicPkt,
            ) -> Result<(), RouterError> {
                self.pushes.fetch_add(1, AtomicOrdering::SeqCst);
                Ok(())
            }
        }

        let storage = Arc::new(FailOnNth {
            inner: MemorySessionStore::new(),
            calls: AtomicUsize::new(0),
            fail_at: 2,
        });
        let interest = Arc::new(MemoryRoomInterest::new());
        interest
            .enter("kim", "viewer", "ch-v", "alice", INBOX_KIND_USER)
            .await
            .unwrap();
        storage.add(&session("ch-v", "viewer")).await.unwrap();
        storage.add(&session("ch-a", "alice")).await.unwrap();
        storage.delete("alice", "ch-a").await.unwrap();

        let pushes = Arc::new(AtomicUsize::new(0));
        let hub = PresenceHub::new(
            interest,
            storage,
            Arc::new(RecDisp {
                pushes: pushes.clone(),
            }),
            Duration::from_millis(30),
        );
        hub.on_location_removed("kim", "alice", "ch-a").await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(
            pushes.load(AtomicOrdering::SeqCst),
            0,
            "storage errors must not fanout OFFLINE"
        );
        assert_eq!(hub.gens_len(), 0, "generation must still be cleaned up");
    }
}
