use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, Selector};
use kim_core::Server;
use kim_metrics::KimMetrics;
use kim_naming::{open_naming, DefaultRegistration};
use kim_protocol::{check_strict_runtime, StrictCheck};
use serde::Deserialize;

use crate::selector::{Route, RouteFile, RouteSelector};
use crate::{
    resolve_jwt_secret, AllowAllRevoke, GatewayHandler, HttpRevoke, KickHook, MetricsHook,
    RevokeStore,
};

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
    #[serde(default)]
    services: Vec<ServiceRow>,
    #[serde(default)]
    route: Option<RouteFile>,
}

#[derive(Deserialize)]
struct SelfSection {
    service_id: String,
    service_name: String,
    listen: String,
    protocol: String,
    #[serde(default)]
    jwt_secret: String,
    #[serde(default)]
    metrics_listen: String,
    #[serde(default)]
    idc: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    public_address: String,
    #[serde(default)]
    public_port: u16,
    #[serde(default)]
    consul_url: String,
    #[serde(default)]
    adult_delay_ms: u64,
    #[serde(default)]
    royal_url: String,
    #[serde(default)]
    hmac_secret: String,
    #[serde(default)]
    token_ttl_secs: i64,
}

#[derive(Deserialize)]
struct ServiceRow {
    service_id: String,
    service_name: String,
    protocol: String,
    public_address: String,
    public_port: u16,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    domain: String,
}

pub struct GatewayConfig {
    pub listen: String,
    pub service_id: String,
    pub service_name: String,
    pub protocol: String,
    pub jwt_secret: String,
    pub metrics_listen: String,
    pub idc: String,
    pub domain: String,
    pub public_address: String,
    pub public_port: u16,
    pub consul_url: String,
    pub adult_delay_ms: u64,
    pub royal_url: String,
    pub hmac_secret: String,
    pub token_ttl_secs: i64,
    pub services: Vec<DefaultRegistration>,
    pub route: Option<RouteFile>,
}

pub fn load_config(path: &Path) -> Result<GatewayConfig, Box<dyn std::error::Error>> {
    let cfg: File = toml::from_str(&std::fs::read_to_string(path)?)?;
    let services = cfg
        .services
        .into_iter()
        .map(|s| {
            let mut meta = HashMap::new();
            if !s.domain.is_empty() {
                meta.insert("domain".into(), s.domain);
            }
            DefaultRegistration {
                service_id: s.service_id,
                service_name: s.service_name,
                protocol: s.protocol,
                public_address: s.public_address,
                public_port: s.public_port,
                tags: s.tags,
                meta,
            }
        })
        .collect();
    Ok(GatewayConfig {
        listen: cfg.this.listen,
        service_id: cfg.this.service_id,
        service_name: cfg.this.service_name,
        protocol: cfg.this.protocol,
        jwt_secret: resolve_jwt_secret(&cfg.this.jwt_secret),
        metrics_listen: cfg.this.metrics_listen,
        idc: cfg.this.idc,
        domain: cfg.this.domain,
        public_address: cfg.this.public_address,
        public_port: cfg.this.public_port,
        consul_url: cfg.this.consul_url,
        adult_delay_ms: cfg.this.adult_delay_ms,
        royal_url: cfg.this.royal_url,
        hmac_secret: cfg.this.hmac_secret,
        token_ttl_secs: cfg.this.token_ttl_secs,
        services,
        route: cfg.route,
    })
}

/// Open Redis-backed JWT revoke storage. A configured `REDIS_URL` must succeed;
/// callers must not skip revoke checks after a connection error.
pub async fn open_redis_revoke(url: Option<&str>) -> Result<Arc<RevokeStore>, String> {
    let url = url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "REDIS_URL is empty".to_string())?;
    Ok(Arc::new(RevokeStore::open(url).await?))
}

