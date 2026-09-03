//! WGateway demo handler: JWT login Accept, local ping, SN_LOGIN forward.

mod run;
mod selector;
mod slots;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub use run::{load_config, open_redis_revoke, run_gateway, GatewayConfig};
pub use selector::{Route, RouteFile, RouteSelector, ZoneFile};
pub use slots::build_slots;

use async_trait::async_trait;
use bytes::Bytes;
use kim_container::{Container, DownlinkHook};
use kim_core::{
    Acceptor, ChannelHandle, Conn, Error, MessageListener, OpCode, Server, StateListener,
};
use kim_metrics::KimMetrics;
use kim_protocol::pkt::{
    AuthResp, DeviceCheckQuery, DeviceCheckStatus, Flag, KickoutNotify, LoginReq, RevokeQuery,
    RevokeStatus, Session, Status, TokenEpoch, TokenEpochQuery,
};
use kim_protocol::{
    generate_with_device, marshal, parse, read, read_logic, resolve_internal_hmac_secret,
    sign_internal_hmac, token_epoch_key, token_revoke_key, BasicPkt, LogicPkt, Packet, ALLOWED_APP,
    CMD_LOGIN_RENEW, CMD_LOGIN_SIGN_IN, CMD_LOGIN_SIGN_OUT, CODE_PING, CODE_PONG,
    DEMO_DEFAULT_SECRET, META_ACCOUNT, META_APP, SN_LOGIN,
};
use kim_session::{key_location, key_session, SESSION_TTL};
use prost::Message;
use tracing::{info, warn};

pub fn resolve_jwt_secret(config_secret: &str) -> String {
    if let Ok(env) = std::env::var("KIM_JWT_SECRET") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let trimmed = config_secret.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    tracing::warn!(secret = "demo-default", "do not use in production");
    DEMO_DEFAULT_SECRET.to_string()
}

pub fn remote_ip(conn: &dyn Conn) -> String {
    let Some(peer) = conn.peer_addr() else {
        return String::new();
    };
    strip_port(&peer)
}

fn strip_port(peer: &str) -> String {
    if let Some(rest) = peer.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return rest[..end].to_string();
        }
        return peer.to_string();
    }
    match peer.rsplit_once(':') {
        Some((ip, _)) => ip.to_string(),
        None => peer.to_string(),
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref().map(str::trim),
        Some("1") | Some("true") | Some("TRUE")
    )
}

async fn write_status(
    conn: &mut dyn Conn,
    header: &kim_protocol::pkt::Header,
    status: Status,
) -> Result<(), Error> {
    let mut pkt = LogicPkt::new_from(header);
    pkt.header.status = status as i32;
    pkt.header.flag = Flag::Response as i32;
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
        .await
}

/// Consecutive heartbeat revoke-store failures allowed before disconnect.
/// Login/handshake revoke errors stay fail-closed (no grace).
const HEARTBEAT_REVOKE_ERROR_GRACE: u32 = 3;

struct ChannelMeta {
    app: String,
    account: String,
    jti: String,
    ver: u32,
    did: String,
    idle_exp: i64,
    jwt_exp: i64,
    revoke_errors: u32,
}

impl ChannelMeta {
    fn new(
        app: impl Into<String>,
        account: impl Into<String>,
        jti: impl Into<String>,
        ver: u32,
        did: impl Into<String>,
        jwt_exp: i64,
    ) -> Self {
        Self {
            app: app.into(),
            account: account.into(),
            jti: jti.into(),
            ver,
            did: did.into(),
            idle_exp: 0,
            jwt_exp,
            revoke_errors: 0,
        }
    }
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[async_trait]
pub trait RevokeCheck: Send + Sync {
    async fn is_revoked(&self, jti: &str) -> Result<bool, String>;
    async fn token_epoch(&self, _account: &str) -> Result<u32, String> {
        Ok(0)
    }
    async fn device_ok(&self, _account: &str, _did: &str) -> Result<bool, String> {
        Ok(true)
    }
}

/// Always `false`. Local / e2e stacks without Redis still accept JWT `jti`.
pub struct AllowAllRevoke;

#[async_trait]
impl RevokeCheck for AllowAllRevoke {
    async fn is_revoked(&self, _jti: &str) -> Result<bool, String> {
        Ok(false)
    }
}

pub struct RevokeStore {
    conn: redis::aio::ConnectionManager,
}

impl RevokeStore {
    pub async fn open(url: &str) -> Result<Self, String> {
        let conn = kim_session::open_connection_manager(url)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    async fn is_revoked(&self, jti: &str) -> Result<bool, String> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(token_revoke_key(jti))
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(found.is_some())
    }

