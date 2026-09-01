use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chat::directory::MemoryGroupDirectory;
use chat::idgen::{resolve_snowflake_node, IdGenerator, SequenceIdGen, SnowflakeGen};
use chat::royal::http_backends_with_hmac;
use chat::store::{open_message_store, PoolConfig};
use chat::users::MemoryUserDirectory;
use chat::ChatHandler;
use chat::{HmacNonceGuard, MemoryHmacNonceGuard};
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer};
use kim_core::Server;
use kim_naming::{open_naming, DefaultRegistration};
use kim_protocol::{
    check_strict_runtime, is_demo_internal_hmac, resolve_internal_hmac_secret, StrictCheck,
};
use kim_session::open_session_store;
use kim_tcp::TcpServer;
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
}

#[derive(Deserialize)]
struct SelfSection {
    service_id: String,
    service_name: String,
    listen: String,
    protocol: String,
    #[serde(default)]
    redis_url: String,
    #[serde(default)]
    database_url: String,
    #[serde(default)]
    royal_url: String,
    #[serde(default)]
    hmac_secret: String,
    #[serde(default = "default_db_max_connections")]
    db_max_connections: u32,
    #[serde(default = "default_db_acquire_timeout_ms")]
    db_acquire_timeout_ms: u64,
    #[serde(default = "default_db_idle_timeout_secs")]
    db_idle_timeout_secs: u64,
    #[serde(default = "default_snowflake_node")]
    snowflake_node: u16,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    metrics_listen: String,
    #[serde(default)]
    public_address: String,
    #[serde(default)]
    public_port: u16,
    #[serde(default)]
    idc: String,
    #[serde(default)]
    consul_url: String,
    #[serde(default)]
    adult_delay_ms: u64,
    #[serde(default)]
    sensitive_words: Vec<String>,
    #[serde(default)]
    blocked_image: Vec<String>,
}

fn default_snowflake_node() -> u16 {
    1
}

fn default_db_max_connections() -> u32 {
    5
}

fn default_db_acquire_timeout_ms() -> u64 {
    3000
}

fn default_db_idle_timeout_secs() -> u64 {
    60
}

fn redis_url_from_env_or_cfg(cfg: &str) -> Option<String> {
    match std::env::var("REDIS_URL") {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ if !cfg.trim().is_empty() => Some(cfg.to_string()),
        _ => None,
    }
}

fn database_url_from_env_or_cfg(cfg: &str) -> Option<String> {
    match std::env::var("DATABASE_URL") {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ if !cfg.trim().is_empty() => Some(cfg.to_string()),
        _ => None,
    }
}

