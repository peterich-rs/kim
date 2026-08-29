use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chat::idgen::{resolve_snowflake_node, IdGenerator, SequenceIdGen, SnowflakeGen};
use chat::store::{open_pg_backends, PoolConfig};
use kim_naming::{open_naming, DefaultRegistration, Naming};
use royal::{serve, JwtConfig, MemoryRevocation, RoyalState, TokenRevocation};
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
}

#[derive(Deserialize)]
struct SelfSection {
    listen: String,
    #[serde(default = "default_node")]
    snowflake_node: u16,
    #[serde(default)]
    service_id: String,
    #[serde(default)]
    service_name: String,
    #[serde(default)]
    public_address: String,
    #[serde(default)]
    public_port: u16,
    #[serde(default)]
    consul_url: String,
    #[serde(default)]
    database_url: String,
    #[serde(default)]
    redis_url: String,
    #[serde(default)]
    jwt_secret: String,
    #[serde(default)]
    token_ttl_secs: i64,
    #[serde(default)]
    app: String,
    #[serde(default)]
    chat_url: String,
}

fn default_node() -> u16 {
    10
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

fn jwt_secret(cfg: &str) -> String {
    env_or_cfg("KIM_JWT_SECRET", cfg).unwrap_or_else(|| {
        tracing::warn!(secret = "demo-default", "do not use in production");
        kim_protocol::DEMO_DEFAULT_SECRET.to_string()
    })
}

async fn open_revoke(url: &str) -> Result<Arc<dyn TokenRevocation>, Box<dyn std::error::Error>> {
    #[cfg(feature = "redis")]
    {
        Ok(Arc::new(royal::RedisRevocation::open(url).await?))
    }
    #[cfg(not(feature = "redis"))]
    {
        let _ = url;
        Err("rebuild royal with --features redis".into())
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
    let node = resolve_snowflake_node(Some(cfg.this.snowflake_node));
    let idgen: Arc<dyn IdGenerator> = match SnowflakeGen::try_new(node) {
        Ok(g) => Arc::new(g),
        Err(err) => {
            tracing::error!(%err, node, "snowflake init failed; using SequenceIdGen");
            Arc::new(SequenceIdGen::new(10_001))
        }
    };

    let jwt = JwtConfig {
        secret: jwt_secret(&cfg.this.jwt_secret),
        ttl_secs: env_nonempty("KIM_TOKEN_TTL_SECS")
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(if cfg.this.token_ttl_secs > 0 {
                cfg.this.token_ttl_secs
            } else {
                86_400
            }),
    };

    let redis = env_or_cfg("REDIS_URL", &cfg.this.redis_url);
    let revoke: Arc<dyn TokenRevocation> = match redis.as_deref() {
        Some(url) => open_revoke(url).await?,
        None => Arc::new(MemoryRevocation::new()),
    };

    let state = if let Some(db) = env_or_cfg("DATABASE_URL", &cfg.this.database_url) {
        let backends =
            open_pg_backends(&db, redis.as_deref(), idgen, PoolConfig::default()).await?;
        RoyalState::with_backends(backends.store, backends.groups, backends.users, jwt, revoke)
    } else {
        RoyalState::memory_with_jwt(idgen, jwt).with_revoke(revoke)
    };
    let app = env_or_cfg("KIM_APP", &cfg.this.app).unwrap_or_else(|| "kim".into());
    let chat_url = env_or_cfg("CHAT_URL", &cfg.this.chat_url).unwrap_or_default();
    let state = state.with_app(app).with_chat_url(chat_url);

    let listen = cfg.this.listen.clone();
    let public_address = env_or_cfg("KIM_PUBLIC_ADDRESS", &cfg.this.public_address);
    let consul = env_or_cfg("CONSUL_HTTP_ADDR", &cfg.this.consul_url);
    let naming = open_naming(consul.as_deref(), vec![])?;
    let service_id =
        env_or_cfg("KIM_SERVICE_ID", &cfg.this.service_id).unwrap_or_else(|| "royal-1".into());
    let service_name = if cfg.this.service_name.is_empty() {
        "royal".into()
    } else {
        cfg.this.service_name.clone()
    };
    let public_port = env_nonempty("KIM_PUBLIC_PORT")
        .and_then(|s| s.parse().ok())
        .or(if cfg.this.public_port == 0 {
            port_from_listen(&listen)
        } else {
            Some(cfg.this.public_port)
        })
        .unwrap_or(8080);

    if let Some(addr) = &public_address {
        let health_port = port_from_listen(&listen).ok_or("listen has no port")?;
        let mut meta = HashMap::new();
        meta.insert("protocol".into(), "http".into());
        meta.insert(
            "health_url".into(),
            format!("http://{addr}:{health_port}/health"),
        );
        Naming::register(
            naming.as_ref(),
            DefaultRegistration {
                service_id: service_id.clone(),
                service_name,
                protocol: "http".into(),
                public_address: addr.clone(),
                public_port,
                tags: vec![],
                meta,
            },
        )
        .await?;
    }

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    let n2 = naming.clone();
    let sid = service_id.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        if public_address.is_some() {
            let _ = Naming::deregister(n2.as_ref(), &sid).await;
        }
        std::process::exit(0);
    });
    serve(listener, state).await?;
    Ok(())
}
