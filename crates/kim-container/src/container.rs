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
use tokio::sync::{Notify, RwLock};
use tracing::{info, warn};

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
    shutdown: Notify,
    closed: AtomicBool,
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
            shutdown: Notify::new(),
            closed: AtomicBool::new(false),
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
        if !self.identity.public_address.is_empty() {
            self.naming.register(self.identity.clone()).await?;
        }
        if self.closed.load(Ordering::SeqCst) {
            let _ = srv.shutdown().await;
            return Ok(());
        }
        let srv2 = srv.clone();
        tokio::spawn(async move {
            let _ = srv2.start().await;
        });
        if self.closed.load(Ordering::SeqCst) {
            let _ = srv.shutdown().await;
            return Ok(());
        }
        self.shutdown.notified().await;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        self.closed.store(true, Ordering::SeqCst);
        self.shutdown.notify_waiters();
        self.shutdown.notify_one();
        if let Some(srv) = self.server.get() {
            let _ = srv.shutdown().await;
        }
        for name in &self.deps {
            let _ = self.naming.unsubscribe(name).await;
        }
        if !self.identity.public_address.is_empty() {
            let _ = self.naming.deregister(&self.identity.service_id).await;
        }
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
        for reg in found {
            let id = reg.service_id.clone();
            let _ = self.build_client(name, reg).await;
            self.force_adult(name, &id).await;
        }
        Ok(())
    }

    async fn apply_snapshot(self: &Arc<Self>, service_name: &str, list: Vec<DefaultRegistration>) {
        let wanted: HashSet<String> = list
            .iter()
            .filter(|r| r.protocol == "tcp")
            .map(|r| r.service_id.clone())
            .collect();
        for reg in list {
            let id = reg.service_id.clone();
            if let Ok(true) = self.build_client(service_name, reg).await {
                let this = self.clone();
                let svc = service_name.to_string();
                tokio::spawn(async move {
                    tokio::time::sleep(this.adult_delay).await;
                    this.try_promote(&svc, &id).await;
                });
            }
        }
        let stale: Vec<(String, Arc<TcpClient>)> = {
            let map = self.clients.read().await;
            match map.get(service_name) {
                Some(cmap) => cmap
                    .ids()
                    .into_iter()
                    .filter(|id| !wanted.contains(id))
                    .filter_map(|id| cmap.get(&id).map(|s| (id, s.client.clone())))
                    .collect(),
                None => Vec::new(),
            }
        };
        if wanted.is_empty() {
            // Empty catalog is the first watch vs Find race; TCP close still
            // drops a dead instance via read_loop.
            return;
        }
        for (id, client) in stale {
            let _ = client.shutdown().await;
            let mut w = self.clients.write().await;
            if let Some(cmap) = w.get_mut(service_name) {
                cmap.remove(&id);
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

    async fn build_client(
        self: &Arc<Self>,
        service_name: &str,
        reg: DefaultRegistration,
    ) -> Result<bool, Error> {
        if reg.protocol != "tcp" {
            warn!(id = %reg.service_id, "skip non-tcp");
            return Ok(false);
        }
        {
            let map = self.clients.read().await;
            if map
                .get(service_name)
                .map(|m| m.contains(&reg.service_id))
                .unwrap_or(false)
            {
                return Ok(false);
            }
        }
        let mut client = TcpClient::new(
            reg.service_id.clone(),
            service_name.to_string(),
            ClientOptions::default(),
        );
        client.set_dialer(self.dialer.clone());
        if let Err(e) = client.connect(&reg.dial_url()).await {
            warn!(id = %reg.service_id, %e, "dial failed");
            return Ok(false);
        }
        let client = Arc::new(client);
        let c2 = client.clone();
        let this = self.clone();
        tokio::spawn(async move {
            this.read_loop(c2).await;
        });
        let id = reg.service_id.clone();
        let mut w = self.clients.write().await;
        let cmap = w.entry(service_name.to_string()).or_default();
        if cmap.contains(&id) {
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
        let mut w = self.clients.write().await;
        if let Some(cmap) = w.get_mut(&service) {
            cmap.remove(&id);
        }
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
