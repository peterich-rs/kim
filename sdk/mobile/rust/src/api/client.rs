use std::sync::Arc;

use kim_client::{
    BotPendingItem, HistoryItem, InboxItem, LinkState, OutgoingContent, SessionSupervisor,
    TalkResult,
};
use kim_sdk::{KimSdk, MediaRef, OutgoingPayload, ReadMarker, SendMessageCommand, StartSession};

use super::rt;
use super::types::{
    AgentProfileDto, AgentRunRequestDto, AgentRunResultDto, MessagePageDto, PersonDto, ProfileDto,
    RoomMemberDto, SdkErrorDto, SendStatusDto, SessionSnapshotDto, SessionUpdateDto, SettingsDto,
    TimelineUpdateDto, TokenPersistDto,
};
use crate::frb_generated::StreamSink;

pub struct KimTalkResult {
    pub message_id: i64,
    pub send_time: i64,
}

pub struct KimCommandReceipt {
    pub request_id: String,
    pub client_id: String,
    pub dest: String,
    pub accepted_at: i64,
    pub send_status: SendStatusDto,
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

pub struct KimBotPendingItem {
    pub message_id: i64,
    pub body: String,
    pub send_time: i64,
}

/// Opaque handle. Protocol plus optional store attach (production always attaches).
pub struct KimSdkHandle {
    inner: Arc<KimSdk>,
}

impl KimSdkHandle {
    /// Always callable. Does not open SQLite.
    #[flutter_rust_bridge::frb(sync)]
    pub fn create() -> Self {
        Self {
            inner: KimSdk::protocol_only(),
        }
    }

