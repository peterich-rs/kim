//! CLI shell around `kim-client`. UI (Flutter) should call the same API.
//!
//! ```text
//! cargo run -p kim-client-demo -- alice
//! cargo run -p kim-client-demo -- alice wss://kim.ainexc.com/
//! KIM_TALK_TO=bob cargo run -p kim-client-demo -- alice
//! ```
//!
//! Token: `KIM_TOKEN` if set, else demo-only local mint (same as pkt-client).
//! Never put a live secret in the repo.

use std::time::{SystemTime, UNIX_EPOCH};

use kim_client::{ClientConfig, KimClient, DEFAULT_LOCAL_URL};
use kim_protocol::{generate, DEMO_DEFAULT_SECRET};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let account = args.next().unwrap_or_else(|| "alice".to_string());
    let url = args
        .next()
        .or_else(|| nonempty("KIM_WS_URL"))
        .unwrap_or_else(|| DEFAULT_LOCAL_URL.to_string());
    let token = match nonempty("KIM_TOKEN") {
        Some(t) => t,
        None => {
            let exp = now_ts() + 86_400;
            generate(DEMO_DEFAULT_SECRET, &account, "kim", exp)?
        }
    };
    let talk_to = nonempty("KIM_TALK_TO");
    let body = nonempty("KIM_TALK_BODY").unwrap_or_else(|| "hello from kim-client".to_string());

    let client = KimClient::new(ClientConfig::new(url.clone(), token).with_env_url());
    info!(url = client.url(), "connecting");
    client.connect().await?;
    let session = client.login().await?;
    info!(channel_id = %session.channel_id, account = %session.account, "logined");
    client.ping().await?;
    info!("pong");
    if let Some(dest) = talk_to {
        let r = client.talk_to_user(&dest, &body).await?;
        info!(
            message_id = r.message_id,
            send_time = r.send_time,
            "talk ok"
        );
    }
    client.disconnect().await?;
    Ok(())
}

fn nonempty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
