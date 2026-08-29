use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use kim_core::{Conn, Error, OpCode};
use kim_protocol::pkt::{Flag, GroupCreateReq, GroupCreateResp, MessageReq, Status};
use kim_protocol::{
    generate, marshal, read, LogicPkt, Packet, CMD_CHAT_GROUP_TALK, CMD_CHAT_USER_TALK,
    CMD_GROUP_CREATE, MESSAGE_TYPE_TEXT,
};
use kim_tcp::TcpConn;
use kim_ws::connect_ws;
use pkt_client::{perform_login, resolve_jwt_secret};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::Stats;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Login,
    User,
    Group,
}

#[derive(Clone, Debug)]
pub struct BenchOpts {
    pub address: String,
    pub secret: String,
    pub app: String,
    pub count: u64,
    pub threads: usize,
    pub timeout: Duration,
    pub keep: Duration,
    pub members: usize,
    pub online: f64,
}

impl Default for BenchOpts {
    fn default() -> Self {
        Self {
            address: "ws://127.0.0.1:8001/".into(),
            secret: resolve_jwt_secret(),
            app: "kim".into(),
            count: 100,
            threads: 10,
            timeout: Duration::from_secs(10),
            keep: Duration::ZERO,
            members: 20,
            online: 0.5,
        }
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mint(secret: &str, acc: &str, app: &str) -> Result<String, Error> {
    generate(secret, acc, app, now_ts() + 86400).map_err(|e| Error::Handshake(e.to_string()))
}

async fn connect(addr: &str) -> Result<Box<dyn Conn>, Error> {
    if addr.starts_with("ws://") || addr.starts_with("wss://") {
        let c = connect_ws(addr).await?;
        Ok(Box::new(c))
    } else {
        let host = addr.strip_prefix("tcp://").unwrap_or(addr);
        let stream = TcpStream::connect(host)
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        Ok(Box::new(TcpConn::new(stream)))
    }
}

fn handshake_status(err: &Error) -> i32 {
    match err {
        Error::Handshake(s) if s.starts_with("status=") => {
            s.trim_start_matches("status=").parse().unwrap_or(-1)
        }
        _ => -1,
    }
}

async fn wait_response(
    conn: &mut dyn Conn,
    command: &str,
    timeout: Duration,
) -> Result<i32, Error> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Error::other("timeout"));
        }
        let frame = tokio::time::timeout(left, conn.read_frame())
            .await
            .map_err(|_| Error::other("timeout"))??;
        match frame.opcode {
            OpCode::Close => return Err(Error::Closed),
            OpCode::Ping => {
                let _ = conn.write_frame(OpCode::Pong, Bytes::new()).await;
            }
            OpCode::Pong | OpCode::Continuation => {}
            OpCode::Binary | OpCode::Text => match read(&frame.payload) {
                Ok(Packet::Logic(p))
                    if p.header.command == command && p.header.flag == Flag::Response as i32 =>
                {
                    return Ok(p.header.status);
                }
                Ok(_) => {}
                Err(e) => return Err(Error::other(e.to_string())),
            },
        }
    }
}

pub async fn run_login(opts: BenchOpts) -> Result<Stats, Error> {
    let issued = Arc::new(AtomicU64::new(0));
    let stats = Arc::new(Mutex::new(Stats::new()));
    let run_id = now_ts() as u64;
    let threads = opts.threads.max(1);
    let total = opts.count;
    let mut joins = Vec::new();
    for t in 0..threads {
        let issued = issued.clone();
        let stats = stats.clone();
        let opts = opts.clone();
        joins.push(tokio::spawn(async move {
            loop {
                let i = issued.fetch_add(1, Ordering::SeqCst);
                if i >= total {
                    break;
                }
                let acc = format!("bench-{run_id}-{t}-{i}");
                let start = Instant::now();
                let (rt, status) = match login_once(&opts, &acc).await {
                    Ok(_) => (start.elapsed(), Status::Success as i32),
                    Err(e) => (start.elapsed(), handshake_status(&e)),
                };
                stats.lock().await.record(rt, status);
            }
        }));
    }
    for j in joins {
        let _ = j.await;
    }
    if !opts.keep.is_zero() {
        tokio::time::sleep(opts.keep).await;
    }
    let out = stats.lock().await.clone();
    Ok(out)
}

async fn login_once(opts: &BenchOpts, acc: &str) -> Result<Box<dyn Conn>, Error> {
    let token = mint(&opts.secret, acc, &opts.app)?;
    let mut conn = connect(&opts.address).await?;
    perform_login(conn.as_mut(), token).await?;
    Ok(conn)
}

