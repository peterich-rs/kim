use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use kim_core::{OpCode, Server};
use kim_naming::{DefaultRegistration, Naming};
use kim_protocol::{marshal, read_logic, LogicPkt, Packet, META_DEST_CHANNELS, META_DEST_SERVER};
use kim_tcp::{ClientOptions, TcpClient, TcpDialer};
use tokio::sync::{Mutex, Notify, RwLock};
use tracing::{info, warn};

const RECONNECT_INTERVAL: Duration = Duration::from_millis(500);

use crate::client_map::{ClientMap, ClientSlot, ADULT, YOUNG};
use crate::error::Error;
use crate::selector::{HashSelector, Selector};

/// 本进程 `Server::push` 成功之后、同一任务调用。
#[async_trait]
pub trait DownlinkHook: Send + Sync {
    async fn after_push(&self, channel_id: &str, pkt: &LogicPkt);
}

pub struct ContainerOpts {
    pub naming: Arc<dyn Naming>,
    pub identity: DefaultRegistration,
    pub dialer: Arc<dyn TcpDialer>,
    pub deps: Vec<String>,
    pub adult_delay: Duration,
    pub selector: Arc<dyn Selector>,
    pub after_downlink: Vec<Arc<dyn DownlinkHook>>,
}

impl ContainerOpts {
    pub fn with_defaults(
        naming: Arc<dyn Naming>,
        identity: DefaultRegistration,
        dialer: Arc<dyn TcpDialer>,
        deps: Vec<String>,
    ) -> Self {
        Self {
            naming,
            identity,
            dialer,
            deps,
            adult_delay: Duration::from_secs(10),
            selector: Arc::new(HashSelector),
            after_downlink: Vec::new(),
        }
    }
}

pub struct Container {
    naming: Arc<dyn Naming>,
    server: OnceLock<Arc<dyn Server + Send + Sync>>,
    identity: DefaultRegistration,
    dialer: Arc<dyn TcpDialer>,
    deps: Vec<String>,
    clients: Arc<RwLock<HashMap<String, ClientMap>>>,
    selector: Arc<dyn Selector>,
    adult_delay: Duration,
    after_downlink: Vec<Arc<dyn DownlinkHook>>,
    /// Last non-empty TCP catalog per dep. Dial failure / peer restart
    /// must not require Consul's passing set to change.
    wanted: RwLock<HashMap<String, HashMap<String, DefaultRegistration>>>,
    dialing: Mutex<HashSet<String>>,
    wake: Notify,
    finished: Notify,
    closed: AtomicBool,
    done: AtomicBool,
}

impl Container {
    pub fn new(opts: ContainerOpts) -> Arc<Self> {
        Arc::new(Self {
            naming: opts.naming,
            server: OnceLock::new(),
            identity: opts.identity,
            dialer: opts.dialer,
            deps: opts.deps,
            clients: Arc::new(RwLock::new(HashMap::new())),
            selector: opts.selector,
            adult_delay: opts.adult_delay,
            after_downlink: opts.after_downlink,
            wanted: RwLock::new(HashMap::new()),
            dialing: Mutex::new(HashSet::new()),
            wake: Notify::new(),
            finished: Notify::new(),
            closed: AtomicBool::new(false),
            done: AtomicBool::new(false),
        })
    }

    pub fn attach_server(&self, server: Arc<dyn Server + Send + Sync>) {
        let _ = self.server.set(server);
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), Error> {
        let srv = self
            .server
            .get()
            .ok_or_else(|| Error::other("attach_server first"))?
            .clone();
        for name in &self.deps {
            self.connect_to_service(name).await?;
        }
        if self.closed.load(Ordering::SeqCst) {
            self.wait_finished().await;
            return Ok(());
        }
        if !self.identity.public_address.is_empty() {
            self.naming.register(self.identity.clone()).await?;
        }
        if self.closed.load(Ordering::SeqCst) {
            if !self.identity.public_address.is_empty() {
                let _ = self.naming.deregister(&self.identity.service_id).await;
            }
            self.wait_finished().await;
            return Ok(());
        }
        let srv2 = srv.clone();
        tokio::spawn(async move {
            let _ = srv2.start().await;
        });
        if self.closed.load(Ordering::SeqCst) {
            self.wait_finished().await;
            return Ok(());
        }
        self.wait_finished().await;
        Ok(())
    }