    async fn token_epoch(&self, account: &str) -> Result<u32, String> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(token_epoch_key(account))
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(found.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    async fn device_ok(&self, account: &str, did: &str) -> Result<bool, String> {
        let mut conn = self.conn.clone();
        let found: Option<String> = redis::cmd("GET")
            .arg(kim_protocol::device_hot_key(did))
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(found.is_some_and(|a| a == account))
    }

    async fn touch_session(&self, account: &str, channel_id: &str) -> Result<(), String> {
        let mut conn = self.conn.clone();
        let ttl = i64::try_from(SESSION_TTL.as_secs()).unwrap_or(i64::MAX);
        redis::pipe()
            .cmd("EXPIRE")
            .arg(key_session(channel_id))
            .arg(ttl)
            .cmd("EXPIRE")
            .arg(key_location(account, ""))
            .arg(ttl)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| e.to_string())
    }
}

#[async_trait]
impl RevokeCheck for RevokeStore {
    async fn is_revoked(&self, jti: &str) -> Result<bool, String> {
        RevokeStore::is_revoked(self, jti).await
    }

    async fn token_epoch(&self, account: &str) -> Result<u32, String> {
        RevokeStore::token_epoch(self, account).await
    }

    async fn device_ok(&self, account: &str, did: &str) -> Result<bool, String> {
        RevokeStore::device_ok(self, account, did).await
    }
}

pub struct HttpRevoke {
    base: String,
    http: reqwest::Client,
    hmac_secret: String,
}

impl HttpRevoke {
    pub fn new(base: &str) -> Result<Self, String> {
        Self::with_hmac(base, &resolve_internal_hmac_secret(""))
    }

    pub fn with_hmac(base: &str, hmac_secret: &str) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            http,
            hmac_secret: hmac_secret.to_string(),
        })
    }

    async fn post_pb<Q: Message, R: Message + Default>(
        &self,
        path: &str,
        body: Q,
    ) -> Result<R, String> {
        let buf = body.encode_to_vec();
        let headers = sign_internal_hmac(self.hmac_secret.as_bytes(), "POST", path, &buf)
            .map_err(|e| e.to_string())?;
        let mut req = self
            .http
            .post(format!("{}{path}", self.base))
            .header("Content-Type", "application/x-protobuf");
        for (k, v) in headers.pairs() {
            req = req.header(k, v);
        }
        let resp = req.body(buf).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("royal http {}", resp.status()));
        }
        let raw = resp.bytes().await.map_err(|e| e.to_string())?;
        R::decode(raw.as_ref()).map_err(|e| e.to_string())
    }
}

#[async_trait]
impl RevokeCheck for HttpRevoke {
    async fn is_revoked(&self, jti: &str) -> Result<bool, String> {
        let status: RevokeStatus = self
            .post_pb(
                "/internal/revoke/check",
                RevokeQuery {
                    jti: jti.to_string(),
                },
            )
            .await?;
        Ok(status.revoked)
    }

    async fn token_epoch(&self, account: &str) -> Result<u32, String> {
        let status: TokenEpoch = self
            .post_pb(
                "/internal/token-epoch",
                TokenEpochQuery {
                    account: account.to_string(),
                },
            )
            .await?;
        Ok(status.epoch)
    }

    async fn device_ok(&self, account: &str, did: &str) -> Result<bool, String> {
        let status: DeviceCheckStatus = self
            .post_pb(
                "/internal/device/check",
                DeviceCheckQuery {
                    account: account.to_string(),
                    device_id: did.to_string(),
                    device_credential: String::new(),
                },
            )
            .await?;
        Ok(status.ok)
    }
}

pub struct GatewayHandler {
    container: Arc<Container>,
    gateway_id: String,
    jwt_secret: String,
    token_ttl_secs: i64,
    seq: AtomicU64,
    pending: Mutex<HashMap<String, LogicPkt>>,
    meta: Mutex<HashMap<String, ChannelMeta>>,
    metrics: Mutex<Option<Arc<KimMetrics>>>,
    revoke: OnceLock<Arc<dyn RevokeCheck>>,
    redis: OnceLock<Arc<RevokeStore>>,
    server: OnceLock<Arc<dyn Server + Send + Sync>>,
    require_jti: AtomicBool,
}

impl GatewayHandler {
    pub fn new(
        container: Arc<Container>,
        gateway_id: impl Into<String>,
        jwt_secret: impl Into<String>,
    ) -> Self {
        Self::with_ttl(container, gateway_id, jwt_secret, 86_400)
    }

    pub fn with_ttl(
        container: Arc<Container>,
        gateway_id: impl Into<String>,
        jwt_secret: impl Into<String>,
        token_ttl_secs: i64,
    ) -> Self {
        Self {
            container,
            gateway_id: gateway_id.into(),
            jwt_secret: jwt_secret.into(),
            token_ttl_secs: if token_ttl_secs > 0 {
                token_ttl_secs
            } else {
                86_400
            },
            seq: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            meta: Mutex::new(HashMap::new()),
            metrics: Mutex::new(None),
            revoke: OnceLock::new(),
            redis: OnceLock::new(),
            server: OnceLock::new(),
            require_jti: AtomicBool::new(env_flag("KIM_REQUIRE_JTI")),
        }
    }

