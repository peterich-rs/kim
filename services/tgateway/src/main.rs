use std::path::PathBuf;

use gateway::{load_config, run_gateway};
use kim_tcp::TcpServer;

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
    let server = TcpServer::bind(&cfg.listen).await?;
    run_gateway(cfg, server).await
}
