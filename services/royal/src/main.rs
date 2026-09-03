use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chat::idgen::{resolve_snowflake_node, IdGenerator, SnowflakeGen};
use chat::open_uncached_session_store;
use chat::store::{open_pg_backends, pending_receipt_enabled, PoolConfig};
use kim_naming::{open_naming, DefaultRegistration, Naming};
use kim_protocol::{
    check_strict_runtime, is_demo_internal_hmac, resolve_internal_hmac_secret, StrictCheck,
    ALLOWED_APP,
};
use royal::{
    router, DeviceDirectory, DeviceHot, JwtConfig, MemoryDeviceDirectory, MemoryDeviceHot,
    MemoryRevocation, RoyalState, TokenRevocation,
};
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
    #[serde(default)]
    hmac_secret: String,
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

async fn scan_empty_jti() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "redis")]
    {
        let url = env_nonempty("REDIS_URL").ok_or("REDIS_URL is required for --scan-empty-jti")?;
        let scanner = kim_session::RedisSessionStore::open(&url).await?;
        let scan = scanner.count_empty_jti_locations().await?;
        println!(
            "empty_jti={} invalid={} wrong_type={} scanned={}",
            scan.empty_jti, scan.invalid, scan.wrong_type, scan.scanned
        );
        if !scan.is_clean() {
            std::process::exit(kim_session::empty_jti_gate_code(scan));
        }
        Ok(())
    }
    #[cfg(not(feature = "redis"))]
    Err("rebuild royal with --features redis".into())
}

fn jwt_secret(cfg: &str) -> String {
    env_or_cfg("KIM_JWT_SECRET", cfg).unwrap_or_else(|| {
        tracing::warn!(secret = "demo-default", "do not use in production");
        kim_protocol::DEMO_DEFAULT_SECRET.to_string()
    })
}

async fn open_devices(url: &str) -> Result<Arc<dyn DeviceDirectory>, Box<dyn std::error::Error>> {
    #[cfg(feature = "postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(url)
            .await?;
        Ok(Arc::new(royal::PostgresDeviceDirectory::from_pool(pool)))
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = url;
        Err("rebuild royal with --features postgres".into())
    }
}

async fn open_device_hot(url: &str) -> Result<Arc<dyn DeviceHot>, Box<dyn std::error::Error>> {
    #[cfg(feature = "redis")]
    {
        Ok(Arc::new(royal::RedisDeviceHot::open(url).await?))
    }
    #[cfg(not(feature = "redis"))]
    {
        let _ = url;
        Err("rebuild royal with --features redis".into())
    }
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

#[cfg(feature = "redis")]
async fn open_nonce(
    url: &str,
) -> Result<Arc<dyn chat::HmacNonceGuard>, Box<dyn std::error::Error>> {
    Ok(Arc::new(chat::RedisHmacNonceGuard::open(url).await?))
}

#[cfg(not(feature = "redis"))]
async fn open_nonce(
    _url: &str,
) -> Result<Arc<dyn chat::HmacNonceGuard>, Box<dyn std::error::Error>> {
    Err("rebuild royal with --features redis".into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let first = args.next();
    if first.as_deref() == Some("--scan-empty-jti") {
        return scan_empty_jti().await;
    }

    let path = first
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.toml"));
    let cfg: File = toml::from_str(&std::fs::read_to_string(&path)?)?;
    let node = resolve_snowflake_node(Some(cfg.this.snowflake_node))?;
    let idgen: Arc<dyn IdGenerator> = Arc::new(SnowflakeGen::try_new(node)?);

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
    let consul = env_or_cfg("CONSUL_HTTP_ADDR", &cfg.this.consul_url);
    let hmac = resolve_internal_hmac_secret(&cfg.this.hmac_secret);
    if is_demo_internal_hmac(&hmac) {
        tracing::warn!(secret = "demo-default-hmac", "do not use in production");
    }
    check_strict_runtime(StrictCheck {
        hmac: Some(&hmac),
        jwt: Some(&jwt.secret),
        redis_url: redis.as_deref(),
        require_redis: true,
        consul_addr: consul.as_deref(),
    })?;

    let revoke: Arc<dyn TokenRevocation> = match redis.as_deref() {
        Some(url) => open_revoke(url).await?,
        None => Arc::new(MemoryRevocation::new()),
    };
    let nonce: Arc<dyn chat::HmacNonceGuard> = match redis.as_deref() {
        Some(url) => open_nonce(url).await?,
        None => Arc::new(chat::MemoryHmacNonceGuard::new()),
    };

    let pending_receipt = pending_receipt_enabled();
    let state = if let Some(db) = env_or_cfg("DATABASE_URL", &cfg.this.database_url) {
        let sessions = match redis.as_deref() {
            None | Some("") => None,
            Some(url) => Some(open_uncached_session_store(url).await?),
        };
        let backends = open_pg_backends(
            &db,
            redis.as_deref(),
            idgen,
            PoolConfig::default(),
            sessions,
            pending_receipt,
        )
        .await?;
        RoyalState::with_backends(
            backends.store,
            backends.groups,
            backends.users,
            backends.social,
            jwt,
            revoke,
        )
        .with_pending_receipt(pending_receipt)
    } else {
        RoyalState::memory_with_jwt_receipt(idgen, jwt, pending_receipt).with_revoke(revoke)
    };
    let app = env_or_cfg("KIM_APP", &cfg.this.app).unwrap_or_else(|| ALLOWED_APP.into());
    if kim_protocol::strict_runtime() && app != ALLOWED_APP {
        return Err(format!("production KIM_APP must be {ALLOWED_APP}").into());
    }
    let chat_url = env_or_cfg("CHAT_URL", &cfg.this.chat_url).unwrap_or_default();
    let devices: Arc<dyn DeviceDirectory> = match env_or_cfg("DATABASE_URL", &cfg.this.database_url)
    {
        Some(db) => open_devices(&db).await?,
        None => Arc::new(MemoryDeviceDirectory::new()),
    };
    let device_hot: Arc<dyn DeviceHot> = match redis.as_deref() {
        Some(url) => open_device_hot(url).await?,
        None => Arc::new(MemoryDeviceHot::new()),
    };
    let state = state
        .with_app(app)
        .with_chat_url(chat_url)
        .with_hmac_secret(hmac)
        .with_nonce(nonce)
        .with_devices(devices)
        .with_device_hot(device_hot);
    state.start_maintenance();
    #[cfg(feature = "redis")]
    if let Some(url) = redis.as_deref() {
        let scanner = kim_session::RedisSessionStore::open(url).await?;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                match scanner.count_empty_jti_locations().await {
                    Ok(scan) => tracing::info!(
                        kim_location_without_jti = scan.empty_jti,
                        invalid = scan.invalid,
                        wrong_type = scan.wrong_type,
                        scanned = scan.scanned,
                        "location jti scan"
                    ),
                    Err(err) => tracing::warn!(%err, "location jti scan failed"),
                }
            }
        });
    }

    let listen = cfg.this.listen.clone();
    let public_address = env_or_cfg("KIM_PUBLIC_ADDRESS", &cfg.this.public_address);
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

    let registered = public_address.is_some();
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    let n2 = naming.clone();
    let sid = service_id.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            kim_core::wait_shutdown_signal().await;
            if registered {
                let _ = Naming::deregister(n2.as_ref(), &sid).await;
            }
        })
        .await?;
    Ok(())
}