    pub fn set_require_jti(&self, require: bool) {
        self.require_jti.store(require, Ordering::Relaxed);
    }

    pub fn with_metrics(&self, m: Arc<KimMetrics>) {
        *self.metrics.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
    }

    pub fn set_revoke(&self, store: Arc<dyn RevokeCheck>) {
        let _ = self.revoke.set(store);
    }

    pub fn set_redis(&self, store: Arc<RevokeStore>) {
        let _ = self.redis.set(store);
    }

    pub fn attach_server(&self, server: Arc<dyn Server + Send + Sync>) {
        let _ = self.server.set(server);
    }

    fn generate_channel_id(&self, account: &str) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        format!("{}_{account}_{n}", self.gateway_id)
    }

    fn insert_meta(&self, channel_id: &str, mut meta: ChannelMeta) {
        meta.idle_exp = now_ts().saturating_add(self.token_ttl_secs);
        self.meta
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(channel_id.to_string(), meta);
    }

    async fn close_now(&self, channel_id: &str) {
        if let Some(srv) = self.server.get() {
            let _ = srv.close_channel(channel_id).await;
        }
    }

    fn note_revoke_error(&self, channel_id: &str) -> u32 {
        let mut guard = self.meta.lock().unwrap_or_else(|e| e.into_inner());
        match guard.get_mut(channel_id) {
            Some(m) => {
                m.revoke_errors = m.revoke_errors.saturating_add(1);
                m.revoke_errors
            }
            None => HEARTBEAT_REVOKE_ERROR_GRACE,
        }
    }

    fn clear_revoke_errors(&self, channel_id: &str) {
        let mut guard = self.meta.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = guard.get_mut(channel_id) {
            m.revoke_errors = 0;
        }
    }

    async fn heartbeat(&self, channel_id: &str) -> Result<Option<LogicPkt>, ()> {
        let meta = {
            let guard = self.meta.lock().unwrap_or_else(|e| e.into_inner());
            guard.get(channel_id).map(|m| {
                (
                    m.app.clone(),
                    m.account.clone(),
                    m.jti.clone(),
                    m.ver,
                    m.did.clone(),
                    m.idle_exp,
                    m.jwt_exp,
                )
            })
        };
        let Some((app, account, jti, ver, did, idle_exp, jwt_exp)) = meta else {
            return Ok(None);
        };
        let mut skip_renew = false;
        if let Some(store) = self.revoke.get() {
            if !jti.is_empty() {
                match store.is_revoked(&jti).await {
                    Ok(true) => {
                        warn!(channel = channel_id, "heartbeat revoked");
                        self.close_now(channel_id).await;
                        return Err(());
                    }
                    Ok(false) => self.clear_revoke_errors(channel_id),
                    Err(err) => {
                        let consecutive = self.note_revoke_error(channel_id);
                        if let Some(m) = self.metrics() {
                            m.on_heartbeat_revoke_error();
                        }
                        if consecutive >= HEARTBEAT_REVOKE_ERROR_GRACE {
                            warn!(
                                channel = channel_id,
                                consecutive,
                                %err,
                                "heartbeat revoke check failed past grace"
                            );
                            self.close_now(channel_id).await;
                            return Err(());
                        }
                        warn!(
                            channel = channel_id,
                            consecutive,
                            grace = HEARTBEAT_REVOKE_ERROR_GRACE,
                            %err,
                            "heartbeat revoke check failed; keeping connection"
                        );
                        skip_renew = true;
                    }
                }
            }
            match store.token_epoch(&account).await {
                Ok(epoch) if ver < epoch => {
                    warn!(channel = channel_id, "heartbeat stale epoch");
                    self.close_now(channel_id).await;
                    return Err(());
                }
                Ok(_) => {}
                Err(err) => {
                    let consecutive = self.note_revoke_error(channel_id);
                    if let Some(m) = self.metrics() {
                        m.on_heartbeat_revoke_error();
                    }
                    if consecutive >= HEARTBEAT_REVOKE_ERROR_GRACE {
                        warn!(
                            channel = channel_id,
                            consecutive,
                            %err,
                            "heartbeat epoch check failed past grace"
                        );
                        self.close_now(channel_id).await;
                        return Err(());
                    }
                    warn!(
                        channel = channel_id,
                        consecutive,
                        grace = HEARTBEAT_REVOKE_ERROR_GRACE,
                        %err,
                        "heartbeat epoch check failed; keeping connection"
                    );
                    skip_renew = true;
                }
            }
            if !did.is_empty() {
                match store.device_ok(&account, &did).await {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(channel = channel_id, "heartbeat device revoked");
                        self.close_now(channel_id).await;
                        return Err(());
                    }
                    Err(err) => {
                        let consecutive = self.note_revoke_error(channel_id);
                        if let Some(m) = self.metrics() {
                            m.on_heartbeat_revoke_error();
                        }
                        if consecutive >= HEARTBEAT_REVOKE_ERROR_GRACE {
                            warn!(
                                channel = channel_id,
                                consecutive,
                                %err,
                                "heartbeat device check failed past grace"
                            );
                            self.close_now(channel_id).await;
                            return Err(());
                        }
                        warn!(
                            channel = channel_id,
                            consecutive,
                            grace = HEARTBEAT_REVOKE_ERROR_GRACE,
                            %err,
                            "heartbeat device check failed; keeping connection"
                        );
                        skip_renew = true;
                    }
                }
            }
        }
        let now = now_ts();
        if idle_exp > 0 && now >= idle_exp {
            warn!(channel = channel_id, "heartbeat expired");
            self.close_now(channel_id).await;
            return Err(());
        }
        let next_idle = now.saturating_add(self.token_ttl_secs);
        let remaining = jwt_exp.saturating_sub(now);
        let half = self.token_ttl_secs / 2;
        let renew = remaining < half && !jti.is_empty() && !skip_renew;
        let next_jwt = if renew { next_idle } else { jwt_exp };
        {
            let mut guard = self.meta.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(m) = guard.get_mut(channel_id) {
                m.idle_exp = next_idle;
                m.jwt_exp = next_jwt;
            }
        }
        if let Some(redis) = self.redis.get() {
            if let Err(err) = redis.touch_session(&account, channel_id).await {
                warn!(%err, "session expire");
            }
        }
        if !renew {
            return Ok(None);
        }
        let did_ref = if did.is_empty() {
            None
        } else {
            Some(did.as_str())
        };
        match generate_with_device(
            &self.jwt_secret,
            &account,
            &app,
            next_jwt,
            &jti,
            ver,
            did_ref,
        ) {
            Ok(token) => {
                let mut pkt = LogicPkt::new(CMD_LOGIN_RENEW, 0, Bytes::new());
                pkt.header.flag = Flag::Push as i32;
                pkt.header.channel_id = channel_id.to_string();
                pkt.write_body(&AuthResp {
                    token,
                    exp: next_jwt,
                    account,
                    device_id: did,
                    device_credential: String::new(),
                });
                Ok(Some(pkt))
            }
            Err(err) => {
                warn!(%err, "renew token");
                Ok(None)
            }
        }
    }

    fn remove_meta(&self, channel_id: &str) {
        self.meta
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(channel_id);
    }

    fn inject_meta(&self, pkt: &mut LogicPkt) {
        let guard = self.meta.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = guard.get(&pkt.header.channel_id) {
            pkt.set_meta(META_APP, &m.app);
            pkt.set_meta(META_ACCOUNT, &m.account);
        }
    }

    async fn forward_logic(&self, service: &str, mut pkt: LogicPkt) -> Result<(), Error> {
        self.inject_meta(&mut pkt);
        match self.container.forward(service, pkt).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if err.to_string() == "no adult instances" {
                    if let Some(m) = self
                        .metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .as_ref()
                    {
                        m.on_no_server();
                    }
                }
                Err(Error::other(err.to_string()))
            }
        }
    }

    fn metrics(&self) -> Option<Arc<KimMetrics>> {
        self.metrics
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

pub struct MetricsHook(pub Arc<KimMetrics>);

#[async_trait]
impl DownlinkHook for MetricsHook {
    async fn after_push(&self, _channel_id: &str, pkt: &LogicPkt) {
        let n = marshal(&Packet::Logic(pkt.clone())).len() as u64;
        self.0.on_message_out(n);
    }
}

#[async_trait]
impl Acceptor for GatewayHandler {
    async fn accept(&self, conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
        let frame = tokio::time::timeout(timeout, conn.read_frame())
            .await
            .map_err(|_| Error::HandshakeTimeout(timeout))??;
        let mut pkt = match read_logic(&frame.payload) {
            Ok(p) => p,
            Err(_) => return Err(Error::Handshake("expected login.signin".into())),
        };
        if pkt.header.command != CMD_LOGIN_SIGN_IN {
            write_status(conn, &pkt.header, Status::InvalidCommand).await?;
            return Err(Error::Handshake("invalid command".into()));
        }
        let req: LoginReq = match pkt.read_body() {
            Ok(r) => r,
            Err(_) => {
                write_status(conn, &pkt.header, Status::InvalidPacketBody).await?;
                return Err(Error::Handshake("invalid packet body".into()));
            }
        };
        let claims = match parse(&self.jwt_secret, &req.token) {
            Ok(c) => c,
            Err(err) => {
                warn!(%err, "unauthorized");
                write_status(conn, &pkt.header, Status::Unauthorized).await?;
                return Err(Error::Handshake("unauthorized".into()));
            }
        };
        if claims.app != ALLOWED_APP {
            write_status(conn, &pkt.header, Status::Unauthorized).await?;
            return Err(Error::Handshake("app not allowed".into()));
        }
        let jti = claims
            .jti
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if self.require_jti.load(Ordering::Relaxed) && jti.is_none() {
            write_status(conn, &pkt.header, Status::Unauthorized).await?;
            return Err(Error::Handshake("unauthorized".into()));
        }
        let did = claims
            .did
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let cred = req.device_credential.trim();
        if jti.is_some() || did.is_some() || !cred.is_empty() {
            let Some(store) = self.revoke.get() else {
                warn!("revoke store missing; refusing login");
                write_status(conn, &pkt.header, Status::Unauthorized).await?;
                return Err(Error::Handshake("revoke store required".into()));
            };
            if let Some(jti) = jti {
                match store.is_revoked(jti).await {
                    Ok(true) => {
                        warn!(account = %claims.account, "revoked token");
                        write_status(conn, &pkt.header, Status::Unauthorized).await?;
                        return Err(Error::Handshake("revoked".into()));
                    }
                    Ok(false) => {}
                    Err(err) => {
                        warn!(%err, "revoke check failed");
                        write_status(conn, &pkt.header, Status::Unauthorized).await?;
                        return Err(Error::Handshake("revoke check failed".into()));
                    }
                }
            }
            match store.token_epoch(&claims.account).await {
                Ok(epoch) if claims.ver < epoch => {
                    warn!(account = %claims.account, "stale token epoch");
                    write_status(conn, &pkt.header, Status::Unauthorized).await?;
                    return Err(Error::Handshake("revoked".into()));
                }
                Ok(_) => {}
                Err(err) => {
                    warn!(%err, "epoch check failed");
                    write_status(conn, &pkt.header, Status::Unauthorized).await?;
                    return Err(Error::Handshake("revoke check failed".into()));
                }
            }
            if did.is_some() || !cred.is_empty() {
                let Some(did) = did else {
                    write_status(conn, &pkt.header, Status::Unauthorized).await?;
                    return Err(Error::Handshake("unauthorized".into()));
                };
                match store.device_ok(&claims.account, did).await {
                    Ok(true) => {}
                    Ok(false) => {
                        warn!(account = %claims.account, "device not allowed");
                        write_status(conn, &pkt.header, Status::Unauthorized).await?;
                        return Err(Error::Handshake("unauthorized".into()));
                    }
                    Err(err) => {
                        warn!(%err, "device check failed");
                        write_status(conn, &pkt.header, Status::Unauthorized).await?;
                        return Err(Error::Handshake("revoke check failed".into()));
                    }
                }
            }
        } else if let Some(store) = self.revoke.get() {
            match store.token_epoch(&claims.account).await {
                Ok(epoch) if claims.ver < epoch => {
                    warn!(account = %claims.account, "stale token epoch");
                    write_status(conn, &pkt.header, Status::Unauthorized).await?;
                    return Err(Error::Handshake("revoked".into()));
                }
                Ok(_) => {}
                Err(err) => {
                    warn!(%err, "epoch check failed");
                    write_status(conn, &pkt.header, Status::Unauthorized).await?;
                    return Err(Error::Handshake("revoke check failed".into()));
                }
            }
        }
        let jti = jti.unwrap_or("").to_string();
        let did = did.unwrap_or("").to_string();
        let id = self.generate_channel_id(&claims.account);
        pkt.header.channel_id = id.clone();
        pkt.write_body(&Session {
            channel_id: id.clone(),
            gate_id: self.gateway_id.clone(),
            account: claims.account.clone(),
            app: claims.app.clone(),
            remote_ip: remote_ip(conn),
            device: req.device,
            jti: jti.clone(),
            device_id: did.clone(),
            ..Session::default()
        });
        self.insert_meta(
            &id,
            ChannelMeta::new(
                claims.app,
                claims.account.clone(),
                jti,
                claims.ver,
                did,
                claims.exp,
            ),
        );
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(id.clone(), pkt);
        }
        info!(account = %claims.account, channel = %id, "accept login");
        Ok(id)
    }

    async fn on_accept_abandoned(&self, channel_id: &str) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.remove(channel_id);
        self.remove_meta(channel_id);
    }

    async fn on_channel_ready(&self, channel_id: &str) -> Result<(), Error> {
        let pkt = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.remove(channel_id)
        };
        let Some(pkt) = pkt else {
            return Err(Error::other("pending login missing"));
        };
        let mut resp = LogicPkt::new_from(&pkt.header);
        resp.header.flag = Flag::Response as i32;
        resp.header.status = Status::ServiceUnavailable as i32;
        if let Err(err) = self.forward_logic(SN_LOGIN, pkt).await {
            warn!(%err, "forward login failed");
            self.remove_meta(channel_id);
            if let Err(e) = self.container.push(channel_id, resp).await {
                warn!(%e, "push ServiceUnavailable failed");
            }
            return Err(err);
        }
        if let Some(m) = self.metrics() {
            m.on_channel_open();
            m.on_login(Status::Success as i32);
        }
        Ok(())
    }
}