    pub async fn attach_store(&self, db_path: String) -> Result<(), SdkErrorDto> {
        self.inner
            .attach_store(db_path)
            .await
            .map_err(SdkErrorDto::from)?;
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        self.inner.install_mobile_agent();
        Ok(())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn store_attached(&self) -> bool {
        self.inner.store_attached()
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_session_snapshot(
        &self,
        sink: StreamSink<SessionSnapshotDto>,
    ) -> Result<(), String> {
        let rx = self.inner.subscribe_session_snapshot();
        let _guard = rt().enter();
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let snap = rx.borrow().clone();
                if sink.add(SessionSnapshotDto::from(snap)).is_err() {
                    break;
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    /// Typed mpsc for Kickout/token/friend. Not the Dart inbox — fat
    /// [`session_events`] remains the inbox until watch carries Snapshot/Delta.
    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_session(&self, sink: StreamSink<SessionUpdateDto>) -> Result<(), String> {
        let mut rx = self.inner.subscribe_session();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(ev) = rx.recv().await {
                if sink.add(SessionUpdateDto::from(ev)).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_timeline(
        &self,
        dest: String,
        limit: i32,
        sink: StreamSink<TimelineUpdateDto>,
    ) -> Result<(), String> {
        let rx = self
            .inner
            .subscribe_timeline(kim_sdk::TimelineQuery { dest, limit });
        let _guard = rt().enter();
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let update = rx.borrow().clone();
                if sink.add(TimelineUpdateDto::from(update)).is_err() {
                    break;
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    pub async fn start_session(
        &self,
        url: String,
        token: String,
        user_agent: String,
        account: String,
    ) -> Result<(), String> {
        let account = if account.is_empty() {
            kim_client::account_from_token(&token).unwrap_or_default()
        } else {
            account
        };
        self.inner
            .start_session(StartSession {
                url,
                token,
                user_agent,
                account,
            })
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn enqueue_message(
        &self,
        dest: String,
        kind: i32,
        content: KimOutgoingContent,
        client_id: String,
        local_path: String,
        mime: String,
        width: i32,
        height: i32,
        byte_size: i64,
    ) -> Result<KimCommandReceipt, SdkErrorDto> {
        let payload = match content.kind {
            2 => OutgoingPayload::Image {
                media: MediaRef {
                    path: if local_path.is_empty() {
                        content.body
                    } else {
                        local_path
                    },
                    mime,
                    width,
                    height,
                    byte_size,
                },
            },
            4 => OutgoingPayload::Video {
                url: content.body,
                extra: content.extra,
            },
            _ => OutgoingPayload::Text { body: content.body },
        };
        let receipt = self
            .inner
            .enqueue_message(SendMessageCommand {
                dest,
                kind,
                payload,
                client_id: if client_id.is_empty() {
                    None
                } else {
                    Some(client_id)
                },
                batch_id: None,
            })
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(KimCommandReceipt {
            request_id: receipt.request_id,
            client_id: receipt.client_id,
            dest: receipt.dest,
            accepted_at: receipt.accepted_at,
            send_status: receipt.send_status.into(),
        })
    }

    pub async fn cancel_send(&self, client_id: String) -> Result<(), SdkErrorDto> {
        self.inner
            .cancel_send(client_id)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn retry_send(&self, client_id: String) -> Result<KimCommandReceipt, SdkErrorDto> {
        let receipt = self
            .inner
            .retry_send(client_id)
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(KimCommandReceipt {
            request_id: receipt.request_id,
            client_id: receipt.client_id,
            dest: receipt.dest,
            accepted_at: receipt.accepted_at,
            send_status: receipt.send_status.into(),
        })
    }

    pub async fn load_older(
        &self,
        dest: String,
        before_at: i64,
        before_key: String,
        before_id: i64,
        limit: i32,
    ) -> Result<MessagePageDto, SdkErrorDto> {
        let page = self
            .inner
            .load_older(kim_sdk::PageCursor {
                dest,
                before_at,
                before_key,
                before_id,
                limit,
            })
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(page.into())
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), SdkErrorDto> {
        self.inner
            .delete_thread(dest)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn mark_thread_read(
        &self,
        dest: String,
        kind: i32,
        message_id: i64,
    ) -> Result<(), SdkErrorDto> {
        self.inner
            .mark_read(ReadMarker {
                dest,
                kind,
                visible_message_id: message_id,
            })
            .await
            .map_err(SdkErrorDto::from)
    }

    fn supervisor(&self) -> Result<Arc<SessionSupervisor>, String> {
        self.inner.supervisor().map_err(|e| e.to_string())
    }

    pub fn stop(&self) {
        if let Ok(sup) = self.supervisor() {
            sup.stop();
        }
        let inner = self.inner.clone();
        let _ = rt().block_on(inner.stop_session());
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn link_state(&self) -> String {
        let Ok(supervisor) = self.supervisor() else {
            return "Offline".into();
        };
        match supervisor.state() {
            LinkState::Connecting => "Connecting".into(),
            LinkState::Online => "Online".into(),
            LinkState::Reconnecting { .. } => "Reconnecting".into(),
            LinkState::Offline => "Offline".into(),
        }
    }

    pub fn notify_radio_up(&self) -> Result<(), String> {
        rt().block_on(self.inner.notify_radio_up())
            .map_err(|e| e.to_string())
    }

    pub fn notify_foreground(&self) -> Result<(), String> {
        rt().block_on(self.inner.notify_foreground())
            .map_err(|e| e.to_string())
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
        let client = self.supervisor()?.client();
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
        let client = self.supervisor()?.client();
        let items = rt()
            .block_on(client.history(&dest, kind, before_id, limit))
            .map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(KimHistoryItem::from).collect())
    }

    pub fn inbox(&self, limit: i32) -> Result<Vec<KimInboxItem>, String> {
        let client = self.supervisor()?.client();
        let items = rt()
            .block_on(client.inbox_list(limit))
            .map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(KimInboxItem::from).collect())
    }

    pub fn mark_read(&self, dest: String, kind: i32, message_id: i64) -> Result<(), String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.mark_read(&dest, kind, message_id))
            .map_err(|e| e.to_string())
    }

    pub fn friend_request(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.friend_request(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_accept(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.friend_accept(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_reject(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.friend_reject(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn friend_remove(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.friend_remove(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_agent_run(&self, sink: StreamSink<AgentRunRequestDto>) -> Result<(), String> {
        let mut rx = self.inner.subscribe_agent_run();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(req) = rx.recv().await {
                if sink.add(AgentRunRequestDto::from(req)).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    pub async fn submit_agent_run(&self, result: AgentRunResultDto) -> Result<(), SdkErrorDto> {
        self.inner.submit_agent_run(result.into());
        Ok(())
    }

    pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfileDto>, SdkErrorDto> {
        let rows = self
            .inner
            .list_agent_profiles()
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(rows.into_iter().map(AgentProfileDto::from).collect())
    }

    pub async fn upsert_agent_profile(&self, row: AgentProfileDto) -> Result<(), SdkErrorDto> {
        self.inner
            .upsert_agent_profile(row.into())
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn delete_agent_profile(&self, profile_id: String) -> Result<(), SdkErrorDto> {
        self.inner
            .delete_agent_profile(profile_id)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn import_agent_profiles(
        &self,
        rows: Vec<AgentProfileDto>,
    ) -> Result<(), SdkErrorDto> {
        self.inner
            .import_agent_profiles(rows.into_iter().map(Into::into).collect())
            .await
            .map_err(SdkErrorDto::from)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_token_persist(&self, sink: StreamSink<TokenPersistDto>) -> Result<(), String> {
        let mut rx = self.inner.subscribe_token_persist();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(ev) = rx.recv().await {
                let dto = match ev {
                    kim_sdk::TokenPersistEvent::Write { token } => TokenPersistDto::Write { token },
                    kim_sdk::TokenPersistEvent::Clear => TokenPersistDto::Clear,
                };
                if sink.add(dto).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    pub async fn settings_get(&self) -> Result<SettingsDto, SdkErrorDto> {
        let row = self.inner.settings_get().await.map_err(SdkErrorDto::from)?;
        Ok(SettingsDto {
            ws_url: row.ws_url,
            http_origin: row.http_origin,
            env: row.env,
            locale: row.locale,
            account: row.account,
        })
    }

    pub async fn settings_patch(
        &self,
        ws_url: Option<String>,
        http_origin: Option<String>,
        env: Option<String>,
    ) -> Result<SettingsDto, SdkErrorDto> {
        let row = self
            .inner
            .settings_patch(ws_url, http_origin, env, None)
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(SettingsDto {
            ws_url: row.ws_url,
            http_origin: row.http_origin,
            env: row.env,
            locale: row.locale,
            account: row.account,
        })
    }

    pub async fn import_device_settings(
        &self,
        ws_url: String,
        http_origin: String,
        env: String,
        locale: String,
    ) -> Result<SettingsDto, SdkErrorDto> {
        let row = self
            .inner
            .import_device_settings(ws_url, http_origin, env, locale)
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(SettingsDto {
            ws_url: row.ws_url,
            http_origin: row.http_origin,
            env: row.env,
            locale: row.locale,
            account: row.account,
        })
    }

    pub fn refresh_contacts(&self) -> Result<Vec<PersonDto>, String> {
        let client = self.supervisor()?.client();
        let friends = rt()
            .block_on(client.friend_list())
            .map_err(|e| e.to_string())?;
        let incoming = rt()
            .block_on(client.friend_incoming())
            .map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        for p in friends {
            rows.push(kim_sdk::PersonRef {
                account: p.account,
                nickname: p.nickname,
                avatar: p.avatar,
                bio: p.bio,
                relation: "friend".into(),
                kind: p.kind,
            });
        }
        for p in incoming {
            rows.push(kim_sdk::PersonRef {
                account: p.account,
                nickname: p.nickname,
                avatar: p.avatar,
                bio: p.bio,
                relation: "incoming".into(),
                kind: p.kind,
            });
        }
        rt().block_on(self.inner.replace_contacts(rows.clone()))
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(PersonDto::from).collect())
    }

    pub fn friend_list(&self) -> Result<Vec<PersonDto>, String> {
        let client = self.supervisor()?.client();
        let users = rt()
            .block_on(client.friend_list())
            .map_err(|e| e.to_string())?;
        Ok(users
            .into_iter()
            .map(|p| PersonDto::from_profile(p, "friend"))
            .collect())
    }

    pub fn friend_incoming(&self) -> Result<Vec<PersonDto>, String> {
        let client = self.supervisor()?.client();
        let users = rt()
            .block_on(client.friend_incoming())
            .map_err(|e| e.to_string())?;
        Ok(users
            .into_iter()
            .map(|p| PersonDto::from_profile(p, "incoming"))
            .collect())
    }

    pub fn profile(&self, dest: String) -> Result<ProfileDto, String> {
        let client = self.supervisor()?.client();
        let p = rt()
            .block_on(client.profile(&dest))
            .map_err(|e| e.to_string())?;
        Ok(p.into())
    }

    pub fn update_profile(
        &self,
        nickname: String,
        avatar: String,
        bio: String,
    ) -> Result<ProfileDto, String> {
        let client = self.supervisor()?.client();
        let p = rt()
            .block_on(client.update_profile(&nickname, &avatar, &bio))
            .map_err(|e| e.to_string())?;
        Ok(p.into())
    }

    pub fn search_users(&self, query: String) -> Result<Vec<PersonDto>, String> {
        let client = self.supervisor()?.client();
        let users = rt()
            .block_on(client.search_users(&query))
            .map_err(|e| e.to_string())?;
        Ok(users
            .into_iter()
            .map(|p| PersonDto::from_profile(p, "none"))
            .collect())
    }

    pub fn room_enter(&self, dest: String, kind: i32) -> Result<Vec<RoomMemberDto>, String> {
        let client = self.supervisor()?.client();
        let rows = rt()
            .block_on(client.room_enter(&dest, kind))
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|e| RoomMemberDto {
                account: e.account,
                status: e.status,
                last_seen: e.last_seen,
            })
            .collect())
    }

    pub fn room_leave(&self, dest: String, kind: i32) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.room_leave(&dest, kind))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn send_typing(&self, dest: String, kind: i32, active: bool) -> Result<(), String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.send_typing(&dest, kind, active))
            .map_err(|e| e.to_string())
    }

    pub fn bot_create(
        &self,
        client_profile_id: String,
        nickname: String,
        avatar: String,
        bio: String,
        model: String,
        thinking_effort: String,
        context_tokens: i32,
        visibility: String,
    ) -> Result<PersonDto, String> {
        let client = self.supervisor()?.client();
        let config = kim_client::BotConfig {
            model,
            thinking_effort,
            context_tokens: if context_tokens > 0 {
                Some(context_tokens)
            } else {
                None
            },
            visibility,
        };
        let p = rt()
            .block_on(client.bot_create(&client_profile_id, &nickname, &avatar, &bio, &config))
            .map_err(|e| e.to_string())?;
        Ok(PersonDto::from_profile(p, "none"))
    }

    pub fn bot_delete(&self, dest: String) -> Result<String, String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.bot_delete(&dest))
            .map_err(|e| e.to_string())?;
        Ok("ok".into())
    }

    pub fn bot_update(
        &self,
        dest: String,
        nickname: String,
        avatar: String,
        bio: String,
        model: String,
        thinking_effort: String,
        context_tokens: i32,
        visibility: String,
    ) -> Result<PersonDto, String> {
        let client = self.supervisor()?.client();
        let config = kim_client::BotConfig {
            model,
            thinking_effort,
            context_tokens: if context_tokens > 0 {
                Some(context_tokens)
            } else {
                None
            },
            visibility,
        };
        let p = rt()
            .block_on(client.bot_update(&dest, &nickname, &avatar, &bio, &config))
            .map_err(|e| e.to_string())?;
        Ok(PersonDto::from_profile(p, "none"))
    }

    pub fn bot_reply(
        &self,
        dest: String,
        body: String,
        in_reply_to: i64,
        client_id: String,
    ) -> Result<KimTalkResult, String> {
        let client = self.supervisor()?.client();
        let result = rt()
            .block_on(client.bot_reply(&dest, &body, in_reply_to, &client_id))
            .map_err(|e| e.to_string())?;
        Ok(KimTalkResult::from(result))
    }

    pub fn bot_pending(&self, dest: String, limit: i32) -> Result<Vec<KimBotPendingItem>, String> {
        let client = self.supervisor()?.client();
        let items = rt()
            .block_on(client.bot_pending(&dest, limit))
            .map_err(|e| e.to_string())?;
        Ok(items.into_iter().map(KimBotPendingItem::from).collect())
    }

    pub fn bot_typing(&self, dest: String, kind: i32, active: bool) -> Result<(), String> {
        let client = self.supervisor()?.client();
        rt().block_on(client.bot_typing(&dest, kind, active))
            .map_err(|e| e.to_string())
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

impl From<BotPendingItem> for KimBotPendingItem {
    fn from(i: BotPendingItem) -> Self {
        Self {
            message_id: i.message_id,
            body: i.body,
            send_time: i.send_time,
        }
    }
}
