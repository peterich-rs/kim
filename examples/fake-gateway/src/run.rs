use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kim_container::{Container, ContainerOpts, HashSelector, InnerTcpDialer, Selector};
use kim_core::Server;
use kim_metrics::KimMetrics;
use kim_naming::{DefaultRegistration, StaticNaming};
use serde::Deserialize;

use crate::selector::{Route, RouteFile, RouteSelector};
use crate::{resolve_jwt_secret, GatewayHandler, KickHook, MetricsHook};

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
        services,
        route: cfg.route,
    })
}

pub async fn run_gateway<S>(
    cfg: GatewayConfig,
    mut server: S,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: Server + Send + Sync + 'static,
{
    let naming = Arc::new(StaticNaming::from_slice(cfg.services));
    let mut tags = Vec::new();
    if !cfg.idc.is_empty() {
        tags.push(format!("IDC:{}", cfg.idc));
    }
    let mut meta = HashMap::new();
    if !cfg.domain.is_empty() {
        meta.insert("domain".into(), cfg.domain.clone());
    }
    let identity = DefaultRegistration {
        service_id: cfg.service_id.clone(),
        service_name: cfg.service_name.clone(),
        protocol: cfg.protocol,
        public_address: String::new(),
        public_port: 0,
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
        adult_delay: Duration::from_millis(0),
        selector,
        after_downlink: hooks,
    });
    let handler = Arc::new(GatewayHandler::new(
        container.clone(),
        cfg.service_id.clone(),
        cfg.jwt_secret,
    ));
    if let Some(m) = &metrics {
        handler.with_metrics(m.clone());
    }
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let server = Arc::new(server);
    kick.attach(server.clone());
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