    async fn wait_finished(&self) {
        let notified = self.finished.notified();
        if self.done.load(Ordering::SeqCst) {
            return;
        }
        notified.await;
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        if self.closed.swap(true, Ordering::SeqCst) {
            self.wait_finished().await;
            return Ok(());
        }
        self.wake.notify_waiters();
        // G-07/G-32: deregister first so Router/Consul stop sending new clients,
        // then stop accept, bounded drain, close connections.
        if !self.identity.public_address.is_empty() {
            let _ = self.naming.deregister(&self.identity.service_id).await;
        }
        for name in &self.deps {
            let _ = self.naming.unsubscribe(name).await;
        }
        if let Some(srv) = self.server.get() {
            let _ = srv.shutdown().await;
        }
        let outbound: Vec<Arc<TcpClient>> = {
            let mut map = self.clients.write().await;
            map.values_mut()
                .flat_map(|cmap| {
                    cmap.ids()
                        .into_iter()
                        .filter_map(|id| cmap.remove(&id).map(|s| s.client))
                })
                .collect()
        };
        for client in outbound {
            let _ = client.shutdown().await;
        }
        self.done.store(true, Ordering::SeqCst);
        self.finished.notify_waiters();
        Ok(())
    }

    pub async fn forward(&self, service_name: &str, mut pkt: LogicPkt) -> Result<(), Error> {
        if pkt.header.channel_id.is_empty() || pkt.header.command.is_empty() {
            return Err(Error::other("empty command or channel_id"));
        }
        pkt.set_meta(META_DEST_SERVER, &self.identity.service_id);
        let map = self.clients.read().await;
        let Some(cmap) = map.get(service_name) else {
            return Err(Error::other("no adult instances"));
        };
        let adult = cmap.adult_services();
        if adult.is_empty() {
            return Err(Error::other("no adult instances"));
        }
        let id = self
            .selector
            .lookup(&pkt.header, &adult)
            .ok_or_else(|| Error::other("no adult instances"))?;
        let client = cmap
            .get(&id)
            .ok_or_else(|| Error::other("selected instance missing"))?
            .client
            .clone();
        drop(map);
        client
            .send(marshal(&Packet::Logic(pkt)))
            .await
            .map_err(Error::from)
    }

    pub async fn push(&self, gateway_id: &str, pkt: LogicPkt) -> Result<(), Error> {
        let srv = self
            .server
            .get()
            .ok_or_else(|| Error::other("no server"))?
            .clone();
        srv.push(gateway_id, marshal(&Packet::Logic(pkt)))
            .await
            .map_err(Error::from)
    }

    pub async fn slot_state(&self, service_name: &str, id: &str) -> Option<u8> {
        let map = self.clients.read().await;
        map.get(service_name)
            .and_then(|m| m.get(id))
            .map(|s| s.state.load(Ordering::SeqCst))
    }

    async fn connect_to_service(self: &Arc<Self>, name: &str) -> Result<(), Error> {
        let this = self.clone();
        let svc = name.to_string();
        self.naming
            .subscribe(
                name,
                Arc::new(move |list: Vec<DefaultRegistration>| {
                    let this = this.clone();
                    let svc = svc.clone();
                    tokio::spawn(async move {
                        this.apply_snapshot(&svc, list).await;
                    });
                }),
            )
            .await?;

        let found = self.naming.find(name, &[]).await?;
        self.apply_snapshot(name, found).await;

        let this = self.clone();
        let svc = name.to_string();
        tokio::spawn(async move {
            this.reconnect_loop(svc).await;
        });
        Ok(())
    }

