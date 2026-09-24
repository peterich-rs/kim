use std::sync::Arc;

use kim_client::{BotPendingItem, SessionSupervisor, TalkResult};
use kim_sdk::{
    ConversationKey, ConversationVisibility, KimSdk, MediaRef, OutgoingPayload, ReadMarker,
    SendMessageCommand, StartSession,
};

use super::failure::ApiFailure;
use super::rt;
use super::types::{
    AgentFlags, AgentProfile, AgentRunRequest, AgentRunResult, CommandAck, ContactsSnapshot,
    DeviceOverlay, LocalMedia, MessageView, Metrics, Person, Profile, ProviderAccount, RoomMember,
    SendStatus, SessionSnapshot, SessionUpdate, Settings, TimelineUpdate, TokenPersist, UiCommand,
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
    pub send_status: SendStatus,
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
#[derive(Clone)]
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

    pub async fn attach_store(&self, db_path: String) -> Result<(), ApiFailure> {
        kim_log::init_beside(&db_path, "kim.log");
        self.inner
            .attach_store(db_path)
            .await
            .map_err(ApiFailure::from)?;
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        kim_desktop_runtime::install(self.inner.as_ref().clone());
        Ok(())
    }

    pub async fn command(&self, cmd: UiCommand) -> Result<CommandAck, ApiFailure> {
        match cmd {
            UiCommand::SendText { dest, text, kind } => {
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
            UiCommand::SendMedia {
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
            UiCommand::RetrySend { client_id } => {
                let receipt = self.retry_send(client_id).await?;
                Ok(ack_from_receipt(receipt))
            }
            UiCommand::CancelSend { client_id } => {
                self.cancel_send(client_id).await?;
                Ok(empty_ack())
            }
            UiCommand::MarkThreadRead {
                dest,
                kind,
                visible_message_id,
            } => {
                self.mark_thread_read(dest, kind, visible_message_id)
                    .await?;
                Ok(empty_ack())
            }
            UiCommand::DeleteThread { dest } => {
                self.delete_thread(dest).await?;
                Ok(empty_ack())
            }
            UiCommand::FriendRequest { dest } => {
                self.friend_request(dest).await?;
                Ok(empty_ack())
            }
            UiCommand::FriendAccept { dest } => {
                self.friend_accept(dest).await?;
                Ok(empty_ack())
            }
            UiCommand::FriendReject { dest } => {
                self.friend_reject(dest).await?;
                Ok(empty_ack())
            }
            UiCommand::FriendRemove { dest } => {
                self.friend_remove(dest).await?;
                Ok(empty_ack())
            }
            UiCommand::AgentEnqueueTurn {
                dest,
                text,
                in_reply_to,
            } => {
                self.inner
                    .enqueue_agent_turn(dest, text, in_reply_to)
                    .await
                    .map_err(ApiFailure::from)?;
                Ok(empty_ack())
            }
            UiCommand::AgentRespondPermission { .. } => {
                Err(ApiFailure::from(kim_sdk::SdkError::InvalidArgument {
                    message: "respond permission via rust_agent session".into(),
                }))
            }
            UiCommand::AgentAbortTurn { .. } => {
                Err(ApiFailure::from(kim_sdk::SdkError::InvalidArgument {
                    message: "abort turn via rust_agent session".into(),
                }))
            }
            UiCommand::AgentRunResult {
                dest,
                profile_id,
                epoch,
                output,
                error,
            } => {
                let failed = error.is_some();
                let replied = !failed && !output.trim().is_empty();
                self.submit_agent_run(AgentRunResult {
                    dest,
                    profile_id,
                    epoch,
                    output,
                    error,
                    stop_reason: if failed {
                        "failed".into()
                    } else if replied {
                        "completed".into()
                    } else {
                        "empty".into()
                    },
                    replied,
                    visible: replied,
                    recently_active: false,
                })
                .await?;
                Ok(empty_ack())
            }
            UiCommand::SettingsPatch {
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
    ) -> Result<Vec<MessageView>, ApiFailure> {
        let rows = self
            .inner
            .search_messages(query, dest)
            .await
            .map_err(ApiFailure::from)?;
        Ok(rows.into_iter().map(MessageView::from).collect())
    }

    pub async fn media_fetch(&self, url: String) -> Result<LocalMedia, ApiFailure> {
        let path = self
            .inner
            .fetch_media(url)
            .await
            .map_err(ApiFailure::from)?;
        let size = tokio::fs::metadata(&path)
            .await
            .map(|m| i64::try_from(m.len()).unwrap_or(0))
            .unwrap_or(0);
        Ok(LocalMedia {
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
    ) -> Result<LocalMedia, ApiFailure> {
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
            .map_err(ApiFailure::from)?;
        Ok(LocalMedia {
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
    pub fn metrics_snapshot(&self) -> Metrics {
        let (enqueue, persist, epoch_drop, wipe) = self.inner.metrics();
        Metrics {
            enqueue_total: enqueue,
            persist_talk_total: persist,
            epoch_drop_total: epoch_drop,
            store_wipe_total: wipe,
        }
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_session_snapshot(
        &self,
        sink: StreamSink<SessionSnapshot>,
    ) -> Result<(), ApiFailure> {
        let rx = self.inner.subscribe_session_snapshot();
        let _guard = rt().enter();
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let snap = rx.borrow().clone();
                if sink.add(SessionSnapshot::from(snap)).is_err() {
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
    pub fn watch_session(&self, sink: StreamSink<SessionUpdate>) -> Result<(), ApiFailure> {
        let mut rx = self.inner.subscribe_session();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(ev) = rx.recv().await {
                if sink.add(SessionUpdate::from(ev)).is_err() {
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
        sink: StreamSink<TimelineUpdate>,
    ) -> Result<(), ApiFailure> {
        let _guard = rt().enter();
        let rx = self
            .inner
            .subscribe_timeline(kim_sdk::TimelineQuery { dest, limit });
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let update = rx.borrow().clone();
                if sink.add(TimelineUpdate::from(update)).is_err() {
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
    pub fn watch_contacts(&self, sink: StreamSink<ContactsSnapshot>) -> Result<(), ApiFailure> {
        let _guard = rt().enter();
        let rx = self.inner.subscribe_contacts();
        rt().spawn(async move {
            let mut rx = rx;
            loop {
                let snapshot = rx.borrow().clone();
                if sink.add(ContactsSnapshot::from(snapshot)).is_err() {
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
    ) -> Result<(), ApiFailure> {
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
            .map_err(ApiFailure::from)
    }

    #[allow(clippy::too_many_arguments)]
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
    ) -> Result<KimCommandReceipt, ApiFailure> {
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
            .map_err(ApiFailure::from)?;
        Ok(KimCommandReceipt {
            request_id: receipt.request_id,
            client_id: receipt.client_id,
            dest: receipt.dest,
            accepted_at: receipt.accepted_at,
            send_status: receipt.send_status.into(),
        })
    }

    pub async fn cancel_send(&self, client_id: String) -> Result<(), ApiFailure> {
        self.inner
            .cancel_send(client_id)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn retry_send(&self, client_id: String) -> Result<KimCommandReceipt, ApiFailure> {
        let receipt = self
            .inner
            .retry_send(client_id)
            .await
            .map_err(ApiFailure::from)?;
        Ok(KimCommandReceipt {
            request_id: receipt.request_id,
            client_id: receipt.client_id,
            dest: receipt.dest,
            accepted_at: receipt.accepted_at,
            send_status: receipt.send_status.into(),
        })
    }

    pub async fn load_older(&self, dest: String) -> Result<(), ApiFailure> {
        self.inner.load_older(dest).await.map_err(ApiFailure::from)
    }

    pub async fn delete_thread(&self, dest: String) -> Result<(), ApiFailure> {
        self.inner
            .delete_thread(dest)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn mark_thread_read(
        &self,
        dest: String,
        kind: i32,
        message_id: i64,
    ) -> Result<(), ApiFailure> {
        if message_id <= 0 {
            return self
                .inner
                .mark_thread_read(dest, kind)
                .await
                .map_err(ApiFailure::from);
        }
        self.inner
            .mark_read(ReadMarker {
                dest,
                kind,
                visible_message_id: message_id,
            })
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn mark_conversation_read(&self, dest: String, kind: i32) -> Result<(), ApiFailure> {
        self.inner
            .mark_thread_read(dest, kind)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn set_conversation_visibility(
        &self,
        generation: u64,
        foreground: bool,
        dest: String,
        kind: i32,
    ) -> Result<(), ApiFailure> {
        self.inner
            .set_conversation_visibility(ConversationVisibility {
                generation,
                foreground,
                conversation: if dest.is_empty() {
                    None
                } else {
                    Some(ConversationKey { dest, kind })
                },
            })
            .await
            .map_err(ApiFailure::from)
    }

    fn supervisor(&self) -> Result<Arc<SessionSupervisor>, ApiFailure> {
        self.inner.supervisor().map_err(ApiFailure::from)
    }

    pub async fn stop(&self) {
        if let Ok(sup) = self.supervisor() {
            sup.stop();
        }
        let inner = self.inner.clone();
        let _ = inner.stop_session().await;
    }

    pub async fn notify_radio_up(&self) -> Result<(), ApiFailure> {
        self.inner.notify_radio_up().await.map_err(ApiFailure::from)
    }

    pub async fn notify_foreground(&self) -> Result<(), ApiFailure> {
        self.inner
            .notify_foreground()
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn mark_read(
        &self,
        dest: String,
        kind: i32,
        message_id: i64,
    ) -> Result<(), ApiFailure> {
        self.mark_thread_read(dest, kind, message_id).await
    }

    pub async fn friend_request(&self, dest: String) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .friend_request(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        self.inner
            .mark_outgoing_contact(dest)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn friend_accept(&self, dest: String) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .friend_accept(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))
    }

    pub async fn friend_reject(&self, dest: String) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .friend_reject(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))
    }

    pub async fn friend_remove(&self, dest: String) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .friend_remove(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        self.inner
            .remove_contact(dest)
            .await
            .map_err(ApiFailure::from)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_agent_run(&self, sink: StreamSink<AgentRunRequest>) -> Result<(), ApiFailure> {
        let mut rx = self.inner.subscribe_agent_run();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(req) = rx.recv().await {
                let dest = req.dest.clone();
                let profile_id = req.profile_id.clone();
                if sink.add(AgentRunRequest::from(req)).is_err() {
                    // Dart port closed ("Fail to post message to Dart"). Drop
                    // this watch; AgentRunLoop must re-subscribe.
                    tracing::warn!(
                        %dest,
                        %profile_id,
                        "agent_run StreamSink closed; dart watch ended"
                    );
                    break;
                }
            }
        });
        Ok(())
    }

    pub async fn submit_agent_run(&self, result: AgentRunResult) -> Result<(), ApiFailure> {
        self.inner.submit_agent_run(result.into());
        Ok(())
    }

    pub async fn list_agent_profiles(&self) -> Result<Vec<AgentProfile>, ApiFailure> {
        let rows = self
            .inner
            .list_agent_profiles()
            .await
            .map_err(ApiFailure::from)?;
        Ok(rows.into_iter().map(AgentProfile::from).collect())
    }

    pub async fn upsert_agent_profile(&self, row: AgentProfile) -> Result<(), ApiFailure> {
        self.inner
            .upsert_agent_profile(row.into())
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn delete_agent_profile(&self, profile_id: String) -> Result<(), ApiFailure> {
        self.inner
            .delete_agent_profile(profile_id)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn import_agent_profiles(&self, rows: Vec<AgentProfile>) -> Result<(), ApiFailure> {
        self.inner
            .import_agent_profiles(rows.into_iter().map(Into::into).collect())
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn list_provider_accounts(&self) -> Result<Vec<ProviderAccount>, ApiFailure> {
        let rows = self
            .inner
            .list_provider_accounts()
            .await
            .map_err(ApiFailure::from)?;
        Ok(rows.into_iter().map(ProviderAccount::from).collect())
    }

    pub async fn upsert_provider_account(&self, row: ProviderAccount) -> Result<(), ApiFailure> {
        self.inner
            .upsert_provider_account(row.into())
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn delete_provider_account(&self, id: String) -> Result<(), ApiFailure> {
        self.inner
            .delete_provider_account(id)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn get_device_overlay(
        &self,
        profile_id: String,
    ) -> Result<Option<DeviceOverlay>, ApiFailure> {
        let row = self
            .inner
            .get_device_overlay(profile_id)
            .await
            .map_err(ApiFailure::from)?;
        Ok(row.map(DeviceOverlay::from))
    }

    pub async fn upsert_device_overlay(&self, row: DeviceOverlay) -> Result<(), ApiFailure> {
        self.inner
            .upsert_device_overlay(row.into())
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn agent_flags(&self) -> Result<AgentFlags, ApiFailure> {
        let raw = self.inner.agent_flags().await.map_err(ApiFailure::from)?;
        if raw.trim().is_empty() {
            return Ok(AgentFlags {
                multi_profile: false,
                server_identity: false,
            });
        }
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|err| ApiFailure::InvalidArgument {
                message: err.to_string(),
            })?;
        Ok(AgentFlags {
            multi_profile: value
                .get("multi_profile")
                .and_then(|item| item.as_bool())
                .unwrap_or(false),
            server_identity: value
                .get("server_identity")
                .and_then(|item| item.as_bool())
                .unwrap_or(false),
        })
    }

    pub async fn set_agent_flags(&self, flags: AgentFlags) -> Result<(), ApiFailure> {
        let raw = serde_json::json!({
            "multi_profile": flags.multi_profile,
            "server_identity": flags.server_identity,
        })
        .to_string();
        self.inner
            .set_agent_flags(raw)
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn sync_agent_specs(&self) -> Result<(), ApiFailure> {
        self.inner
            .sync_agent_specs()
            .await
            .map_err(ApiFailure::from)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn watch_token_persist(&self, sink: StreamSink<TokenPersist>) -> Result<(), ApiFailure> {
        let mut rx = self.inner.subscribe_token_persist();
        let _guard = rt().enter();
        rt().spawn(async move {
            while let Some(ev) = rx.recv().await {
                let dto = match ev {
                    kim_sdk::TokenPersistEvent::Write { token } => TokenPersist::Write { token },
                    kim_sdk::TokenPersistEvent::Clear => TokenPersist::Clear,
                };
                if sink.add(dto).is_err() {
                    break;
                }
            }
        });
        Ok(())
    }

    pub async fn settings_get(&self) -> Result<Settings, ApiFailure> {
        let row = self.inner.settings_get().await.map_err(ApiFailure::from)?;
        Ok(Settings {
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
    ) -> Result<Settings, ApiFailure> {
        let row = self
            .inner
            .settings_patch(ws_url, http_origin, env, None)
            .await
            .map_err(ApiFailure::from)?;
        Ok(Settings {
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
    ) -> Result<Settings, ApiFailure> {
        let row = self
            .inner
            .import_device_settings(ws_url, http_origin, env, locale)
            .await
            .map_err(ApiFailure::from)?;
        Ok(Settings {
            ws_url: row.ws_url,
            http_origin: row.http_origin,
            env: row.env,
            locale: row.locale,
            account: row.account,
        })
    }

    pub async fn refresh_contacts(&self) -> Result<(), ApiFailure> {
        self.inner
            .refresh_contacts()
            .await
            .map_err(ApiFailure::from)
    }

    pub async fn friend_list(&self) -> Result<Vec<Person>, ApiFailure> {
        let client = self.supervisor()?.client();
        let users = client.friend_list().await.map_err(ApiFailure::from)?;
        Ok(users
            .into_iter()
            .map(|p| Person::from_profile(p, "friend"))
            .collect())
    }

    pub async fn friend_incoming(&self) -> Result<Vec<Person>, ApiFailure> {
        let client = self.supervisor()?.client();
        let users = client.friend_incoming().await.map_err(ApiFailure::from)?;
        Ok(users
            .into_iter()
            .map(|p| Person::from_profile(p, "incoming"))
            .collect())
    }

    pub async fn profile(&self, dest: String) -> Result<Profile, ApiFailure> {
        let client = self.supervisor()?.client();
        let p = client
            .profile(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok(p.into())
    }

    pub async fn update_profile(
        &self,
        nickname: String,
        avatar: String,
        bio: String,
    ) -> Result<Profile, ApiFailure> {
        let client = self.supervisor()?.client();
        let p = client
            .update_profile(&nickname, &avatar, &bio)
            .await
            .map_err(ApiFailure::from)?;
        Ok(p.into())
    }

    pub async fn search_users(&self, query: String) -> Result<Vec<Person>, ApiFailure> {
        let client = self.supervisor()?.client();
        let users = client
            .search_users(&query)
            .await
            .map_err(ApiFailure::from)?;
        Ok(users
            .into_iter()
            .map(|p| Person::from_profile(p, "none"))
            .collect())
    }

    pub async fn room_enter(&self, dest: String, kind: i32) -> Result<Vec<RoomMember>, ApiFailure> {
        let client = self.supervisor()?.client();
        let rows = client
            .room_enter(&dest, kind)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok(rows
            .into_iter()
            .map(|e| RoomMember {
                account: e.account,
                status: e.status,
                last_seen: e.last_seen,
            })
            .collect())
    }

    pub async fn room_leave(&self, dest: String, kind: i32) -> Result<String, ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .room_leave(&dest, kind)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok("ok".into())
    }

    pub async fn send_typing(
        &self,
        dest: String,
        kind: i32,
        active: bool,
    ) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .send_typing(&dest, kind, active)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn bot_create(
        &self,
        client_profile_id: String,
        nickname: String,
        avatar: String,
        bio: String,
        model: String,
        thinking_effort: String,
        context_tokens: i32,
        visibility: String,
    ) -> Result<Person, ApiFailure> {
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
        let p = client
            .bot_create(&client_profile_id, &nickname, &avatar, &bio, &config)
            .await
            .map_err(ApiFailure::from)?;
        Ok(Person::from_profile(p, "none"))
    }

    pub async fn bot_delete(&self, dest: String) -> Result<String, ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .bot_delete(&dest)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        self.inner
            .remove_contact(dest)
            .await
            .map_err(ApiFailure::from)?;
        Ok("ok".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn bot_update(
        &self,
        dest: String,
        nickname: String,
        avatar: String,
        bio: String,
        model: String,
        thinking_effort: String,
        context_tokens: i32,
        visibility: String,
    ) -> Result<Person, ApiFailure> {
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
        let p = client
            .bot_update(&dest, &nickname, &avatar, &bio, &config)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok(Person::from_profile(p, "none"))
    }

    pub async fn bot_reply(
        &self,
        dest: String,
        body: String,
        in_reply_to: i64,
        client_id: String,
    ) -> Result<KimTalkResult, ApiFailure> {
        let client = self.supervisor()?.client();
        let result = client
            .bot_reply(&dest, &body, in_reply_to, &client_id)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok(KimTalkResult::from(result))
    }

    pub async fn bot_pending(
        &self,
        dest: String,
        limit: i32,
    ) -> Result<Vec<KimBotPendingItem>, ApiFailure> {
        let client = self.supervisor()?.client();
        let items = client
            .bot_pending(&dest, limit)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))?;
        Ok(items.into_iter().map(KimBotPendingItem::from).collect())
    }

    pub async fn bot_typing(
        &self,
        dest: String,
        kind: i32,
        active: bool,
    ) -> Result<(), ApiFailure> {
        let client = self.supervisor()?.client();
        client
            .bot_typing(&dest, kind, active)
            .await
            .map_err(|e| ApiFailure::from_client(e, &dest))
    }
}

fn empty_ack() -> CommandAck {
    CommandAck {
        request_id: String::new(),
        client_id: String::new(),
        dest: String::new(),
        accepted_at: 0,
        send_status: SendStatus::Sent,
    }
}

fn ack_from_receipt(r: KimCommandReceipt) -> CommandAck {
    CommandAck {
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
