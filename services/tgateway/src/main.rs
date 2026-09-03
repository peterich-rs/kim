use std::path::PathBuf;

use gateway::{load_config, run_gateway};
use kim_tcp::{SocketOpts, TcpServer};
use serde::Deserialize;

mod tls;

#[derive(Deserialize, Default)]
struct TlsSection {
    #[serde(default)]
    tls_cert: String,
    #[serde(default)]
    tls_key: String,
    #[serde(default)]
    max_connections: Option<usize>,
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
    let cfg = load_config(&path)?;
    let extra: TlsSection = toml::from_str(&std::fs::read_to_string(&path)?)?;
    let tls = tls::load_tls(&extra.tls_cert, &extra.tls_key)?;
    let server = TcpServer::bind(&cfg.listen).await?;
    server.set_socket_opts(SocketOpts::default());
    server.set_max_connections(extra.max_connections);
    match tls {
        None => run_gateway(cfg, server).await,
        Some(acceptor) => {
            let server = tls::TlsFrontend::wrap(server, acceptor, None);
            run_gateway(cfg, server).await
        }
    }
}