#[async_trait]
impl MessageListener for GatewayHandler {
    async fn receive(&self, handle: &dyn ChannelHandle, payload: Bytes) {
        let pkt = match read(&payload) {
            Ok(p) => p,
            Err(err) => {
                warn!(%err, "bad payload");
                return;
            }
        };
        match pkt {
            Packet::Basic(p) if p.code == CODE_PING => {
                let id = handle.id().to_string();
                let renew = match self.heartbeat(&id).await {
                    Ok(v) => v,
                    Err(()) => return,
                };
                info!(channel = %id, "basic ping, local pong");
                let _ = handle
                    .push(marshal(&Packet::Basic(BasicPkt {
                        code: CODE_PONG,
                        body: Bytes::new(),
                    })))
                    .await;
                if let Some(pkt) = renew {
                    let _ = handle.push(marshal(&Packet::Logic(pkt))).await;
                }
            }
            Packet::Basic(_) => {}
            Packet::Logic(mut logic) => {
                if let Some(m) = self.metrics() {
                    m.on_message_in(payload.len() as u64);
                }
                logic.header.channel_id = handle.id().to_string();
                let svc = logic.service_name().to_string();
                let header = logic.header.clone();
                if let Err(err) = self.forward_logic(&svc, logic).await {
                    warn!(%err, "forward failed");
                    let mut resp = LogicPkt::new_from(&header);
                    resp.header.flag = Flag::Response as i32;
                    resp.header.status = Status::ServiceUnavailable as i32;
                    let _ = handle.push(marshal(&Packet::Logic(resp))).await;
                }
            }
        }
    }
}

