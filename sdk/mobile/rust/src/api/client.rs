use std::sync::Arc;

use kim_client::{
    ClientConfig, HistoryItem, InboxItem, IncomingTalk, LinkState, OutgoingContent, SessionEvent,
    SessionSupervisor, TalkResult,
};

use super::rt;
use crate::frb_generated::StreamSink;

pub struct KimTalkResult {
    pub message_id: i64,
    pub send_time: i64,
}

/// Wire content. `kind`: 1 text, 2 image, 3 voice, 4 video. `body` is text or URL.
pub struct KimOutgoingContent {
    pub kind: i32,
    pub body: String,
    pub extra: String,
}

pub struct KimInboxItem {
    pub dest: String,
    pub kind: i32,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub last_sender: String,
    pub last_message_id: i64,
    pub last_send_time: i64,
    pub unread: i32,
}

pub struct KimHistoryItem {
    pub message_id: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
    pub sender: String,
    pub send_time: i64,
    pub direction: i32,
}

/// Supervisor events. `kind` is link/inbox/talk/sync_progress/sync_done/sync_failed/kick/token/friend/group.
pub struct KimSessionEvent {
    pub kind: String,
    pub state: String,
    pub attempt: u32,
    pub items: Vec<KimInboxItem>,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub extra: String,
    pub message_id: i64,
    pub send_time: i64,
    pub command: String,
    pub msg_type: i32,
    pub pulled: u64,
    pub page_pending: bool,
    pub error: String,
    pub channel_id: String,
    pub token: String,
    pub exp: i64,
    pub nickname: String,
    pub members: Vec<String>,
}

impl KimSessionEvent {
    fn empty() -> Self {
        Self {
            kind: String::new(),
            state: String::new(),
            attempt: 0,
            items: Vec::new(),
            dest: String::new(),
            sender: String::new(),
            body: String::new(),
            extra: String::new(),
            message_id: 0,
            send_time: 0,
            command: String::new(),
            msg_type: 0,
            pulled: 0,
            page_pending: false,
            error: String::new(),
            channel_id: String::new(),
            token: String::new(),
            exp: 0,
            nickname: String::new(),
            members: Vec::new(),
        }
    }
}

/// Opaque handle. SessionSupervisor owns connect/login/sync/reconnect.
pub struct KimApi {
    supervisor: Arc<SessionSupervisor>,
}

impl KimApi {
    #[flutter_rust_bridge::frb(sync)]
    pub fn start(url: String, token: String, user_agent: String) -> Self {
        Self {
            supervisor: Arc::new(SessionSupervisor::new(
                ClientConfig::new(url, token).with_user_agent(user_agent),
            )),
        }
    }

    pub fn stop(&self) {
        self.supervisor.stop();
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn link_state(&self) -> String {
        match self.supervisor.state() {
            LinkState::Connecting => "Connecting".into(),
            LinkState::Online => "Online".into(),
            LinkState::Reconnecting { .. } => "Reconnecting".into(),
            LinkState::Offline => "Offline".into(),
        }
    }

    /// Supervisor event stream. Replaces `listen` / `KimPush`.
    #[flutter_rust_bridge::frb(sync)]
    pub fn session_events(&self, sink: StreamSink<KimSessionEvent>) -> Result<(), String> {
        let _guard = rt().enter();
        let mut rx = self.supervisor.events();
        self.supervisor.ensure_running();
        let _ = sink.add(map_link(&self.supervisor));
        let supervisor = self.supervisor.clone();
        rt().spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        let mapped = match ev {
                            SessionEvent::Link(_) => map_link(&supervisor),
                            other => map_event(other),
                        };
                        if sink.add(mapped).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "session event lag");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Ok(())
    }

    pub fn sync_confirm(&self, cursor: i64) -> Result<(), String> {
        self.supervisor.sync_confirm(cursor);
        Ok(())
    }

    pub fn notify_radio_up(&self) -> Result<(), String> {
        self.supervisor.notify_radio_up();
        Ok(())
    }

    pub fn notify_foreground(&self) -> Result<(), String> {
        self.supervisor.notify_foreground();
        Ok(())
    }

    pub fn send_message(
        &self,
        dest: String,
        kind: i32,
        content: KimOutgoingContent,
        client_id: String,
    ) -> Result<KimTalkResult, String> {
        let outgoing = match content.kind {
            2 => OutgoingContent::Image {
                url: content.body,
                extra: content.extra,
            },
            3 => OutgoingContent::Voice {
                url: content.body,
                extra: content.extra,
            },
            4 => OutgoingContent::Video {
                url: content.body,
                extra: content.extra,
            },
            _ => OutgoingContent::Text(content.body),
        };
        let client = self.supervisor.client();
        let result = rt()
            .block_on(client.send_message(&dest, kind, outgoing, &client_id))
            .map_err(|e| e.to_string())?;
        Ok(KimTalkResult::from(result))
    }