fn royal_url_from_env_or_cfg(cfg: &str) -> Option<String> {
    match std::env::var("ROYAL_URL") {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ if !cfg.trim().is_empty() => Some(cfg.to_string()),
        _ => None,
    }
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn env_or_cfg(key: &str, cfg: &str) -> Option<String> {
    env_nonempty(key).or_else(|| {
        let t = cfg.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}

fn port_from_listen(listen: &str) -> Option<u16> {
    listen.rsplit_once(':')?.1.parse().ok()
}

#[cfg(feature = "redis")]
async fn open_nonce_guard(
    url: &str,
) -> Result<Arc<dyn HmacNonceGuard>, Box<dyn std::error::Error>> {
    Ok(Arc::new(chat::RedisHmacNonceGuard::open(url).await?))
}

#[cfg(not(feature = "redis"))]
async fn open_nonce_guard(
    _url: &str,
) -> Result<Arc<dyn HmacNonceGuard>, Box<dyn std::error::Error>> {
    Err("rebuild chat with --features redis".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.toml"));
    let cfg: File = toml::from_str(&std::fs::read_to_string(&path)?)?;

    let service_id =
        env_or_cfg("KIM_SERVICE_ID", &cfg.this.service_id).unwrap_or_else(|| "chat-1".into());
    let service_name = cfg.this.service_name.clone();
    let zone = env_or_cfg("KIM_ZONE", &cfg.this.zone).unwrap_or_else(|| cfg.this.zone.clone());
    let public_address =
        env_or_cfg("KIM_PUBLIC_ADDRESS", &cfg.this.public_address).unwrap_or_default();
    let public_port = env_nonempty("KIM_PUBLIC_PORT")
        .and_then(|s| s.parse().ok())
        .unwrap_or(cfg.this.public_port);
    let consul = env_or_cfg("CONSUL_HTTP_ADDR", &cfg.this.consul_url);
    let redis_url = redis_url_from_env_or_cfg(&cfg.this.redis_url);
    let hmac = resolve_internal_hmac_secret(&cfg.this.hmac_secret);
    if is_demo_internal_hmac(&hmac) {
        tracing::warn!(secret = "demo-default-hmac", "do not use in production");
    }
    check_strict_runtime(StrictCheck {
        hmac: Some(&hmac),
        jwt: None,
        redis_url: redis_url.as_deref(),
        require_redis: true,
        consul_addr: consul.as_deref(),
    })?;
    let naming = open_naming(consul.as_deref(), vec![])?;
    let mut tags = Vec::new();
    if !zone.is_empty() {
        tags.push(format!("zone:{zone}"));
    }
    if let Some(idc) = env_or_cfg("KIM_IDC", &cfg.this.idc) {
        tags.push(format!("IDC:{idc}"));
    } else if !cfg.this.idc.is_empty() {
        tags.push(format!("IDC:{}", cfg.this.idc));
    }
    let mut meta = HashMap::new();
    meta.insert("protocol".into(), cfg.this.protocol.clone());
    if !zone.is_empty() {
        meta.insert("zone".into(), zone.clone());
    }
    if !public_address.is_empty() {
        let health_port = port_from_listen(&cfg.this.metrics_listen).ok_or_else(|| {
            std::io::Error::other("metrics_listen required when public_address is set")
        })?;
        meta.insert(
            "health_url".into(),
            format!("http://{public_address}:{health_port}/health"),
        );
    }
    let identity = DefaultRegistration {
        service_id: service_id.clone(),
        service_name: service_name.clone(),
        protocol: cfg.this.protocol,
        public_address,
        public_port,
        tags,
        meta,
    };

    let cache = open_session_store(redis_url.as_deref()).await?;
    let nonce: Arc<dyn HmacNonceGuard> = match redis_url.as_deref() {
        Some(url) => open_nonce_guard(url).await?,
        None => Arc::new(MemoryHmacNonceGuard::new()),
    };

    let node = resolve_snowflake_node(Some(cfg.this.snowflake_node));
    let idgen: Arc<dyn IdGenerator> = match SnowflakeGen::try_new(node) {
        Ok(g) => Arc::new(g),
        Err(err) => {
            tracing::error!(%err, node, "snowflake init failed; using SequenceIdGen");
            Arc::new(SequenceIdGen::new(10_001))
        }
    };
    let (store, groups, users, social) =
        if let Some(royal) = royal_url_from_env_or_cfg(&cfg.this.royal_url) {
            http_backends_with_hmac(&royal, &hmac)?
        } else {
            let store = open_message_store(
                database_url_from_env_or_cfg(&cfg.this.database_url).as_deref(),
                redis_url.as_deref(),
                idgen.clone(),
                PoolConfig {
                    max_connections: cfg.this.db_max_connections.max(1),
                    acquire_timeout: Duration::from_millis(cfg.this.db_acquire_timeout_ms.max(1)),
                    idle_timeout: Duration::from_secs(cfg.this.db_idle_timeout_secs.max(1)),
                },
            )
            .await?;
            let groups: Arc<dyn chat::directory::GroupDirectory> =
                Arc::new(MemoryGroupDirectory::new(idgen));
            let users: Arc<dyn chat::users::UserDirectory> = Arc::new(MemoryUserDirectory::new());
            let social: Arc<dyn chat::social::SocialDirectory> =
                Arc::new(chat::social::MemorySocialDirectory::new());
            (store, groups, users, social)
        };

    let mut server = TcpServer::bind(&cfg.this.listen).await?;
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: service_id.clone(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(cfg.this.adult_delay_ms),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let handler = Arc::new(ChatHandler::with_social(
        container.clone(),
        cache,
        store,
        groups,
        zone,
        chat::builtin_talk_filter(cfg.this.sensitive_words, cfg.this.blocked_image),
        users,
        social,
        chat::store::pending_receipt_enabled(),
    ));
    if !cfg.this.metrics_listen.is_empty() {
        if let Ok(addr) = cfg.this.metrics_listen.parse::<std::net::SocketAddr>() {
            let mut http = chat::admin_router(handler.admin(hmac.clone(), nonce.clone()));
            if let Ok(m) = kim_metrics::KimMetrics::new(&service_id, &service_name) {
                handler.with_metrics(m.clone());
                http = http.merge(kim_metrics::router(m.registry()));
            }
            tokio::spawn(async move {
                let _ = chat::serve_admin(addr, http).await;
            });
        }
    }
    server.set_lane_key(Arc::new(|payload: &[u8]| {
        kim_protocol::logic_channel_id(payload)
    }));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    container.attach_server(Arc::new(server));

    let c = container.clone();
    tokio::spawn(async move {
        kim_core::wait_shutdown_signal().await;
        let _ = c.shutdown().await;
    });
    container.start().await?;
    Ok(())
}