#[async_trait]
impl StateListener for GatewayHandler {
    async fn disconnect(&self, channel_id: &str) -> Result<(), Error> {
        info!(channel = channel_id, "disconnect");
        let mut logout = LogicPkt::new(CMD_LOGIN_SIGN_OUT, 0, Bytes::new());
        logout.header.channel_id = channel_id.to_string();
        if let Err(err) = self.forward_logic(SN_LOGIN, logout).await {
            warn!(%err, "signout forward failed");
        }
        self.remove_meta(channel_id);
        if let Some(m) = self.metrics() {
            m.on_channel_close();
        }
        Ok(())
    }
}

/// Closes the channel after a Kickout Push is written. Attach the WsServer
/// after two-phase assembly (`set_*` then `Arc::new`).
pub struct KickHook {
    server: OnceLock<Arc<dyn Server + Send + Sync>>,
}

impl KickHook {
    pub fn new() -> Self {
        Self {
            server: OnceLock::new(),
        }
    }

    pub fn attach(&self, server: Arc<dyn Server + Send + Sync>) {
        let _ = self.server.set(server);
    }
}

impl Default for KickHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DownlinkHook for KickHook {
    async fn after_push(&self, channel_id: &str, pkt: &LogicPkt) {
        if pkt.header.flag != Flag::Push as i32 {
            return;
        }
        if pkt.header.command != CMD_LOGIN_SIGN_IN {
            return;
        }
        let Ok(notify) = pkt.read_body::<KickoutNotify>() else {
            return;
        };
        if notify.channel_id != channel_id {
            return;
        }
        info!(channel = channel_id, "kick close");
        if let Some(srv) = self.server.get() {
            let _ = srv.close_channel(channel_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use bytes::Bytes;
    use kim_container::{Container, ContainerOpts, InnerTcpDialer};
    use kim_core::{Acceptor, Conn, Error, Frame, MessageListener, OpCode, Server, StateListener};
    use kim_naming::{DefaultRegistration, StaticNaming};
    use kim_protocol::{
        generate_with_device, generate_with_session, marshal, LogicPkt, Packet, CMD_LOGIN_SIGN_IN,
        DEMO_DEFAULT_SECRET,
    };

    use super::{
        strip_port, AllowAllRevoke, ChannelMeta, GatewayHandler, LoginReq, RevokeCheck,
        HEARTBEAT_REVOKE_ERROR_GRACE,
    };

    #[test]
    fn strip_ipv4_port() {
        assert_eq!(strip_port("127.0.0.1:8001"), "127.0.0.1");
    }

    #[test]
    fn strip_ipv6_port() {
        assert_eq!(strip_port("[::1]:8001"), "::1");
    }

    struct ScriptedConn {
        incoming: Option<Frame>,
    }

    #[async_trait]
    impl Conn for ScriptedConn {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            self.incoming.take().ok_or(Error::Closed)
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    struct AlwaysRevoked;

    #[async_trait]
    impl RevokeCheck for AlwaysRevoked {
        async fn is_revoked(&self, _jti: &str) -> Result<bool, String> {
            Ok(true)
        }
    }

    fn test_handler() -> GatewayHandler {
        let container = Container::new(ContainerOpts {
            naming: Arc::new(StaticNaming::from_slice(vec![])),
            identity: DefaultRegistration {
                service_id: "wg-1".into(),
                service_name: "wgateway".into(),
                protocol: "ws".into(),
                public_address: "127.0.0.1".into(),
                public_port: 8001,
                tags: vec![],
                meta: Default::default(),
            },
            dialer: Arc::new(InnerTcpDialer {
                local_service_id: "wg-1".into(),
            }),
            deps: vec![],
            adult_delay: Duration::from_millis(0),
            selector: Arc::new(kim_container::HashSelector),
            after_downlink: vec![],
        });
        GatewayHandler::new(container, "wg-1", DEMO_DEFAULT_SECRET)
    }

    fn login_conn(jti: &str) -> ScriptedConn {
        login_conn_session(jti, 0, None)
    }

    fn login_conn_session(jti: &str, ver: u32, did: Option<&str>) -> ScriptedConn {
        let exp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            .saturating_add(3600);
        let token =
            generate_with_device(DEMO_DEFAULT_SECRET, "alice", "kim", exp, jti, ver, did).unwrap();
        let mut pkt = LogicPkt::new(CMD_LOGIN_SIGN_IN, 1, Bytes::new());
        pkt.write_body(&LoginReq {
            token,
            device: "web".into(),
            ..Default::default()
        });
        ScriptedConn {
            incoming: Some(Frame::binary(marshal(&Packet::Logic(pkt)))),
        }
    }

    fn handshake_err(err: Error) -> String {
        match err {
            Error::Handshake(msg) => msg,
            other => panic!("expected handshake error, got {other}"),
        }
    }

    #[tokio::test]
    async fn missing_revoke_store_rejects_login() {
        let handler = test_handler();
        let mut conn = login_conn("jti-revoked");
        let err = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect_err("login without revoke store must fail");
        let msg = handshake_err(err);
        assert!(msg.contains("revoke store"), "{msg}");
    }

    /// Local/demo gateway (no REDIS_URL/ROYAL_URL) installs [`AllowAllRevoke`].
    #[tokio::test]
    async fn allow_all_revoke_satisfies_login_store() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(AllowAllRevoke));
        let mut conn = login_conn("jti-ok");
        let id = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect("demo AllowAllRevoke must satisfy login revoke store");
        assert!(id.starts_with("wg-1_alice_"));
    }

    #[tokio::test]
    async fn revoked_jti_cannot_login() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(AlwaysRevoked));
        let mut conn = login_conn("jti-revoked");
        let err = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect_err("revoked jwt must not login");
        assert_eq!(handshake_err(err), "revoked");
    }