    pub fn history(
        &self,
        dest: String,
        kind: i32,
        before_id: i64,
        limit: i32,
    ) -> Result<Vec<KimHistoryItem>, String> {
        let client = self.supervisor.client();
        let items = rt()
            .block_on(client.history(&dest, kind, before_id, limit))
            .map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(KimHistoryItem::from).collect())
    }

    pub fn inbox(&self, limit: i32) -> Result<Vec<KimInboxItem>, String> {
        let client = self.supervisor.client();
        let items = rt()
            .block_on(client.inbox_list(limit))
            .map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(KimInboxItem::from).collect())
    }

    pub fn mark_read(&self, dest: String, kind: i32, message_id: i64) -> Result<(), String> {
        let client = self.supervisor.client();
        rt().block_on(client.mark_read(&dest, kind, message_id))
            .map_err(|e| e.to_string())
    }

    pub fn ack(&self, message_id: i64) -> Result<(), String> {
        let client = self.supervisor.client();
        rt().block_on(client.ack(message_id))
            .map_err(|e| e.to_string())
    }

    pub fn friend_request(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor.client();
        rt().block_on(client.friend_request(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_accept(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor.client();
        rt().block_on(client.friend_accept(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_reject(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor.client();
        rt().block_on(client.friend_reject(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_list(&self) -> Result<String, String> {
        let client = self.supervisor.client();
        let users = rt()
            .block_on(client.friend_list())
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }

    pub fn friend_incoming(&self) -> Result<String, String> {
        let client = self.supervisor.client();
        let users = rt()
            .block_on(client.friend_incoming())
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }

    pub fn profile(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor.client();
        let p = rt()
            .block_on(client.profile(&dest))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_one(&p)
    }

    pub fn update_profile(
        &self,
        nickname: String,
        avatar: String,
        bio: String,
    ) -> Result<String, String> {
        let client = self.supervisor.client();
        let p = rt()
            .block_on(client.update_profile(&nickname, &avatar, &bio))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_one(&p)
    }

    pub fn search_users(&self, query: String) -> Result<String, String> {
        let client = self.supervisor.client();
        let users = rt()
            .block_on(client.search_users(&query))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }
}

fn map_link(supervisor: &SessionSupervisor) -> KimSessionEvent {
    let mut ev = KimSessionEvent::empty();
    ev.kind = "link".into();
    match supervisor.state() {
        LinkState::Connecting => ev.state = "Connecting".into(),
        LinkState::Online => ev.state = "Online".into(),
        LinkState::Reconnecting { attempt } => {
            ev.state = "Reconnecting".into();
            ev.attempt = attempt;
            if let Some(reason) = supervisor.last_drop_reason() {
                ev.error = reason.as_str().into();
            }
        }
        LinkState::Offline => ev.state = "Offline".into(),
    }
    ev
}

fn map_event(event: SessionEvent) -> KimSessionEvent {
    match event {
        SessionEvent::Link(_) => {
            // Link events are mapped via `map_link(&supervisor)` in session_events.
            let mut ev = KimSessionEvent::empty();
            ev.kind = "link".into();
            ev
        }
        SessionEvent::Inbox(items) => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "inbox".into();
            ev.items = items.into_iter().map(KimInboxItem::from).collect();
            ev
        }
        SessionEvent::Talk(t) => KimSessionEvent::from(t),
        SessionEvent::SyncPage { page_id, talks } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "sync_page".into();
            ev.message_id = page_id;
            ev.page_pending = true;
            ev.body = serde_json::to_string(&talks).unwrap_or_else(|_| "[]".into());
            ev
        }
        SessionEvent::SyncProgress {
            pulled,
            page_pending,
        } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "sync_progress".into();
            ev.pulled = pulled as u64;
            ev.page_pending = page_pending;
            ev
        }
        SessionEvent::SyncDone { pulled } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "sync_done".into();
            ev.pulled = pulled as u64;
            ev
        }
        SessionEvent::SyncFailed(error) => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "sync_failed".into();
            ev.error = error;
            ev
        }
        SessionEvent::Kickout { channel_id } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "kick".into();
            ev.channel_id = channel_id;
            ev
        }
        SessionEvent::TokenRenew { token, exp } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "token".into();
            ev.token = token;
            ev.exp = exp;
            ev
        }
        SessionEvent::FriendRequest { from, nickname } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "friend".into();
            ev.dest = from.clone();
            ev.sender = from;
            ev.nickname = nickname;
            ev
        }
        SessionEvent::FriendAccepted { from, nickname } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "friend_accepted".into();
            ev.dest = from.clone();
            ev.sender = from;
            ev.nickname = nickname;
            ev
        }
        SessionEvent::GroupCreate { group_id, members } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "group".into();
            ev.dest = group_id;
            ev.members = members;
            ev
        }
        SessionEvent::AuthFailed { reason } => {
            let mut ev = KimSessionEvent::empty();
            ev.kind = "auth".into();
            ev.error = reason;
            ev
        }
    }
}

impl From<TalkResult> for KimTalkResult {
    fn from(r: TalkResult) -> Self {
        Self {
            message_id: r.message_id,
            send_time: r.send_time,
        }
    }
}

impl From<InboxItem> for KimInboxItem {
    fn from(i: InboxItem) -> Self {
        Self {
            dest: i.dest,
            kind: i.kind,
            title: i.title,
            avatar: i.avatar,
            last_body: i.last_body,
            last_sender: i.last_sender,
            last_message_id: i.last_message_id,
            last_send_time: i.last_send_time,
            unread: i.unread,
        }
    }
}

impl From<HistoryItem> for KimHistoryItem {
    fn from(h: HistoryItem) -> Self {
        Self {
            message_id: h.message_id,
            msg_type: h.msg_type,
            body: h.body,
            extra: h.extra,
            sender: h.sender,
            send_time: h.send_time,
            direction: h.direction,
        }
    }
}

impl From<IncomingTalk> for KimSessionEvent {
    fn from(t: IncomingTalk) -> Self {
        let mut ev = KimSessionEvent::empty();
        ev.kind = "talk".into();
        ev.dest = t.dest;
        ev.sender = t.sender;
        ev.body = t.body;
        ev.extra = t.extra;
        ev.message_id = t.message_id;
        ev.send_time = t.send_time;
        ev.command = t.command;
        ev.msg_type = t.msg_type;
        ev
    }
}
