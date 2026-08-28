use std::path::PathBuf;

use fake_chat::idgen::resolve_snowflake_node;
use fake_royal::{serve, RoyalState};
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
}

fn default_node() -> u16 {
    1
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
    let listener = tokio::net::TcpListener::bind(&cfg.this.listen).await?;
    serve(listener, RoyalState::with_snowflake(node)).await?;
    Ok(())
}
