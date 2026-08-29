use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use kim_naming::{DefaultRegistration, Naming};

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
    let (listen, state) = router::load(&path)?;
    let public_address = std::env::var("KIM_PUBLIC_ADDRESS")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(addr) = &public_address {
        let port: u16 = listen
            .rsplit_once(':')
            .and_then(|(_, p)| p.parse().ok())
            .ok_or("listen has no port")?;
        let mut meta = HashMap::new();
        meta.insert("protocol".into(), "http".into());
        meta.insert("health_url".into(), format!("http://{addr}:{port}/health"));
        Naming::register(
            state.lookup.naming.as_ref(),
            DefaultRegistration {
                service_id: "router-1".into(),
                service_name: "router".into(),
                protocol: "http".into(),
                public_address: addr.clone(),
                public_port: port,
                tags: vec![],
                meta,
            },
        )
        .await?;
    }
    let app = router::app_from_state(state.clone())?;
    let addr: SocketAddr = listen.parse()?;
    tracing::info!(%addr, "router listen");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let naming = state.lookup.naming.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        if public_address.is_some() {
            let _ = Naming::deregister(naming.as_ref(), "router-1").await;
        }
        std::process::exit(0);
    });
    axum::serve(listener, app).await?;
    Ok(())
}