pub async fn run_gateway<S>(
    cfg: GatewayConfig,
    mut server: S,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: Server + Send + Sync + 'static,
{
    let consul = std::env::var("CONSUL_HTTP_ADDR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let t = cfg.consul_url.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });
    let redis_url = std::env::var("REDIS_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    check_strict_runtime(StrictCheck {
        hmac: None,
        jwt: Some(&cfg.jwt_secret),
        redis_url: redis_url.as_deref(),
        require_redis: true,
        consul_addr: consul.as_deref(),
    })?;
    let public_address = std::env::var("KIM_PUBLIC_ADDRESS")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| cfg.public_address.clone());
    let naming = if consul.is_some() {
        open_naming(consul.as_deref(), vec![])?
    } else {
        open_naming(None, cfg.services)?
    };
    let mut tags = Vec::new();
    if !cfg.idc.is_empty() {
        tags.push(format!("IDC:{}", cfg.idc));
    }
    let mut meta = HashMap::new();
    if !cfg.domain.is_empty() {
        meta.insert("domain".into(), cfg.domain.clone());
    }
    meta.insert("protocol".into(), cfg.protocol.clone());
    if !public_address.is_empty() {
        let health_port = cfg
            .metrics_listen
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse::<u16>().ok())
            .ok_or("metrics_listen required when public_address is set")?;
        meta.insert(
            "health_url".into(),
            format!("http://{public_address}:{health_port}/health"),
        );
    }
    let public_port = std::env::var("KIM_PUBLIC_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(cfg.public_port);
    let identity = DefaultRegistration {
        service_id: cfg.service_id.clone(),
        service_name: cfg.service_name.clone(),
        protocol: cfg.protocol,
        public_address,
        public_port,
        tags,
        meta,
    };
    let selector: Arc<dyn Selector> = match cfg.route {
        Some(file) => Arc::new(RouteSelector::new(Route::from_config(file))),
        None => Arc::new(HashSelector),
    };
    let kick = Arc::new(KickHook::new());
    let metrics = if cfg.metrics_listen.is_empty() {
        None
    } else {
        Some(KimMetrics::new(&cfg.service_id, &cfg.service_name)?)
    };
    let mut hooks: Vec<Arc<dyn kim_container::DownlinkHook>> = vec![kick.clone()];
    if let Some(m) = &metrics {
        hooks.push(Arc::new(MetricsHook(m.clone())));
    }
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: cfg.service_id.clone(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(cfg.adult_delay_ms),
        selector,
        after_downlink: hooks,
    });
    let ttl = std::env::var("KIM_TOKEN_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(if cfg.token_ttl_secs > 0 {
            cfg.token_ttl_secs
        } else {
            86_400
        });
    let handler = Arc::new(GatewayHandler::with_ttl(
        container.clone(),
        cfg.service_id.clone(),
        cfg.jwt_secret,
        ttl,
    ));
    if let Some(m) = &metrics {
        handler.with_metrics(m.clone());
    }
    if redis_url.is_some() {
        let store = open_redis_revoke(redis_url.as_deref()).await?;
        handler.set_redis(store.clone());
        handler.set_revoke(store);
    } else {
        let royal = std::env::var("ROYAL_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                let t = cfg.royal_url.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            });
        match royal {
            Some(base) => {
                let store = HttpRevoke::with_hmac(
                    &base,
                    &kim_protocol::resolve_internal_hmac_secret(&cfg.hmac_secret),
                )?;
                handler.set_revoke(Arc::new(store));
            }
            None => {
                // Production never reaches this: check_strict_runtime(require_redis: true)
                // already failed when REDIS_URL is missing. Local/demo/e2e (Memory chat,
                // no Redis, no Royal) still need a revoke store for login.
                tracing::warn!(
                    "do not use in production: JWT revoke checks disabled without REDIS_URL or ROYAL_URL"
                );
                handler.set_revoke(Arc::new(AllowAllRevoke));
            }
        }
    }
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler.clone());
    let server = Arc::new(server);
    kick.attach(server.clone());
    handler.attach_server(server.clone());
    container.attach_server(server);

    if let (Some(m), Ok(addr)) = (metrics, cfg.metrics_listen.parse::<SocketAddr>()) {
        let registry = m.registry();
        tokio::spawn(async move {
            if let Err(err) = kim_metrics::serve(addr, registry).await {
                tracing::warn!(%err, "metrics serve failed");
            }
        });
    }

    let c = container.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = c.shutdown().await;
    });
    container.start().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::open_redis_revoke;

    fn must_err(result: Result<Arc<super::RevokeStore>, String>, why: &str) -> String {
        match result {
            Ok(_) => panic!("{why}"),
            Err(err) => err,
        }
    }

    #[tokio::test]
    async fn allow_all_revoke_never_blocks_jti() {
        use crate::{AllowAllRevoke, RevokeCheck};
        let store = AllowAllRevoke;
        assert!(!store.is_revoked("any-jti").await.unwrap());
    }

    #[tokio::test]
    async fn missing_redis_url_is_error() {
        let err = must_err(open_redis_revoke(None).await, "empty url");
        assert!(err.contains("REDIS_URL"), "{err}");
    }

    #[tokio::test]
    async fn invalid_redis_url_is_error_not_skip() {
        let err = must_err(
            open_redis_revoke(Some("not-a-redis-url")).await,
            "bad url must fail",
        );
        assert!(!err.is_empty(), "{err}");
    }

    #[tokio::test]
    async fn unreachable_redis_is_error_not_skip() {
        let fut = open_redis_revoke(Some("redis://:secret@127.0.0.1:1/0"));
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), fut)
            .await
            .unwrap_or_else(|_| Err("timed out connecting to redis".into()));
        let err = must_err(result, "refused redis must fail start");
        assert!(!err.is_empty(), "{err}");
    }
}