pub async fn run_user(opts: BenchOpts) -> Result<Stats, Error> {
    let dest = format!("bench-dest-{}", now_ts());
    let mut dest_conn = login_once(&opts, &dest).await?;
    let threads = opts.threads.max(1);
    let mut senders: Vec<Box<dyn Conn>> = Vec::new();
    for t in 0..threads {
        let acc = format!("bench-send-{}-{t}", now_ts());
        senders.push(login_once(&opts, &acc).await?);
    }
    let issued = Arc::new(AtomicU64::new(0));
    let stats = Arc::new(Mutex::new(Stats::new()));
    let total = opts.count;
    let mut joins = Vec::new();
    for mut conn in senders {
        let issued = issued.clone();
        let stats = stats.clone();
        let dest = dest.clone();
        let timeout = opts.timeout;
        joins.push(tokio::spawn(async move {
            loop {
                let i = issued.fetch_add(1, Ordering::SeqCst);
                if i >= total {
                    break;
                }
                let start = Instant::now();
                let status = talk_once(conn.as_mut(), CMD_CHAT_USER_TALK, &dest, timeout)
                    .await
                    .unwrap_or(-1);
                stats.lock().await.record(start.elapsed(), status);
            }
        }));
    }
    for j in joins {
        let _ = j.await;
    }
    let _ = dest_conn.shutdown().await;
    let out = stats.lock().await.clone();
    Ok(out)
}

async fn talk_once(
    conn: &mut dyn Conn,
    command: &str,
    dest: &str,
    timeout: Duration,
) -> Result<i32, Error> {
    let mut pkt = LogicPkt::new(command, 2, Bytes::new());
    pkt.set_dest(dest);
    pkt.write_body(&MessageReq {
        r#type: MESSAGE_TYPE_TEXT,
        body: "bench".into(),
        extra: String::new(),
        client_id: uuid::Uuid::new_v4().to_string(),
    });
    conn.write_frame(OpCode::Binary, marshal(&Packet::Logic(pkt)))
        .await?;
    wait_response(conn, command, timeout).await
}

pub async fn run_group(opts: BenchOpts) -> Result<Stats, Error> {
    let owner = format!("bench-owner-{}", now_ts());
    let mut owner_conn = login_once(&opts, &owner).await?;
    let members: Vec<String> = (0..opts.members.max(1))
        .map(|i| format!("bench-m-{i}-{}", now_ts()))
        .collect();
    let mut create = LogicPkt::new(CMD_GROUP_CREATE, 2, Bytes::new());
    create.write_body(&GroupCreateReq {
        name: "bench".into(),
        avatar: String::new(),
        introduction: String::new(),
        owner: owner.clone(),
        members: members.clone(),
    });
    owner_conn
        .write_frame(OpCode::Binary, marshal(&Packet::Logic(create)))
        .await?;
    let group_id = wait_group_id(owner_conn.as_mut(), opts.timeout).await?;
    let online_n = ((opts.members as f64 * opts.online).floor() as usize).max(1);
    let mut online_conns: Vec<Box<dyn Conn>> = Vec::new();
    for acc in members.iter().take(online_n) {
        if acc == &owner {
            continue;
        }
        online_conns.push(login_once(&opts, acc).await?);
    }
    let stats = Arc::new(Mutex::new(Stats::new()));
    let timeout = opts.timeout;
    let dest = group_id.clone();
    for _ in 0..opts.count {
        let start = Instant::now();
        let status = talk_once(owner_conn.as_mut(), CMD_CHAT_GROUP_TALK, &dest, timeout)
            .await
            .unwrap_or(-1);
        stats.lock().await.record(start.elapsed(), status);
    }
    let _ = owner_conn.shutdown().await;
    for mut c in online_conns {
        let _ = c.shutdown().await;
    }
    let out = stats.lock().await.clone();
    Ok(out)
}

async fn wait_group_id(conn: &mut dyn Conn, timeout: Duration) -> Result<String, Error> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Error::other("timeout"));
        }
        let frame = tokio::time::timeout(left, conn.read_frame())
            .await
            .map_err(|_| Error::other("timeout"))??;
        match frame.opcode {
            OpCode::Binary | OpCode::Text => match read(&frame.payload) {
                Ok(Packet::Logic(p))
                    if p.header.command == CMD_GROUP_CREATE
                        && p.header.flag == Flag::Response as i32 =>
                {
                    if p.header.status != Status::Success as i32 {
                        return Err(Error::Handshake(format!("status={}", p.header.status)));
                    }
                    let resp: GroupCreateResp =
                        p.read_body().map_err(|e| Error::other(e.to_string()))?;
                    return Ok(resp.group_id);
                }
                _ => {}
            },
            OpCode::Ping => {
                let _ = conn.write_frame(OpCode::Pong, Bytes::new()).await;
            }
            OpCode::Close => return Err(Error::Closed),
            _ => {}
        }
    }
}
