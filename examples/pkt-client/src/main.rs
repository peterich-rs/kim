use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kim_protocol::pkt::{Flag, Status};
use kim_protocol::{marshal, read, BasicPkt, LogicPkt, Packet, CMD_DEMO_ECHO, CODE_PONG};
use kim_ws::{ClientOptions, WsClient};
use pkt_client::{is_kickout, resolve_jwt_secret, LoginDialer};
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
    let url = args
        .next()
        .unwrap_or_else(|| "ws://127.0.0.1:8001/".to_string());
    let expect_unavailable = std::env::var("KIM_EXPECT_UNAVAILABLE").ok().as_deref() == Some("1");
    let ping_only = std::env::var("KIM_PING_ONLY").ok().as_deref() == Some("1");
    let bad_token = std::env::var("KIM_BAD_TOKEN").ok().as_deref() == Some("1");
    let hold = std::env::var("KIM_HOLD").ok().as_deref() == Some("1");

    let mut dialer = LoginDialer::new(resolve_jwt_secret());
    if bad_token {
        dialer = dialer.with_bad_token();
    }
    let dialer = Arc::new(dialer);

    let mut client = WsClient::new(
        id,
        "pkt-client",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(dialer.clone());
    match client.connect(&url).await {
        Ok(()) => {
            if bad_token || expect_unavailable {
                return Err("expected handshake failure".into());
            }
        }
        Err(err) => {
            if bad_token || expect_unavailable {
                info!(%err, "handshake failed as expected");
                return Ok(());
            }
            return Err(err.into());
        }
    }

    let channel_id = dialer
        .channel_id()
        .ok_or("login succeeded without channel_id")?;
    info!(channel_id, "logined");

    if hold {
        loop {
            let frame = client.read().await?;
            if let Packet::Logic(p) = read(&frame.payload)? {
                if let Some(notify) = is_kickout(&p) {
                    if notify.channel_id != channel_id {
                        return Err(format!(
                            "kickout channel_id {} != {}",
                            notify.channel_id, channel_id
                        )
                        .into());
                    }
                    info!(channel_id = %notify.channel_id, "got kickout");
                    client.close().await?;
                    return Ok(());
                }
            }
        }
    }

    client
        .send(marshal(&Packet::Basic(BasicPkt::ping())))
        .await?;
    let pong = timeout_read(&client).await?;
    match read(&pong.payload)? {
        Packet::Basic(p) if p.code == CODE_PONG => info!("got basic pong"),
        _ => return Err("expected pong, got unexpected packet".into()),
    }

    if ping_only {
        client.close().await?;
        return Ok(());
    }

    let seq = 2u32;
    let req = LogicPkt::new(CMD_DEMO_ECHO, seq, Bytes::from_static(b"hello pkt"));
    client.send(marshal(&Packet::Logic(req))).await?;
    let resp = timeout_read(&client).await?;
    match read(&resp.payload)? {
        Packet::Logic(p) => {
            if p.header.sequence != seq {
                return Err("echo sequence mismatch".into());
            }
            if p.header.flag != Flag::Response as i32 {
                return Err("echo expected Response".into());
            }
            if p.header.status != Status::Success as i32 {
                return Err(format!("echo status {}", p.header.status).into());
            }
            if p.body.as_ref() != b"hello pkt" {
                return Err("echo body mismatch".into());
            }
            info!("got echo response");
        }
        _ => return Err("expected logic".into()),
    }
    client.close().await?;
    Ok(())
}

async fn timeout_read(client: &WsClient) -> Result<kim_core::Frame, Box<dyn std::error::Error>> {
    Ok(tokio::time::timeout(Duration::from_secs(5), client.read()).await??)
}