    async fn reconnect_loop(self: Arc<Self>, service: String) {
        loop {
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(RECONNECT_INTERVAL) => {}
            }
            if self.closed.load(Ordering::SeqCst) {
                return;
            }
            self.reconnect_missing(&service).await;
        }
    }

    async fn reconnect_missing(self: &Arc<Self>, service: &str) {
        let regs: Vec<DefaultRegistration> = {
            let wanted = self.wanted.read().await;
            wanted
                .get(service)
                .map(|m| m.values().cloned().collect())
                .unwrap_or_default()
        };
        for reg in regs {
            let id = reg.service_id.clone();
            if let Ok(true) = self.build_client(service, reg).await {
                self.schedule_promote(service, &id);
            }
        }
    }

    fn schedule_promote(self: &Arc<Self>, service: &str, id: &str) {
        let this = self.clone();
        let svc = service.to_string();
        let id = id.to_string();
        if this.adult_delay.is_zero() {
            tokio::spawn(async move {
                this.force_adult(&svc, &id).await;
            });
            return;
        }
        tokio::spawn(async move {
            tokio::time::sleep(this.adult_delay).await;
            this.try_promote(&svc, &id).await;
        });
    }

    async fn apply_snapshot(self: &Arc<Self>, service_name: &str, list: Vec<DefaultRegistration>) {
        let tcp: Vec<DefaultRegistration> =
            list.into_iter().filter(|r| r.protocol == "tcp").collect();
        // Empty passing set is the first watch vs Find race, or a brief
        // Consul blip. Keep the last non-empty wanted set so a restart
        // that does not change catalog still redials.
        if !tcp.is_empty() {
            let mut wanted = self.wanted.write().await;
            let map = wanted.entry(service_name.to_string()).or_default();
            map.clear();
            for reg in &tcp {
                map.insert(reg.service_id.clone(), reg.clone());
            }
        }
        for reg in tcp {
            let id = reg.service_id.clone();
            if let Ok(true) = self.build_client(service_name, reg).await {
                self.schedule_promote(service_name, &id);
            }
        }
        let wanted_ids: HashSet<String> = {
            let wanted = self.wanted.read().await;
            wanted
                .get(service_name)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default()
        };
        if wanted_ids.is_empty() {
            return;
        }
        let stale: Vec<(String, Arc<TcpClient>)> = {
            let map = self.clients.read().await;
            match map.get(service_name) {
                Some(cmap) => cmap
                    .ids()
                    .into_iter()
                    .filter(|id| !wanted_ids.contains(id))
                    .filter_map(|id| cmap.get(&id).map(|s| (id, s.client.clone())))
                    .collect(),
                None => Vec::new(),
            }
        };
        for (id, client) in stale {
            let _ = client.shutdown().await;
            let mut w = self.clients.write().await;
            if let Some(cmap) = w.get_mut(service_name) {
                if cmap
                    .get(&id)
                    .is_some_and(|s| Arc::ptr_eq(&s.client, &client))
                {
                    cmap.remove(&id);
                }
            }
        }
    }

    async fn try_promote(&self, service_name: &str, id: &str) {
        let map = self.clients.read().await;
        if let Some(slot) = map.get(service_name).and_then(|m| m.get(id)) {
            let _ = slot
                .state
                .compare_exchange(YOUNG, ADULT, Ordering::SeqCst, Ordering::SeqCst);
        }
    }

    async fn force_adult(&self, service_name: &str, id: &str) {
        let map = self.clients.read().await;
        if let Some(slot) = map.get(service_name).and_then(|m| m.get(id)) {
            slot.state.store(ADULT, Ordering::SeqCst);
        }
    }

    fn dial_key(service: &str, id: &str) -> String {
        format!("{service}/{id}")
    }

    async fn claim_dial(&self, service: &str, id: &str) -> bool {
        {
            let map = self.clients.read().await;
            if map.get(service).is_some_and(|m| m.contains(id)) {
                return false;
            }
        }
        let mut dialing = self.dialing.lock().await;
        dialing.insert(Self::dial_key(service, id))
    }

    async fn release_dial(&self, service: &str, id: &str) {
        self.dialing
            .lock()
            .await
            .remove(&Self::dial_key(service, id));
    }

    async fn build_client(
        self: &Arc<Self>,
        service_name: &str,
        reg: DefaultRegistration,
    ) -> Result<bool, Error> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(false);
        }
        if reg.protocol != "tcp" {
            warn!(id = %reg.service_id, "skip non-tcp");
            return Ok(false);
        }
        let id = reg.service_id.clone();
        if !self.claim_dial(service_name, &id).await {
            return Ok(false);
        }
        let result = self.dial_and_insert(service_name, reg).await;
        self.release_dial(service_name, &id).await;
        result
    }

    async fn dial_and_insert(
        self: &Arc<Self>,
        service_name: &str,
        reg: DefaultRegistration,
    ) -> Result<bool, Error> {
        let id = reg.service_id.clone();
        {
            let map = self.clients.read().await;
            if map.get(service_name).is_some_and(|m| m.contains(&id)) {
                return Ok(false);
            }
        }
        let mut client = TcpClient::new(
            id.clone(),
            service_name.to_string(),
            ClientOptions::default(),
        );
        client.set_dialer(self.dialer.clone());
        if let Err(e) = client.connect(&reg.dial_url()).await {
            warn!(id = %id, %e, "dial failed");
            return Ok(false);
        }
        let client = Arc::new(client);
        let c2 = client.clone();
        let this = self.clone();
        tokio::spawn(async move {
            this.read_loop(c2).await;
        });
        let mut w = self.clients.write().await;
        let cmap = w.entry(service_name.to_string()).or_default();
        if cmap.contains(&id) {
            drop(w);
            let _ = client.shutdown().await;
            return Ok(false);
        }
        cmap.insert(ClientSlot {
            reg,
            client,
            state: Arc::new(AtomicU8::new(YOUNG)),
        });
        info!(service = %service_name, id = %id, "dialed young");
        Ok(true)
    }

    async fn read_loop(self: Arc<Self>, client: Arc<TcpClient>) {
        let id = client.id().to_string();
        let service = client.name().to_string();
        loop {
            match client.read().await {
                Ok(frame) if matches!(frame.opcode, OpCode::Binary | OpCode::Text) => {
                    if let Err(err) = self.deliver_down(frame.payload).await {
                        warn!(%err, "deliver down");
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    warn!(id = %id, service = %service, %err, "upstream closed");
                    break;
                }
            }
        }
        {
            let mut w = self.clients.write().await;
            if let Some(cmap) = w.get_mut(&service) {
                if cmap
                    .get(&id)
                    .is_some_and(|s| Arc::ptr_eq(&s.client, &client))
                {
                    cmap.remove(&id);
                }
            }
        }
        self.wake.notify_waiters();
    }

    async fn deliver_down(&self, payload: Bytes) -> Result<(), Error> {
        let mut pkt = read_logic(&payload)?;
        let dest = pkt.get_meta(META_DEST_SERVER).unwrap_or("").to_string();
        if dest != self.identity.service_id {
            warn!(%dest, me = %self.identity.service_id, "drop pkt for other gateway");
            return Ok(());
        }
        let channels = pkt.get_meta(META_DEST_CHANNELS).unwrap_or("").to_string();
        pkt.del_meta(META_DEST_SERVER);
        pkt.del_meta(META_DEST_CHANNELS);
        let hook_pkt = if self.after_downlink.is_empty() {
            None
        } else {
            Some(LogicPkt {
                header: pkt.header.clone(),
                body: pkt.body.clone(),
            })
        };
        let bytes = marshal(&Packet::Logic(pkt));
        let srv = self
            .server
            .get()
            .ok_or_else(|| Error::other("no server"))?
            .clone();
        for id in channels.split(',').filter(|s| !s.is_empty()) {
            match srv.push(id, bytes.clone()).await {
                Ok(()) => {
                    if let Some(p) = &hook_pkt {
                        for h in &self.after_downlink {
                            h.after_push(id, p).await;
                        }
                    }
                }
                Err(e) => warn!(%e, channel = %id, "push down failed"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    use crate::selector::HashSelector;
    use crate::InnerTcpDialer;
    use kim_core::Error as CoreError;
    use kim_naming::StaticNaming;
    use tokio::sync::Notify as TokioNotify;

    struct Rec {
        log: Arc<StdMutex<Vec<&'static str>>>,
    }

    struct RecordingNaming {
        inner: StaticNaming,
        rec: Rec,
    }

    #[async_trait]
    impl Naming for RecordingNaming {
        async fn find(
            &self,
            service_name: &str,
            tags: &[&str],
        ) -> Result<Vec<DefaultRegistration>, kim_naming::Error> {
            self.inner.find(service_name, tags).await
        }

        async fn subscribe(
            &self,
            service_name: &str,
            callback: Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>,
        ) -> Result<(), kim_naming::Error> {
            self.inner.subscribe(service_name, callback).await
        }

        async fn unsubscribe(&self, service_name: &str) -> Result<(), kim_naming::Error> {
            self.rec.log.lock().unwrap().push("unsubscribe");
            self.inner.unsubscribe(service_name).await
        }

        async fn register(&self, service: DefaultRegistration) -> Result<(), kim_naming::Error> {
            self.inner.register(service).await
        }

        async fn deregister(&self, service_id: &str) -> Result<(), kim_naming::Error> {
            self.rec.log.lock().unwrap().push("deregister");
            self.inner.deregister(service_id).await
        }
    }

    struct BlockingServer {
        rec: Rec,
        entered: Arc<TokioNotify>,
        release: Arc<TokioNotify>,
    }

    #[async_trait]
    impl Server for BlockingServer {
        fn set_acceptor(&mut self, _acceptor: Arc<dyn kim_core::Acceptor>) {}
        fn set_message_listener(&mut self, _listener: Arc<dyn kim_core::MessageListener>) {}
        fn set_state_listener(&mut self, _listener: Arc<dyn kim_core::StateListener>) {}
        fn set_read_wait(&mut self, _wait: Duration) {}

        async fn start(&self) -> Result<(), CoreError> {
            Ok(())
        }
        async fn push(&self, _channel_id: &str, _payload: Bytes) -> Result<(), CoreError> {
            Ok(())
        }
        async fn close_channel(&self, _channel_id: &str) -> Result<(), CoreError> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<(), CoreError> {
            self.rec.log.lock().unwrap().push("server_shutdown");
            self.entered.notify_waiters();
            self.release.notified().await;
            Ok(())
        }
    }

    fn ident() -> DefaultRegistration {
        DefaultRegistration {
            service_id: "gw-1".into(),
            service_name: "wgateway".into(),
            protocol: "ws".into(),
            public_address: "127.0.0.1".into(),
            public_port: 8001,
            tags: vec![],
            meta: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn shutdown_deregisters_before_server() {
        let log = Arc::new(StdMutex::new(Vec::new()));
        let rec = Rec { log: log.clone() };
        let naming = Arc::new(RecordingNaming {
            inner: StaticNaming::from_slice(vec![]),
            rec: Rec { log: log.clone() },
        });
        Naming::register(naming.as_ref(), ident()).await.unwrap();
        assert_eq!(naming.find("wgateway", &[]).await.unwrap().len(), 1);

        let entered = Arc::new(TokioNotify::new());
        let release = Arc::new(TokioNotify::new());
        let server = Arc::new(BlockingServer {
            rec,
            entered: entered.clone(),
            release: release.clone(),
        });

        let c = Container::new(ContainerOpts {
            naming: naming.clone(),
            identity: ident(),
            dialer: Arc::new(InnerTcpDialer {
                local_service_id: "gw-1".into(),
            }),
            deps: vec![],
            adult_delay: Duration::ZERO,
            selector: Arc::new(HashSelector),
            after_downlink: vec![],
        });
        c.attach_server(server);

        let run = {
            let c = c.clone();
            tokio::spawn(async move { c.shutdown().await })
        };
        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .expect("server shutdown should start");
        assert_eq!(
            log.lock().unwrap().as_slice(),
            ["deregister", "server_shutdown"]
        );
        assert!(
            naming.find("wgateway", &[]).await.unwrap().is_empty(),
            "lookup must not return this instance after deregister, while drain still runs"
        );
        release.notify_waiters();
        run.await.unwrap().unwrap();
    }
}
