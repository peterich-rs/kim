use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kim_protocol::pkt::Flag;
use kim_protocol::{marshal, read, BasicPkt, LogicPkt, Packet, CMD_DEMO_ECHO, CODE_PONG};
use kim_ws::{ClientOptions, WsClient, WsIdentityDialer};
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

    let mut client = WsClient::new(
        id,
        "pkt-client",
        ClientOptions {
            heartbeat: None,
            ..ClientOptions::default()
        },
    );
    client.set_dialer(Arc::new(WsIdentityDialer));
    client.connect(&url).await?;

    client
        .send(marshal(&Packet::Basic(BasicPkt::ping())))
        .await?;
    let pong = timeout_read(&client).await?;
    match read(&pong.payload)? {
        Packet::Basic(p) if p.code == CODE_PONG => info!("got basic pong"),
        _ => panic!("expected pong, got unexpected packet"),
    }

    if ping_only {
        client.close().await?;
        return Ok(());
    }

    let seq = 1u32;
    let req = LogicPkt::new(CMD_DEMO_ECHO, seq, Bytes::from_static(b"hello pkt"));
    client.send(marshal(&Packet::Logic(req))).await?;
    let resp = timeout_read(&client).await?;
    match read(&resp.payload)? {
        Packet::Logic(p) => {
            assert_eq!(p.header.sequence, seq);
            assert_eq!(p.header.flag, Flag::Response as i32);
            if expect_unavailable {
                assert_eq!(
                    p.header.status,
                    kim_protocol::pkt::Status::ServiceUnavailable as i32
                );
                info!("got ServiceUnavailable as expected");
            } else {
                assert_eq!(p.header.status, kim_protocol::pkt::Status::Success as i32);
                assert_eq!(&p.body[..], b"hello pkt");
                info!("got echo response");
            }
        }
        _ => panic!("expected logic"),
    }
    client.close().await?;
    Ok(())
}

async fn timeout_read(client: &WsClient) -> Result<kim_core::Frame, Box<dyn std::error::Error>> {
    Ok(tokio::time::timeout(Duration::from_secs(5), client.read()).await??)
}
