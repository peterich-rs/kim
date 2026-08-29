use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fake_chat::directory::MemoryGroupDirectory;
use fake_chat::idgen::{resolve_snowflake_node, IdGenerator, SequenceIdGen, SnowflakeGen};
use fake_chat::royal::http_backends;
use fake_chat::store::{open_message_store, PoolConfig};
use fake_chat::ChatHandler;
use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer};
use kim_core::Server;
use kim_naming::{DefaultRegistration, StaticNaming};
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

    let naming = Arc::new(StaticNaming::from_slice(vec![]));
    let service_id = cfg.this.service_id.clone();
    let service_name = cfg.this.service_name.clone();
    let identity = DefaultRegistration {
        service_id: service_id.clone(),
        service_name: service_name.clone(),
        protocol: cfg.this.protocol,
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    };

    let redis_url = redis_url_from_env_or_cfg(&cfg.this.redis_url);
    let cache = open_session_store(redis_url.as_deref()).await?;

    let node = resolve_snowflake_node(Some(cfg.this.snowflake_node));
    let idgen: Arc<dyn IdGenerator> = match SnowflakeGen::try_new(node) {
        Ok(g) => Arc::new(g),
        Err(err) => {
            tracing::error!(%err, node, "snowflake init failed; using SequenceIdGen");
            Arc::new(SequenceIdGen::new(10_001))
        }
    };
    let (store, groups) = if let Some(royal) = royal_url_from_env_or_cfg(&cfg.this.royal_url) {
        http_backends(&royal)?
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
        let groups: Arc<dyn fake_chat::directory::GroupDirectory> =
            Arc::new(MemoryGroupDirectory::new(idgen));
        (store, groups)
    };

    let mut server = TcpServer::bind(&cfg.this.listen).await?;
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: service_id.clone(),
        }),
        deps: vec![],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(HashSelector),
        after_downlink: vec![],
    });
    let handler = Arc::new(ChatHandler::with_seams_and_zone(
        container.clone(),
        cache,
        store,
        groups,
        cfg.this.zone.clone(),
    ));
    if !cfg.this.metrics_listen.is_empty() {
        if let Ok(m) = kim_metrics::KimMetrics::new(&service_id, &service_name) {
            handler.with_metrics(m.clone());
            if let Ok(addr) = cfg.this.metrics_listen.parse::<std::net::SocketAddr>() {
                tokio::spawn(async move {
                    let _ = kim_metrics::serve(addr, m.registry()).await;
                });
            }
        }
    }
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    container.attach_server(Arc::new(server));

    let c = container.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = c.shutdown().await;
    });
    container.start().await?;
    Ok(())
}
