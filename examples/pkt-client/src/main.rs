use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use kim_protocol::pkt::{
    Flag, GroupCreateReq, GroupCreateResp, MessageAckReq, MessageContentReq, MessageContentResp,
    MessageIndexReq, MessageIndexResp, MessagePush, MessageReq, MessageResp, Status,
};
use kim_protocol::{
    marshal, read, BasicPkt, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_TALK_ACK,
    CMD_CHAT_USER_TALK, CMD_DEMO_ECHO, CMD_GROUP_CREATE, CMD_OFFLINE_CONTENT, CMD_OFFLINE_INDEX,
    CODE_PONG, MESSAGE_TYPE_TEXT,
};
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
    let skip_ack = std::env::var("KIM_SKIP_ACK").ok().as_deref() == Some("1");
    let sync_offline = std::env::var("KIM_SYNC_OFFLINE").ok().as_deref() == Some("1");
    let talk_to = env_nonempty("KIM_TALK_TO");
    let group_members = env_nonempty("KIM_GROUP_MEMBERS");
    let talk_body = env_nonempty("KIM_TALK_BODY");
    let ack_from = env_nonempty("KIM_ACK_FROM")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let ack_delay = env_nonempty("KIM_ACK_DELAY_MS")
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(200));

    let mut dialer = LoginDialer::new(resolve_jwt_secret());
    if bad_token {
        dialer = dialer.with_bad_token();
    }
    let dialer = Arc::new(dialer);

    let mut client = WsClient::new(
        id.clone(),
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

    let mut seen = HashSet::new();
    let mut seq = 2u32;
    if sync_offline {
        seq = pull_offline(&client, seq, ack_from, &mut seen).await?;
    }

    if hold && talk_to.is_none() && group_members.is_none() {
        return hold_read_loop(&mut client, &channel_id, skip_ack, ack_delay, seen, seq).await;
    }

    if let Some(members) = group_members {
        ping_pong(&client).await?;
        let members: Vec<String> = members
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let mut pkt = LogicPkt::new(CMD_GROUP_CREATE, seq, Bytes::new());
        seq = seq.saturating_add(1);
        pkt.write_body(&GroupCreateReq {
            name: "demo".into(),
            avatar: String::new(),
            introduction: String::new(),
            owner: id,
            members,
        });
        client.send(marshal(&Packet::Logic(pkt))).await?;
        let frame = timeout_read(&client).await?;
        let group_id = match read(&frame.payload)? {
            Packet::Logic(p) => {
                if p.header.status != Status::Success as i32 {
                    return Err(format!("group create status {}", p.header.status).into());
                }
                let resp: GroupCreateResp = p.read_body()?;
                if resp.group_id.is_empty() {
                    return Err("empty group_id".into());
                }
                info!(group_id = %resp.group_id, "created group");
                resp.group_id
            }
            _ => return Err("expected GroupCreateResp".into()),
        };

        let body = talk_body.unwrap_or_else(|| "hellogroup".to_string());
        let mut pkt = LogicPkt::new(CMD_CHAT_GROUP_TALK, seq, Bytes::new());
        seq = seq.saturating_add(1);
        pkt.set_dest(group_id);
        pkt.write_body(&MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body,
            extra: String::new(),
        });
        client.send(marshal(&Packet::Logic(pkt))).await?;
        read_message_resp(&client).await?;
        if hold {
            return hold_read_loop(&mut client, &channel_id, skip_ack, ack_delay, seen, seq).await;
        }
        client.close().await?;
        return Ok(());
    }

    if let Some(dest) = talk_to {
        ping_pong(&client).await?;
        let body = talk_body.unwrap_or_else(|| "hello world".to_string());
        let mut pkt = LogicPkt::new(CMD_CHAT_USER_TALK, seq, Bytes::new());
        seq = seq.saturating_add(1);
        pkt.set_dest(dest);
        pkt.write_body(&MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body,
            extra: String::new(),
        });
        client.send(marshal(&Packet::Logic(pkt))).await?;
        read_message_resp(&client).await?;
        if hold {
            return hold_read_loop(&mut client, &channel_id, skip_ack, ack_delay, seen, seq).await;
        }
        client.close().await?;
        return Ok(());
    }

    ping_pong(&client).await?;

    if ping_only {
        client.close().await?;
        return Ok(());
    }

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

fn env_nonempty(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    }
}

async fn ping_pong(client: &WsClient) -> Result<(), Box<dyn std::error::Error>> {
    client
        .send(marshal(&Packet::Basic(BasicPkt::ping())))
        .await?;
    let pong = timeout_read(client).await?;
    match read(&pong.payload)? {
        Packet::Basic(p) if p.code == CODE_PONG => info!("got basic pong"),
        _ => return Err("expected pong, got unexpected packet".into()),
    }
    Ok(())
}

