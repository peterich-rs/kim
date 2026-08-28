use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fake_gateway::{resolve_jwt_secret, GatewayHandler, KickHook};
use kim_container::{Container, ContainerOpts, InnerTcpDialer};
use kim_core::Server;
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_ws::WsServer;
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
    #[serde(default)]
    services: Vec<ServiceRow>,
}

#[derive(Deserialize)]
struct SelfSection {
    service_id: String,
    service_name: String,
    listen: String,
    protocol: String,
    #[serde(default)]
    jwt_secret: String,
}

#[derive(Deserialize)]
struct ServiceRow {
    service_id: String,
    service_name: String,
    protocol: String,
    public_address: String,
    public_port: u16,
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
    let jwt_secret = resolve_jwt_secret(&cfg.this.jwt_secret);

    let regs: Vec<DefaultRegistration> = cfg
        .services
        .into_iter()
        .map(|s| DefaultRegistration {
            service_id: s.service_id,
            service_name: s.service_name,
            protocol: s.protocol,
            public_address: s.public_address,
            public_port: s.public_port,
            tags: vec![],
            meta: HashMap::new(),
        })
        .collect();
    let naming = Arc::new(StaticNaming::from_slice(regs));
    let identity = DefaultRegistration {
        service_id: cfg.this.service_id.clone(),
        service_name: cfg.this.service_name,
        protocol: cfg.this.protocol,
        public_address: String::new(),
        public_port: 0,
        tags: vec![],
        meta: HashMap::new(),
    };

    let hook = Arc::new(KickHook::new());
    let mut server = WsServer::bind(&cfg.this.listen).await?;
    let container = Container::new(ContainerOpts {
        naming,
        identity,
        dialer: Arc::new(InnerTcpDialer {
            local_service_id: cfg.this.service_id.clone(),
        }),
        deps: vec!["chat".into()],
        adult_delay: Duration::from_millis(0),
        selector: Arc::new(kim_container::HashSelector),
        after_downlink: Some(hook.clone()),
    });
    let handler = Arc::new(GatewayHandler::new(
        container.clone(),
        cfg.this.service_id,
        jwt_secret,
    ));
    server.set_acceptor(handler.clone());
    server.set_message_listener(handler.clone());
    server.set_state_listener(handler);
    let server = Arc::new(server);
    hook.attach(server.clone());
    container.attach_server(server);

    let c = container.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = c.shutdown().await;
    });
    container.start().await?;
    Ok(())
}
