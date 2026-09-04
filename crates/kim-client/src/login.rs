use std::time::Duration;

use kim_core::{Conn, Error as CoreError, Frame, OpCode};
use kim_protocol::pkt::{LoginResp, Status};
use kim_protocol::{read, Packet, CMD_LOGIN_SIGN_IN, CODE_PONG};

use crate::session::MemorySession;
use crate::token::account_from_token;
use crate::wire::{encode_login, encode_ping};
use crate::ClientError;

/// First business frame after WS Upgrade: JWT `login.signin`. Token is in the
/// body, never on the Upgrade URL.
pub async fn login_on_conn(
    conn: &mut dyn Conn,
    token: &str,
    timeout: Duration,
) -> Result<MemorySession, ClientError> {
    let account = account_from_token(token)?;
    conn.write_frame(OpCode::Binary, encode_login(token))
        .await?;
    conn.flush().await?;
    let channel_id = tokio::time::timeout(timeout, wait_login_resp(conn))
        .await
        .map_err(|_| ClientError::HandshakeTimeout(timeout))??;
    Ok(MemorySession {
        channel_id,
        account,
        token: token.to_string(),
    })
}

async fn wait_login_resp(conn: &mut dyn Conn) -> Result<String, ClientError> {
    loop {
        let frame = match conn.read_frame().await {
            Ok(f) => f,
            Err(CoreError::Closed) => {
                return Err(ClientError::Handshake("closed".into()));
            }
            Err(e) => return Err(e.into()),
        };
        match frame.opcode {
            OpCode::Close => return Err(ClientError::Handshake("closed".into())),
            OpCode::Ping => {
                let _ = conn.write_frame(OpCode::Pong, bytes::Bytes::new()).await;
            }
            OpCode::Pong | OpCode::Continuation => {}
            OpCode::Binary | OpCode::Text => match read(&frame.payload)? {
                Packet::Logic(p) => {
                    if p.header.command != CMD_LOGIN_SIGN_IN {
                        continue;
                    }
                    let st = p.header.status;
                    if st != Status::Success as i32 {
                        return Err(if st == Status::Unauthorized as i32 {
                            ClientError::Unauthorized
                        } else {
                            ClientError::Handshake(format!("status={st}"))
                        });
                    }
                    let resp: LoginResp = p.read_body()?;
                    if resp.channel_id.is_empty() || resp.channel_id == "alice" {
                        return Err(ClientError::Handshake("bad channel_id".into()));
                    }
                    return Ok(resp.channel_id);
                }
                Packet::Basic(_) => {}
            },
        }
    }
}

pub async fn send_ping(conn: &mut dyn Conn) -> Result<(), ClientError> {
    conn.write_frame(OpCode::Binary, encode_ping()).await?;
    conn.flush().await?;
    Ok(())
}

pub async fn wait_pong(conn: &mut dyn Conn, timeout: Duration) -> Result<(), ClientError> {
    let frame = tokio::time::timeout(timeout, read_data_frame(conn))
        .await
        .map_err(|_| ClientError::other("pong timeout"))??;
    match read(&frame.payload)? {
        Packet::Basic(p) if p.code == CODE_PONG => Ok(()),
        _ => Err(ClientError::other("expected pong")),
    }
}

async fn read_data_frame(conn: &mut dyn Conn) -> Result<Frame, ClientError> {
    loop {
        let frame = conn.read_frame().await?;
        match frame.opcode {
            OpCode::Close => return Err(ClientError::from(CoreError::Closed)),
            OpCode::Ping => {
                let _ = conn.write_frame(OpCode::Pong, bytes::Bytes::new()).await;
            }
            OpCode::Pong | OpCode::Continuation => {}
            OpCode::Binary | OpCode::Text => return Ok(frame),
        }
    }
}
