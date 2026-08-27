use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kim_tcp::{ClientOptions, IdentityDialer, TcpClient};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "alice".to_string());
    let addr = args.next().unwrap_or_else(|| "127.0.0.1:8000".to_string());

    let mut client = TcpClient::new(
        id.clone(),
        "echo-client",
        ClientOptions {
            heartbeat: Some(Duration::from_secs(15)),
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(IdentityDialer));
    client.connect(&addr).await?;
    info!(%id, %addr, "connected");

    for i in 0..5 {
        let msg = format!("hello {i}");
        client.send(Bytes::from(msg.clone())).await?;
        let frame = client.read().await?;
        info!("recv: {}", String::from_utf8_lossy(&frame.payload));
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    client.close().await?;
    Ok(())
}
