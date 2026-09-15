use std::sync::Arc;

use kim_client::{BotPendingItem, SessionSupervisor, TalkResult};
use kim_sdk::{KimSdk, MediaRef, OutgoingPayload, ReadMarker, SendMessageCommand, StartSession};

use super::rt;
use super::types::{
    AgentProfileDto, AgentRunRequestDto, AgentRunResultDto, CommandAckDto, ContactsSnapshotDto,
    DeviceOverlayDto, LocalMediaDto, MessageViewDto, MetricsDto, PersonDto, ProfileDto,
    ProviderAccountDto, RoomMemberDto, SdkErrorDto, SendStatusDto, SessionSnapshotDto,
    SessionUpdateDto, SettingsDto, TimelineUpdateDto, TokenPersistDto, UiCommandDto,
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

pub struct KimBotPendingItem {
    pub message_id: i64,
    pub body: String,
    pub send_time: i64,
}

/// Opaque handle. Protocol plus optional store attach (production always attaches).
pub struct KimUiHandle {
    inner: Arc<KimSdk>,
}

impl KimUiHandle {
    /// Always callable. Does not open SQLite.
    #[flutter_rust_bridge::frb(sync)]
    pub fn create() -> Self {
        let inner = KimSdk::protocol_only();
        inner.install_panic_hook();
        Self { inner }
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

    pub async fn command(&self, cmd: UiCommandDto) -> Result<CommandAckDto, SdkErrorDto> {
        match cmd {
            UiCommandDto::SendText { dest, text, kind } => {
                let receipt = self
                    .enqueue_message(
                        dest,
                        kind,
                        KimOutgoingContent {
                            kind: 1,
                            body: text,
                            extra: String::new(),
                        },
                        String::new(),
                        String::new(),
                        String::new(),
                        0,
                        0,
                        0,
                    )
                    .await?;
                Ok(ack_from_receipt(receipt))
            }
            UiCommandDto::SendMedia {
                dest,
                path,
                mime,
                width,
                height,
                byte_size,
                kind,
            } => {
                let receipt = self
                    .enqueue_message(
                        dest,
                        kind,
                        KimOutgoingContent {
                            kind: if kind == 0 { 2 } else { kind },
                            body: path.clone(),
                            extra: String::new(),
                        },
                        String::new(),
                        path,
                        mime,
                        width,
                        height,
                        byte_size,
                    )
                    .await?;
                Ok(ack_from_receipt(receipt))
            }
            UiCommandDto::RetrySend { client_id } => {
                let receipt = self.retry_send(client_id).await?;
                Ok(ack_from_receipt(receipt))
            }
            UiCommandDto::CancelSend { client_id } => {
                self.cancel_send(client_id).await?;
                Ok(empty_ack())
            }
            UiCommandDto::MarkThreadRead {
                dest,
                kind,
                visible_message_id,
            } => {
                self.mark_thread_read(dest, kind, visible_message_id)
                    .await?;
                Ok(empty_ack())
            }
            UiCommandDto::DeleteThread { dest } => {
                self.delete_thread(dest).await?;
                Ok(empty_ack())
            }
            UiCommandDto::FriendRequest { dest } => {
                self.friend_request(dest).await?;
                Ok(empty_ack())
            }
            UiCommandDto::FriendAccept { dest } => {
                self.friend_accept(dest).await?;
                Ok(empty_ack())
            }
            UiCommandDto::FriendReject { dest } => {
                self.friend_reject(dest).await?;
                Ok(empty_ack())
            }
            UiCommandDto::FriendRemove { dest } => {
                self.friend_remove(dest).await?;
                Ok(empty_ack())
            }
            UiCommandDto::AgentEnqueueTurn {
                dest,
                text,
                in_reply_to,
            } => {
                self.inner
                    .enqueue_agent_turn(dest, text, in_reply_to)
                    .await
                    .map_err(SdkErrorDto::from)?;
                Ok(empty_ack())
            }
            UiCommandDto::AgentRespondPermission { .. } => {
                Err(SdkErrorDto::from(kim_sdk::SdkError::InvalidArgument {
                    message: "respond permission via rust_agent session".into(),
                }))
            }
            UiCommandDto::AgentAbortTurn { .. } => {
                Err(SdkErrorDto::from(kim_sdk::SdkError::InvalidArgument {
                    message: "abort turn via rust_agent session".into(),
                }))
            }
            UiCommandDto::AgentRunResult {
                dest,
                profile_id,
                epoch,
                output,
                error,
            } => {
                self.submit_agent_run(AgentRunResultDto {
                    dest,
                    profile_id,
                    epoch,
                    output,
                    error,
                })
                .await?;
                Ok(empty_ack())
            }
            UiCommandDto::SettingsPatch {
                ws_url,
                http_origin,
                env,
            } => {
                self.settings_patch(ws_url, http_origin, env).await?;
                Ok(empty_ack())
            }
        }
    }

    pub async fn search_messages(
        &self,
        query: String,
        dest: Option<String>,
    ) -> Result<Vec<MessageViewDto>, SdkErrorDto> {
        let rows = self
            .inner
            .search_messages(query, dest)
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(rows.into_iter().map(MessageViewDto::from).collect())
    }

    pub async fn media_fetch(&self, url: String) -> Result<LocalMediaDto, SdkErrorDto> {
        let path = self
            .inner
            .fetch_media(url)
            .await
            .map_err(SdkErrorDto::from)?;
        let size = tokio::fs::metadata(&path)
            .await
            .map(|m| i64::try_from(m.len()).unwrap_or(0))
            .unwrap_or(0);
        Ok(LocalMediaDto {
            local_path: path,
            byte_size: size,
            width: 0,
            height: 0,
        })
    }

    pub async fn media_upload(
        &self,
        path: String,
        mime: String,
        width: i32,
        height: i32,
        byte_size: i64,
    ) -> Result<LocalMediaDto, SdkErrorDto> {
        let url = self
            .inner
            .upload_media(MediaRef {
                path: path.clone(),
                mime,
                width,
                height,
                byte_size,
            })
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(LocalMediaDto {
            local_path: url,
            byte_size,
            width,
            height,
        })
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn store_attached(&self) -> bool {
        self.inner.store_attached()
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn metrics_snapshot(&self) -> MetricsDto {
        let (enqueue, persist, epoch_drop, wipe) = self.inner.metrics();
        MetricsDto {
            enqueue_total: enqueue,
            persist_talk_total: persist,
            epoch_drop_total: epoch_drop,
            store_wipe_total: wipe,
        }
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

    /// Discrete Kickout/token/friend/agent events. Inbox/link live on snapshot.
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
        let _guard = rt().enter();
        let rx = self
            .inner
            .subscribe_timeline(kim_sdk::TimelineQuery { dest, limit });
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

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_contacts(&self, sink: StreamSink<ContactsSnapshotDto>) -> Result<(), String> {
        let _guard = rt().enter();
        let rx = self.inner.subscribe_contacts();
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let snapshot = rx.borrow().clone();
                if sink.add(ContactsSnapshotDto::from(snapshot)).is_err() {
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

    pub async fn load_older(&self, dest: String) -> Result<(), SdkErrorDto> {
        self.inner
            .load_older(dest)
            .await
            .map_err(SdkErrorDto::from)
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

    pub fn notify_radio_up(&self) -> Result<(), String> {
        rt().block_on(self.inner.notify_radio_up())
            .map_err(|e| e.to_string())
    }

    pub fn notify_foreground(&self) -> Result<(), String> {
        rt().block_on(self.inner.notify_foreground())
            .map_err(|e| e.to_string())
    }

    pub async fn mark_read(
        &self,
        dest: String,
        kind: i32,
        message_id: i64,
    ) -> Result<(), SdkErrorDto> {
        self.mark_thread_read(dest, kind, message_id).await
    }

    pub async fn friend_request(&self, dest: String) -> Result<(), SdkErrorDto> {
        let client = self.supervisor().map_err(SdkErrorDto::from_str)?.client();
        client
            .friend_request(&dest)
            .await
            .map_err(|e| SdkErrorDto::from(kim_sdk::map_client(e, &dest)))?;
        self.inner
            .mark_outgoing_contact(dest)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn friend_accept(&self, dest: String) -> Result<(), SdkErrorDto> {
        let client = self.supervisor().map_err(SdkErrorDto::from_str)?.client();
        client
            .friend_accept(&dest)
            .await
            .map_err(|e| SdkErrorDto::from(kim_sdk::map_client(e, &dest)))
    }

    pub async fn friend_reject(&self, dest: String) -> Result<(), SdkErrorDto> {
        let client = self.supervisor().map_err(SdkErrorDto::from_str)?.client();
        client
            .friend_reject(&dest)
            .await
            .map_err(|e| SdkErrorDto::from(kim_sdk::map_client(e, &dest)))
    }

    pub async fn friend_remove(&self, dest: String) -> Result<(), SdkErrorDto> {
        let client = self.supervisor().map_err(SdkErrorDto::from_str)?.client();
        client
            .friend_remove(&dest)
            .await
            .map_err(|e| SdkErrorDto::from(kim_sdk::map_client(e, &dest)))?;
        self.inner
            .remove_contact(dest)
            .await
            .map_err(SdkErrorDto::from)
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

    pub async fn list_provider_accounts(&self) -> Result<Vec<ProviderAccountDto>, SdkErrorDto> {
        let rows = self
            .inner
            .list_provider_accounts()
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(rows.into_iter().map(ProviderAccountDto::from).collect())
    }

    pub async fn upsert_provider_account(
        &self,
        row: ProviderAccountDto,
    ) -> Result<(), SdkErrorDto> {
        self.inner
            .upsert_provider_account(row.into())
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn delete_provider_account(&self, id: String) -> Result<(), SdkErrorDto> {
        self.inner
            .delete_provider_account(id)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn get_device_overlay(
        &self,
        profile_id: String,
    ) -> Result<Option<DeviceOverlayDto>, SdkErrorDto> {
        let row = self
            .inner
            .get_device_overlay(profile_id)
            .await
            .map_err(SdkErrorDto::from)?;
        Ok(row.map(DeviceOverlayDto::from))
    }

    pub async fn upsert_device_overlay(&self, row: DeviceOverlayDto) -> Result<(), SdkErrorDto> {
        self.inner
            .upsert_device_overlay(row.into())
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn agent_flags(&self) -> Result<String, SdkErrorDto> {
        self.inner.agent_flags().await.map_err(SdkErrorDto::from)
    }

    pub async fn set_agent_flags(&self, flags_json: String) -> Result<(), SdkErrorDto> {
        self.inner
            .set_agent_flags(flags_json)
            .await
            .map_err(SdkErrorDto::from)
    }

    pub async fn sync_agent_specs(&self) -> Result<(), SdkErrorDto> {
        self.inner
            .sync_agent_specs()
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

    pub async fn refresh_contacts(&self) -> Result<(), SdkErrorDto> {
        self.inner
            .refresh_contacts()
            .await
            .map_err(SdkErrorDto::from)
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
        rt().block_on(self.inner.remove_contact(dest))
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

fn empty_ack() -> CommandAckDto {
    CommandAckDto {
        request_id: String::new(),
        client_id: String::new(),
        dest: String::new(),
        accepted_at: 0,
        send_status: SendStatusDto::Sent,
    }
}

fn ack_from_receipt(r: KimCommandReceipt) -> CommandAckDto {
    CommandAckDto {
        request_id: r.request_id,
        client_id: r.client_id,
        dest: r.dest,
        accepted_at: r.accepted_at,
        send_status: r.send_status,
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

impl From<BotPendingItem> for KimBotPendingItem {
    fn from(i: BotPendingItem) -> Self {
        Self {
            message_id: i.message_id,
            body: i.body,
            send_time: i.send_time,
        }
    }
}
