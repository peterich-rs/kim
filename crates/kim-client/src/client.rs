use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use kim_core::{Conn, Error as CoreError, Frame, OpCode};
use kim_ws::{connect_ws_with_user_agent, WsHandshakeConn};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::ClientConfig;
use crate::events::{Event, Profile, TalkResult};
use crate::login::{login_on_conn, send_ping};
use crate::pump::{start_split_pump, Live};
use crate::session::MemorySession;
use crate::wire::{
    decode_event, encode_ack, encode_dest_cmd, encode_empty_cmd, encode_ping, encode_user_search,
    encode_user_talk,
};
use crate::ClientError;
use kim_protocol::{
    CMD_FRIEND_ACCEPT, CMD_FRIEND_INCOMING, CMD_FRIEND_LIST, CMD_FRIEND_REJECT, CMD_FRIEND_REQUEST,
};

enum Io {
    Off,
    Handshake(WsHandshakeConn),
    #[allow(dead_code)]
    Conn(Box<dyn Conn + Send>),
    Live(Arc<Live>),
}

/// Session/login/talk/ack. UI (Flutter / CLI) is a shell around this.
///
/// Connect uses `kim-ws` (`ws://` / `wss://` → [`kim_core::Conn`]). After
/// `login.signin` the handshake socket is split: a reader task demuxes pushes
/// vs request/response so [`Self::recv`] and [`Self::talk_to_user`] can run
/// together. Tests that inject a `Conn` stay sequential.
pub struct KimClient {
    config: ClientConfig,
    session: StdMutex<MemorySession>,
    io: Mutex<Io>,
    buffered: Mutex<VecDeque<Frame>>,
    next_seq: AtomicU32,
}