    struct NeverRevoked;

    #[async_trait]
    impl RevokeCheck for NeverRevoked {
        async fn is_revoked(&self, _jti: &str) -> Result<bool, String> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn missing_jti_rejected_when_required() {
        let handler = test_handler();
        handler.set_require_jti(true);
        handler.set_revoke(Arc::new(NeverRevoked));
        let mut conn = login_conn("");
        let err = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect_err("empty jti must not login when required");
        assert_eq!(handshake_err(err), "unauthorized");
    }

    #[tokio::test]
    async fn missing_jti_allowed_when_not_required() {
        let handler = test_handler();
        handler.set_require_jti(false);
        let mut conn = login_conn("");
        let id = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect("empty jti is compatible when require=0");
        assert!(id.starts_with("wg-1_alice_"));
    }

    struct RevokeBroken;

    #[async_trait]
    impl RevokeCheck for RevokeBroken {
        async fn is_revoked(&self, _jti: &str) -> Result<bool, String> {
            Err("redis down".into())
        }
    }

    #[derive(Default)]
    struct RecordingServer {
        closed: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Server for RecordingServer {
        fn set_acceptor(&mut self, _acceptor: Arc<dyn Acceptor>) {}
        fn set_message_listener(&mut self, _listener: Arc<dyn MessageListener>) {}
        fn set_state_listener(&mut self, _listener: Arc<dyn StateListener>) {}
        fn set_read_wait(&mut self, _wait: Duration) {}
        async fn start(&self) -> Result<(), Error> {
            Ok(())
        }
        async fn push(&self, _channel_id: &str, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
        async fn close_channel(&self, channel_id: &str) -> Result<(), Error> {
            self.closed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(channel_id.to_string());
            Ok(())
        }
        async fn shutdown(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    fn live_channel(handler: &GatewayHandler, jti: &str) -> String {
        let id = "wg-1_alice_hb".to_string();
        let exp = super::now_ts().saturating_add(86_400);
        handler.insert_meta(&id, ChannelMeta::new("kim", "alice", jti, 0, "", exp));
        id
    }

    fn closed_ids(server: &RecordingServer) -> Vec<String> {
        server
            .closed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    #[tokio::test]
    async fn heartbeat_revoked_true_closes() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(AlwaysRevoked));
        let server = Arc::new(RecordingServer::default());
        handler.attach_server(server.clone());
        let id = live_channel(&handler, "jti-revoked");
        assert!(
            handler.heartbeat(&id).await.is_err(),
            "confirmed revoke must close",
        );
        assert_eq!(closed_ids(&server), vec![id]);
    }

    #[tokio::test]
    async fn heartbeat_revoke_error_twice_keeps_connection() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(RevokeBroken));
        let server = Arc::new(RecordingServer::default());
        handler.attach_server(server.clone());
        let id = live_channel(&handler, "jti-ok");
        for i in 1..=2 {
            handler
                .heartbeat(&id)
                .await
                .unwrap_or_else(|_| panic!("error {i} must stay within grace"));
        }
        assert!(
            closed_ids(&server).is_empty(),
            "two revoke-store errors must not kick"
        );
    }

