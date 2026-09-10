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
        // Only announce when transitioning OFFLINE→ONLINE (sole location).
        if locs.len() != 1 {
            return;
        }
        self.fanout(app, account, PresenceStatus::PresenceOnline, 0)
            .await;
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
                let locs = match storage.list_locations(&account).await {
                    Ok(v) => v,
                    Err(SessionError::NotFound) => Vec::new(),
                    Err(err) => {
                        warn!(%err, account, "presence debounce list_locations");
                        Vec::new()
                    }
                };
                if locs.is_empty() {
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
    use kim_router::{RouterError, SessionError, SessionStorage};
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
}
