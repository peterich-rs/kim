use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

use kim_core::{Conn, Error as CoreError, Frame, OpCode};
use kim_ws::connect_ws;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::ClientConfig;
use crate::events::{Event, TalkResult};
use crate::login::{login_on_conn, send_ping};
use crate::session::MemorySession;
use crate::wire::{decode_event, encode_ack, encode_user_talk};
use crate::ClientError;

/// Session/login/talk/ack. UI (Flutter / CLI) is a shell around this.
///
/// Connect uses `kim-ws` (`ws://` / `wss://` → [`kim_core::Conn`]). Login and
/// talk take `&mut dyn Conn`, so a future TCP/QUIC Conn impl plugs in without
/// changing packet code.
pub struct KimClient {
    config: ClientConfig,
    session: MemorySession,
    conn: Option<Mutex<Box<dyn Conn + Send>>>,
    buffered: Mutex<VecDeque<Frame>>,
    next_seq: AtomicU32,
}

impl KimClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            session: MemorySession::default(),
            conn: None,
            buffered: Mutex::new(VecDeque::new()),
            next_seq: AtomicU32::new(2),
        }
    }

    pub fn session(&self) -> &MemorySession {
        &self.session
    }

    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// HTTP Upgrade only. Does **not** send `login.signin`. Token stays off the URL.
    pub async fn connect(&mut self) -> Result<(), ClientError> {
        if self.conn.is_some() {
            return Err(ClientError::AlreadyConnected);
        }
        let url = self.config.url.clone();
        let conn = tokio::time::timeout(self.config.handshake_timeout, connect_ws(&url))
            .await
            .map_err(|_| ClientError::HandshakeTimeout(self.config.handshake_timeout))??;
        self.conn = Some(Mutex::new(Box::new(conn)));
        Ok(())
    }

    /// First business frame: JWT `login.signin`. Must follow [`Self::connect`].
    pub async fn login(&mut self) -> Result<MemorySession, ClientError> {
        let conn = self.conn.as_ref().ok_or(ClientError::NotConnected)?;
        let mut guard = conn.lock().await;
        let timeout = self.config.handshake_timeout;
        let session = login_on_conn(&mut **guard, &self.config.token, timeout).await?;
        self.session = session.clone();
        Ok(session)
    }

    pub async fn ping(&self) -> Result<(), ClientError> {
        let conn = self.conn.as_ref().ok_or(ClientError::NotConnected)?;
        let mut guard = conn.lock().await;
        send_ping(&mut **guard).await?;
        loop {
            let frame = read_data(&mut **guard).await?;
            match decode_event(&frame)? {
                Event::Pong => return Ok(()),
                Event::Closed => return Err(ClientError::from(CoreError::Closed)),
                _ => self.buffered.lock().await.push_back(frame),
            }
        }
    }

    pub async fn talk_to_user(&self, dest: &str, body: &str) -> Result<TalkResult, ClientError> {
        if !self.session.is_logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let client_id = Uuid::new_v4().to_string();
        let payload = encode_user_talk(seq, dest, body, &client_id);
        let conn = self.conn.as_ref().ok_or(ClientError::NotConnected)?;
        let mut guard = conn.lock().await;
        guard.write_frame(OpCode::Binary, payload).await?;
        guard.flush().await?;
        loop {
            let frame = read_data(&mut **guard).await?;
            match decode_event(&frame)? {
                Event::TalkResp(r) if r.sequence == seq => return Ok(r),
                Event::Status {
                    status, sequence, ..
                } if sequence == seq => {
                    return Err(ClientError::Status(status));
                }
                Event::Closed => return Err(ClientError::from(CoreError::Closed)),
                other => {
                    drop(other);
                    self.buffered.lock().await.push_back(frame);
                }
            }
        }
    }

    pub async fn ack(&self, message_id: i64) -> Result<(), ClientError> {
        if !self.session.is_logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let conn = self.conn.as_ref().ok_or(ClientError::NotConnected)?;
        let mut guard = conn.lock().await;
        guard
            .write_frame(OpCode::Binary, encode_ack(seq, message_id))
            .await?;
        guard.flush().await?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<Event, ClientError> {
        if let Some(frame) = self.buffered.lock().await.pop_front() {
            return decode_event(&frame);
        }
        let conn = self.conn.as_ref().ok_or(ClientError::NotConnected)?;
        let mut guard = conn.lock().await;
        let frame = read_data(&mut **guard).await?;
        decode_event(&frame)
    }

    pub async fn disconnect(&mut self) -> Result<(), ClientError> {
        if let Some(conn) = self.conn.take() {
            let mut guard = conn.lock().await;
            let _ = guard.write_frame(OpCode::Close, bytes::Bytes::new()).await;
            let _ = guard.shutdown().await;
        }
        self.session = MemorySession::default();
        self.buffered.lock().await.clear();
        Ok(())
    }
}

async fn read_data(conn: &mut dyn Conn) -> Result<Frame, ClientError> {
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

/// Used by tests: drive login/talk against any Conn (no kim-ws).
#[cfg(test)]
impl KimClient {
    pub(crate) fn with_conn(config: ClientConfig, conn: Box<dyn Conn + Send>) -> Self {
        Self {
            config,
            session: MemorySession::default(),
            conn: Some(Mutex::new(conn)),
            buffered: Mutex::new(VecDeque::new()),
            next_seq: AtomicU32::new(2),
        }
    }

    pub(crate) fn force_session(&mut self, session: MemorySession) {
        self.session = session;
    }
}