impl KimClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            session: StdMutex::new(MemorySession::default()),
            io: Mutex::new(Io::Off),
            buffered: Mutex::new(VecDeque::new()),
            next_seq: AtomicU32::new(2),
        }
    }

    pub fn session(&self) -> MemorySession {
        lock_session(&self.session)
    }

    pub fn url(&self) -> &str {
        &self.config.url
    }

    fn logged_in(&self) -> bool {
        lock_session(&self.session).is_logged_in()
    }

    fn store_session(&self, session: MemorySession) {
        *lock_session_mut(&self.session) = session;
    }

    /// HTTP Upgrade only. Does **not** send `login.signin`. Token stays off the URL.
    pub async fn connect(&self) -> Result<(), ClientError> {
        let mut io = self.io.lock().await;
        if !matches!(*io, Io::Off) {
            return Err(ClientError::AlreadyConnected);
        }
        let url = self.config.url.clone();
        let user_agent = self.config.user_agent.clone();
        let conn = tokio::time::timeout(
            self.config.handshake_timeout,
            connect_ws_with_user_agent(&url, &user_agent),
        )
        .await
        .map_err(|_| ClientError::HandshakeTimeout(self.config.handshake_timeout))??;
        *io = Io::Handshake(conn);
        Ok(())
    }

    /// First business frame: JWT `login.signin`. Must follow [`Self::connect`].
    /// On the WGateway path this also starts the read/write pump.
    pub async fn login(&self) -> Result<MemorySession, ClientError> {
        let mut io = self.io.lock().await;
        let taken = std::mem::replace(&mut *io, Io::Off);
        match taken {
            Io::Handshake(mut ws) => {
                match login_on_conn(&mut ws, &self.config.token, self.config.handshake_timeout)
                    .await
                {
                    Ok(session) => {
                        let (read, write) = ws.split_conn();
                        *io = Io::Live(start_split_pump(read, write));
                        self.store_session(session.clone());
                        Ok(session)
                    }
                    Err(err) => {
                        *io = Io::Handshake(ws);
                        Err(err)
                    }
                }
            }
            Io::Conn(mut conn) => {
                match login_on_conn(
                    &mut *conn,
                    &self.config.token,
                    self.config.handshake_timeout,
                )
                .await
                {
                    Ok(session) => {
                        *io = Io::Conn(conn);
                        self.store_session(session.clone());
                        Ok(session)
                    }
                    Err(err) => {
                        *io = Io::Conn(conn);
                        Err(err)
                    }
                }
            }
            other => {
                *io = other;
                Err(ClientError::NotConnected)
            }
        }
    }

    pub async fn ping(&self) -> Result<(), ClientError> {
        if let Some(live) = self.live().await {
            return live.ping(encode_ping()).await;
        }
        let mut io = self.io.lock().await;
        let conn = conn_mut(&mut io)?;
        send_ping(conn).await?;
        loop {
            let frame = read_data(conn).await?;
            match decode_event(&frame)? {
                Event::Pong => return Ok(()),
                Event::Closed => return Err(ClientError::from(CoreError::Closed)),
                _ => self.buffered.lock().await.push_back(frame),
            }
        }
    }

    pub async fn talk_to_user(&self, dest: &str, body: &str) -> Result<TalkResult, ClientError> {
        if !self.logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let client_id = Uuid::new_v4().to_string();
        let payload = encode_user_talk(seq, dest, body, &client_id);
        self.write_wait(payload, seq, |ev| match ev {
            Event::TalkResp(r) if r.sequence == seq => Some(Ok(r.clone())),
            Event::Status {
                status, sequence, ..
            } if *sequence == seq => Some(Err(ClientError::Status(*status))),
            _ => None,
        })
        .await
    }

    pub async fn friend_request(&self, dest: &str) -> Result<(), ClientError> {
        self.dest_status(CMD_FRIEND_REQUEST, dest).await
    }

    pub async fn friend_accept(&self, dest: &str) -> Result<(), ClientError> {
        self.dest_status(CMD_FRIEND_ACCEPT, dest).await
    }

    pub async fn friend_reject(&self, dest: &str) -> Result<(), ClientError> {
        self.dest_status(CMD_FRIEND_REJECT, dest).await
    }

    pub async fn friend_list(&self) -> Result<Vec<Profile>, ClientError> {
        self.user_list(CMD_FRIEND_LIST).await
    }

    pub async fn friend_incoming(&self) -> Result<Vec<Profile>, ClientError> {
        self.user_list(CMD_FRIEND_INCOMING).await
    }

    pub async fn search_users(&self, query: &str) -> Result<Vec<Profile>, ClientError> {
        if !self.logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        self.write_wait(encode_user_search(seq, query), seq, |ev| match ev {
            Event::UserList {
                sequence, users, ..
            } if *sequence == seq => Some(Ok(users.clone())),
            Event::Status {
                status, sequence, ..
            } if *sequence == seq => Some(Err(ClientError::Status(*status))),
            _ => None,
        })
        .await
    }

    async fn dest_status(&self, command: &str, dest: &str) -> Result<(), ClientError> {
        if !self.logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        self.write_wait(encode_dest_cmd(command, seq, dest), seq, |ev| match ev {
            Event::Status {
                status, sequence, ..
            } if *sequence == seq => {
                if *status == 0 {
                    Some(Ok(()))
                } else {
                    Some(Err(ClientError::Status(*status)))
                }
            }
            _ => None,
        })
        .await
    }

    async fn user_list(&self, command: &str) -> Result<Vec<Profile>, ClientError> {
        if !self.logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        self.write_wait(encode_empty_cmd(command, seq), seq, |ev| match ev {
            Event::UserList {
                sequence, users, ..
            } if *sequence == seq => Some(Ok(users.clone())),
            Event::Status {
                status, sequence, ..
            } if *sequence == seq => Some(Err(ClientError::Status(*status))),
            _ => None,
        })
        .await
    }

    async fn write_wait<T>(
        &self,
        payload: bytes::Bytes,
        seq: u32,
        mut take: impl FnMut(&Event) -> Option<Result<T, ClientError>>,
    ) -> Result<T, ClientError> {
        if let Some(live) = self.live().await {
            return live.write_wait(payload, seq, take).await;
        }
        let mut io = self.io.lock().await;
        let conn = conn_mut(&mut io)?;
        conn.write_frame(OpCode::Binary, payload).await?;
        conn.flush().await?;
        loop {
            let frame = read_data(conn).await?;
            let event = decode_event(&frame)?;
            if let Some(done) = take(&event) {
                let _ = seq;
                return done;
            }
            match event {
                Event::Closed => return Err(ClientError::from(CoreError::Closed)),
                _ => self.buffered.lock().await.push_back(frame),
            }
        }
    }

    pub async fn ack(&self, message_id: i64) -> Result<(), ClientError> {
        if !self.logged_in() {
            return Err(ClientError::NotLoggedIn);
        }
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let payload = encode_ack(seq, message_id);
        if let Some(live) = self.live().await {
            return live.write_frame(OpCode::Binary, payload).await;
        }
        let mut io = self.io.lock().await;
        let conn = conn_mut(&mut io)?;
        conn.write_frame(OpCode::Binary, payload).await?;
        conn.flush().await?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<Event, ClientError> {
        loop {
            let event = self.next_event().await?;
            if is_unsolicited(&event) {
                return Ok(event);
            }
        }
    }

    async fn next_event(&self) -> Result<Event, ClientError> {
        if let Some(live) = self.live().await {
            return live.recv().await;
        }
        if let Some(frame) = self.buffered.lock().await.pop_front() {
            return decode_event(&frame);
        }
        let mut io = self.io.lock().await;
        let conn = conn_mut(&mut io)?;
        let frame = read_data(conn).await?;
        decode_event(&frame)
    }

    async fn live(&self) -> Option<Arc<Live>> {
        match &*self.io.lock().await {
            Io::Live(live) => Some(live.clone()),
            _ => None,
        }
    }

    pub async fn disconnect(&self) -> Result<(), ClientError> {
        let mut io = self.io.lock().await;
        let prev = std::mem::replace(&mut *io, Io::Off);
        drop(io);
        match prev {
            Io::Live(live) => live.shutdown(),
            Io::Handshake(mut ws) => {
                let _ = ws.write_frame(OpCode::Close, bytes::Bytes::new()).await;
                let _ = ws.shutdown().await;
            }
            Io::Conn(mut conn) => {
                let _ = conn.write_frame(OpCode::Close, bytes::Bytes::new()).await;
                let _ = conn.shutdown().await;
            }
            Io::Off => {}
        }
        self.store_session(MemorySession::default());
        self.buffered.lock().await.clear();
        Ok(())
    }
}

fn lock_session(session: &StdMutex<MemorySession>) -> MemorySession {
    session.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn lock_session_mut(session: &StdMutex<MemorySession>) -> std::sync::MutexGuard<'_, MemorySession> {
    session.lock().unwrap_or_else(|e| e.into_inner())
}

fn conn_mut(io: &mut Io) -> Result<&mut dyn Conn, ClientError> {
    match io {
        Io::Conn(conn) => Ok(&mut **conn),
        Io::Handshake(ws) => Ok(ws),
        _ => Err(ClientError::NotConnected),
    }
}

fn is_unsolicited(event: &Event) -> bool {
    matches!(
        event,
        Event::Talk(_)
            | Event::Kickout { .. }
            | Event::TokenRenew { .. }
            | Event::GroupCreate { .. }
            | Event::FriendRequest { .. }
            | Event::Closed
    )
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
            session: StdMutex::new(MemorySession::default()),
            io: Mutex::new(Io::Conn(conn)),
            buffered: Mutex::new(VecDeque::new()),
            next_seq: AtomicU32::new(2),
        }
    }

    pub(crate) fn force_session(&self, session: MemorySession) {
        self.store_session(session);
    }
}
