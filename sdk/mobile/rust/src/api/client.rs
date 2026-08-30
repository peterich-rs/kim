use std::sync::Arc;

use kim_client::{ClientConfig, Event, KimClient};

use super::rt;
use crate::frb_generated::StreamSink;

/// Push / kick / token / friend events after login.
pub struct KimPush {
    pub kind: String,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub extra: String,
    pub message_id: i64,
    pub send_time: i64,
    pub token: String,
    pub exp: i64,
}

impl KimPush {
    fn from_event(event: Event) -> Option<Self> {
        match event {
            Event::Talk(t) => Some(Self {
                kind: "talk".into(),
                dest: t.dest,
                sender: t.sender,
                body: t.body,
                extra: t.extra,
                message_id: t.message_id,
                send_time: t.send_time,
                token: String::new(),
                exp: 0,
            }),
            Event::Kickout { channel_id } => Some(Self {
                kind: "kick".into(),
                dest: String::new(),
                sender: String::new(),
                body: String::new(),
                extra: channel_id,
                message_id: 0,
                send_time: 0,
                token: String::new(),
                exp: 0,
            }),
            Event::TokenRenew { token, exp } => Some(Self {
                kind: "token".into(),
                dest: String::new(),
                sender: String::new(),
                body: String::new(),
                extra: String::new(),
                message_id: 0,
                send_time: 0,
                token,
                exp,
            }),
            Event::GroupCreate { group_id, .. } => Some(Self {
                kind: "group".into(),
                dest: group_id,
                sender: String::new(),
                body: String::new(),
                extra: String::new(),
                message_id: 0,
                send_time: 0,
                token: String::new(),
                exp: 0,
            }),
            Event::FriendRequest { from, nickname } => Some(Self {
                kind: "friend".into(),
                dest: from.clone(),
                sender: from,
                body: String::new(),
                extra: nickname,
                message_id: 0,
                send_time: 0,
                token: String::new(),
                exp: 0,
            }),
            Event::Closed => Some(Self {
                kind: "closed".into(),
                dest: String::new(),
                sender: String::new(),
                body: String::new(),
                extra: String::new(),
                message_id: 0,
                send_time: 0,
                token: String::new(),
                exp: 0,
            }),
            _ => None,
        }
    }
}

/// Opaque handle. UI is a shell; session/login/talk live here.
pub struct KimApi {
    inner: Arc<KimClient>,
}

impl KimApi {
    #[flutter_rust_bridge::frb(sync)]
    pub fn new(url: String, token: String, user_agent: String) -> Self {
        Self {
            inner: Arc::new(KimClient::new(
                ClientConfig::new(url, token).with_user_agent(user_agent),
            )),
        }
    }

    pub fn connect(&self) -> Result<String, String> {
        rt().block_on(self.inner.connect())
            .map_err(|e| e.to_string())?;
        Ok(format!("connected {}", self.inner.url()))
    }

    pub fn login(&self) -> Result<String, String> {
        let s = rt()
            .block_on(self.inner.login())
            .map_err(|e| e.to_string())?;
        Ok(format!("channel_id={} account={}", s.channel_id, s.account))
    }

    pub fn ping(&self) -> Result<String, String> {
        rt().block_on(self.inner.ping())
            .map_err(|e| e.to_string())?;
        Ok("pong".into())
    }

    pub fn talk_to_user(&self, dest: String, body: String) -> Result<String, String> {
        let r = rt()
            .block_on(self.inner.talk_to_user(&dest, &body))
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "message_id={} send_time={}",
            r.message_id, r.send_time
        ))
    }

    pub fn talk_image(&self, dest: String, url: String, extra: String) -> Result<String, String> {
        let r = rt()
            .block_on(self.inner.talk_image(&dest, &url, &extra))
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "message_id={} send_time={}",
            r.message_id, r.send_time
        ))
    }

    pub fn ack(&self, message_id: i64) -> Result<String, String> {
        rt().block_on(self.inner.ack(message_id))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    /// Unsolicited events after login. Does not hold the talk path.
    #[flutter_rust_bridge::frb(sync)]
    pub fn listen(&self, sink: StreamSink<KimPush>) -> Result<(), String> {
        let client = self.inner.clone();
        rt().spawn(async move {
            loop {
                match client.recv().await {
                    Ok(event) => {
                        let Some(push) = KimPush::from_event(event) else {
                            continue;
                        };
                        let closed = push.kind == "closed";
                        if sink.add(push).is_err() {
                            break;
                        }
                        if closed {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = sink.add(KimPush {
                            kind: "closed".into(),
                            dest: String::new(),
                            sender: String::new(),
                            body: String::new(),
                            extra: String::new(),
                            message_id: 0,
                            send_time: 0,
                            token: String::new(),
                            exp: 0,
                        });
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    pub fn friend_request(&self, dest: String) -> Result<String, String> {
        rt().block_on(self.inner.friend_request(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_accept(&self, dest: String) -> Result<String, String> {
        rt().block_on(self.inner.friend_accept(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_reject(&self, dest: String) -> Result<String, String> {
        rt().block_on(self.inner.friend_reject(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_list(&self) -> Result<String, String> {
        let users = rt()
            .block_on(self.inner.friend_list())
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }

    pub fn friend_incoming(&self) -> Result<String, String> {
        let users = rt()
            .block_on(self.inner.friend_incoming())
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }

    pub fn profile(&self, dest: String) -> Result<String, String> {
        let p = rt()
            .block_on(self.inner.profile(&dest))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_one(&p)
    }

    pub fn update_profile(
        &self,
        nickname: String,
        avatar: String,
        bio: String,
    ) -> Result<String, String> {
        let p = rt()
            .block_on(self.inner.update_profile(&nickname, &avatar, &bio))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_one(&p)
    }

    pub fn search_users(&self, query: String) -> Result<String, String> {
        let users = rt()
            .block_on(self.inner.search_users(&query))
            .map_err(|e| e.to_string())?;
        kim_client::Profile::encode_list(&users)
    }

    pub fn disconnect(&self) -> Result<String, String> {
        rt().block_on(self.inner.disconnect())
            .map_err(|e| e.to_string())?;
        Ok("disconnected".into())
    }
}