async fn read_message_resp(client: &WsClient) -> Result<(), Box<dyn std::error::Error>> {
    let frame = timeout_read(client).await?;
    match read(&frame.payload)? {
        Packet::Logic(p) => {
            if p.header.flag != Flag::Response as i32 {
                return Err("talk expected Response".into());
            }
            if p.header.status != Status::Success as i32 {
                return Err(format!("talk status {}", p.header.status).into());
            }
            let resp: MessageResp = p.read_body()?;
            if resp.message_id <= 10_000 {
                return Err(format!("message_id {}", resp.message_id).into());
            }
            if resp.send_time <= 1000 {
                return Err(format!("send_time {}", resp.send_time).into());
            }
            info!(
                message_id = resp.message_id,
                send_time = resp.send_time,
                "got MessageResp"
            );
            Ok(())
        }
        _ => Err("expected MessageResp".into()),
    }
}

async fn pull_offline(
    client: &WsClient,
    mut seq: u32,
    ack_from: i64,
    seen: &mut HashSet<i64>,
) -> Result<u32, Box<dyn std::error::Error>> {
    let mut pkt = LogicPkt::new(CMD_OFFLINE_INDEX, seq, Bytes::new());
    pkt.write_body(&MessageIndexReq {
        message_id: ack_from,
    });
    seq = seq.saturating_add(1);
    client.send(marshal(&Packet::Logic(pkt))).await?;
    let frame = timeout_read(client).await?;
    let indexes = match read(&frame.payload)? {
        Packet::Logic(p) if p.header.status == Status::Success as i32 => {
            let resp: MessageIndexResp = p.read_body()?;
            resp.indexes
        }
        Packet::Logic(p) => {
            return Err(format!("offline index status {}", p.header.status).into());
        }
        _ => return Err("expected offline index".into()),
    };
    info!(count = indexes.len(), "offline index");
    let ids: Vec<i64> = indexes.iter().map(|i| i.message_id).collect();
    for chunk in ids.chunks(200) {
        if chunk.is_empty() {
            continue;
        }
        let mut pkt = LogicPkt::new(CMD_OFFLINE_CONTENT, seq, Bytes::new());
        pkt.write_body(&MessageContentReq {
            message_ids: chunk.to_vec(),
        });
        seq = seq.saturating_add(1);
        client.send(marshal(&Packet::Logic(pkt))).await?;
        let frame = timeout_read(client).await?;
        match read(&frame.payload)? {
            Packet::Logic(p) if p.header.status == Status::Success as i32 => {
                let resp: MessageContentResp = p.read_body()?;
                for m in resp.messages {
                    if seen.insert(m.message_id) {
                        info!(
                            message_id = m.message_id,
                            body_len = m.body.len(),
                            "offline content"
                        );
                    }
                }
            }
            Packet::Logic(p) => {
                return Err(format!("offline content status {}", p.header.status).into());
            }
            _ => return Err("expected offline content".into()),
        }
    }
    Ok(seq)
}

async fn hold_read_loop(
    client: &mut WsClient,
    channel_id: &str,
    skip_ack: bool,
    ack_delay: Duration,
    mut seen: HashSet<i64>,
    mut seq: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut max_id: i64 = 0;
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
            if p.header.flag == Flag::Push as i32
                && (p.header.command == CMD_CHAT_USER_TALK
                    || p.header.command == CMD_CHAT_GROUP_TALK)
            {
                let push: MessagePush = p.read_body()?;
                if seen.insert(push.message_id) {
                    info!(
                        message_id = push.message_id,
                        sender = %push.sender,
                        msg_type = push.r#type,
                        body_len = push.body.len(),
                        "got talk push"
                    );
                }
                if push.message_id > max_id {
                    max_id = push.message_id;
                }
                if !skip_ack && max_id > 0 {
                    tokio::time::sleep(ack_delay).await;
                    let mut ack = LogicPkt::new(CMD_CHAT_TALK_ACK, seq, Bytes::new());
                    seq = seq.saturating_add(1);
                    ack.write_body(&MessageAckReq { message_id: max_id });
                    client.send(marshal(&Packet::Logic(ack))).await?;
                }
            }
        }
    }
}

async fn timeout_read(client: &WsClient) -> Result<kim_core::Frame, Box<dyn std::error::Error>> {
    Ok(tokio::time::timeout(Duration::from_secs(5), client.read()).await??)
}