    #[tokio::test]
    async fn heartbeat_revoke_error_past_grace_closes() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(RevokeBroken));
        let server = Arc::new(RecordingServer::default());
        handler.attach_server(server.clone());
        let id = live_channel(&handler, "jti-ok");
        for i in 1..HEARTBEAT_REVOKE_ERROR_GRACE {
            handler
                .heartbeat(&id)
                .await
                .unwrap_or_else(|_| panic!("error {i} must stay within grace"));
            assert!(
                closed_ids(&server).is_empty(),
                "error {i} must not close yet"
            );
        }
        assert!(
            handler.heartbeat(&id).await.is_err(),
            "errors past grace must close",
        );
        assert_eq!(closed_ids(&server), vec![id]);
    }

    #[tokio::test]
    async fn revoke_store_error_rejects_login() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(RevokeBroken));
        let mut conn = login_conn("jti-ok");
        let err = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect_err("login revoke errors stay fail-closed");
        assert_eq!(handshake_err(err), "revoke check failed");
    }

    struct LiveRevoke {
        epoch: std::sync::atomic::AtomicU32,
        denied: std::sync::Mutex<Vec<String>>,
    }

    impl LiveRevoke {
        fn with_epoch(epoch: u32) -> Self {
            Self {
                epoch: std::sync::atomic::AtomicU32::new(epoch),
                denied: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl RevokeCheck for LiveRevoke {
        async fn is_revoked(&self, _jti: &str) -> Result<bool, String> {
            Ok(false)
        }

        async fn token_epoch(&self, _account: &str) -> Result<u32, String> {
            Ok(self.epoch.load(std::sync::atomic::Ordering::Relaxed))
        }

        async fn device_ok(&self, _account: &str, did: &str) -> Result<bool, String> {
            let denied = self.denied.lock().unwrap_or_else(|e| e.into_inner());
            Ok(!denied.iter().any(|d| d == did))
        }
    }

    #[tokio::test]
    async fn zero_ver_passes_when_epoch_is_zero() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(NeverRevoked));
        let mut conn = login_conn_session("jti-ok", 0, None);
        let id = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect("ver=0 epoch=0 must login");
        assert!(id.starts_with("wg-1_alice_"));
    }

    #[tokio::test]
    async fn stale_epoch_rejects_login() {
        let handler = test_handler();
        handler.set_revoke(Arc::new(LiveRevoke::with_epoch(2)));
        let mut conn = login_conn_session("jti-old", 0, None);
        let err = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect_err("ver < epoch must not login");
        assert_eq!(handshake_err(err), "revoked");
    }

    #[tokio::test]
    async fn heartbeat_fails_after_epoch_bump() {
        let handler = test_handler();
        let store = Arc::new(LiveRevoke::with_epoch(0));
        handler.set_revoke(store.clone());
        let mut conn = login_conn_session("jti-live", 0, None);
        let id = handler
            .accept(&mut conn, Duration::from_secs(1))
            .await
            .expect("login before bump");
        store.epoch.store(1, std::sync::atomic::Ordering::Relaxed);
        assert!(
            handler.heartbeat(&id).await.is_err(),
            "same jti heartbeat must fail after epoch bump"
        );
    }

    #[tokio::test]
    async fn revoked_did_rejects_only_that_device() {
        let handler = test_handler();
        let store = Arc::new(LiveRevoke::with_epoch(0));
        store
            .denied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push("dev-1".into());
        handler.set_revoke(store);
        let mut denied = login_conn_session("jti-d1", 0, Some("dev-1"));
        let err = handler
            .accept(&mut denied, Duration::from_secs(1))
            .await
            .expect_err("revoked did must not login");
        assert_eq!(handshake_err(err), "unauthorized");
        let mut other = login_conn_session("jti-web", 0, None);
        let id = handler
            .accept(&mut other, Duration::from_secs(1))
            .await
            .expect("session without did must still login");
        assert!(id.starts_with("wg-1_alice_"));
    }

    #[test]
    fn session_token_keeps_ver_for_renew() {
        let token =
            generate_with_session(DEMO_DEFAULT_SECRET, "alice", "kim", 9_999_999_999, "j", 4)
                .unwrap();
        let claims = kim_protocol::parse(DEMO_DEFAULT_SECRET, &token).unwrap();
        assert_eq!(claims.ver, 4);
        assert_eq!(claims.jti.as_deref(), Some("j"));
    }
}
