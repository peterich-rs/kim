//! Domain handles over [`KimUiHandle`]. Dart talks to these objects, not a flat
//! procedure list. Timeline bytes stay in Rust; the sink only carries view rows.

use super::client::{KimCommandReceipt, KimOutgoingContent, KimUiHandle};
use super::failure::ApiFailure;
use super::types::{
    AgentProfile, ContactsSnapshot, LocalMedia, Person, Profile, ProviderAccount,
    RoomMember, TimelineUpdate,
};
use crate::frb_generated::StreamSink;

pub struct InboxHandle {
    app: KimUiHandle,
}

impl InboxHandle {
    /// Session snapshot still carries the thread list. The handle is the object
    /// boundary; splitting the projection type is a later cut.
    pub fn app(&self) -> KimUiHandle {
        self.app.clone()
    }
}

pub struct ConversationHandle {
    app: KimUiHandle,
    dest: String,
}

impl ConversationHandle {
    #[must_use]
    pub fn dest(&self) -> String {
        self.dest.clone()
    }

    pub fn watch_timeline(
        &self,
        limit: i32,
        sink: StreamSink<TimelineUpdate>,
    ) -> Result<(), ApiFailure> {
        self.app.watch_timeline(self.dest.clone(), limit, sink)
    }

    pub async fn send_text(
        &self,
        text: String,
        client_id: String,
    ) -> Result<KimCommandReceipt, ApiFailure> {
        self.app
            .enqueue_message(
                self.dest.clone(),
                0,
                KimOutgoingContent {
                    kind: 1,
                    body: text,
                    extra: String::new(),
                },
                client_id,
                String::new(),
                String::new(),
                0,
                0,
                0,
            )
            .await
    }

    pub async fn enter(&self) -> Result<Vec<RoomMember>, ApiFailure> {
        self.app.room_enter(self.dest.clone(), 0).await
    }

    pub async fn leave(&self) -> Result<String, ApiFailure> {
        self.app.room_leave(self.dest.clone(), 0).await
    }

    pub async fn set_typing(&self, active: bool) -> Result<(), ApiFailure> {
        self.app.send_typing(self.dest.clone(), 0, active).await
    }

    pub async fn mark_read(&self) -> Result<(), ApiFailure> {
        self.app.mark_conversation_read(self.dest.clone(), 0).await
    }

    pub async fn load_older(&self) -> Result<(), ApiFailure> {
        self.app.load_older(self.dest.clone()).await
    }
}

pub struct ContactsHandle {
    app: KimUiHandle,
}

impl ContactsHandle {
    pub fn watch(&self, sink: StreamSink<ContactsSnapshot>) -> Result<(), ApiFailure> {
        self.app.watch_contacts(sink)
    }

    pub async fn friends(&self) -> Result<Vec<Person>, ApiFailure> {
        self.app.friend_list().await
    }

    pub async fn search(&self, query: String) -> Result<Vec<Person>, ApiFailure> {
        self.app.search_users(query).await
    }

    pub async fn profile(&self, dest: String) -> Result<Profile, ApiFailure> {
        self.app.profile(dest).await
    }
}

pub struct MediaHandle {
    app: KimUiHandle,
}

impl MediaHandle {
    pub async fn fetch(&self, url: String) -> Result<LocalMedia, ApiFailure> {
        self.app.media_fetch(url).await
    }

    pub async fn upload(
        &self,
        path: String,
        mime: String,
        width: i32,
        height: i32,
        byte_size: i64,
    ) -> Result<LocalMedia, ApiFailure> {
        self.app
            .media_upload(path, mime, width, height, byte_size)
            .await
    }
}

pub struct AgentCatalogHandle {
    app: KimUiHandle,
}

impl AgentCatalogHandle {
    pub async fn list_profiles(&self) -> Result<Vec<AgentProfile>, ApiFailure> {
        self.app.list_agent_profiles().await
    }

    pub async fn upsert_profile(&self, row: AgentProfile) -> Result<(), ApiFailure> {
        self.app.upsert_agent_profile(row).await
    }

    pub async fn delete_profile(&self, id: String) -> Result<(), ApiFailure> {
        self.app.delete_agent_profile(id).await
    }

    pub async fn list_accounts(&self) -> Result<Vec<ProviderAccount>, ApiFailure> {
        self.app.list_provider_accounts().await
    }
}

impl KimUiHandle {
    #[must_use]
    pub fn inbox(&self) -> InboxHandle {
        InboxHandle { app: self.clone() }
    }

    #[must_use]
    pub fn conversation(&self, dest: String) -> ConversationHandle {
        ConversationHandle {
            app: self.clone(),
            dest,
        }
    }

    #[must_use]
    pub fn contacts(&self) -> ContactsHandle {
        ContactsHandle { app: self.clone() }
    }

    #[must_use]
    pub fn media(&self) -> MediaHandle {
        MediaHandle { app: self.clone() }
    }

    #[must_use]
    pub fn agent_catalog(&self) -> AgentCatalogHandle {
        AgentCatalogHandle { app: self.clone() }
    }

    /// Seeds the desktop secret vault once. Not part of a turn payload.
    pub fn cache_agent_secret(&self, key_ref: String, secret: String) {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        kim_desktop_runtime::cache_secret(&key_ref, &secret);
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let _ = (key_ref, secret);
    }

    pub fn respond_agent_permission(&self, call_id: String, allow: bool) {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        kim_desktop_runtime::respond_permission(&call_id, allow);
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        let _ = (call_id, allow);
    }

    /// Desktop permission cards. Phone returns an idle stream.
    pub fn watch_agent_permission(
        &self,
        sink: StreamSink<AgentPermissionEvent>,
    ) -> Result<(), ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let Some(runtime) = kim_desktop_runtime::installed() else {
                return Err(ApiFailure::unavailable("host agent runtime"));
            };
            let mut rx = runtime.permissions.subscribe();
            let _guard = super::rt().enter();
            super::rt().spawn(async move {
                while let Some(event) = rx.recv().await {
                    if sink
                        .add(AgentPermissionEvent {
                            dest: event.dest,
                            call_id: event.call_id,
                            name: event.name,
                            preview: event.preview,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = sink;
            Ok(())
        }
    }

    /// Desktop pet presence (`running` / `done` / `failed`). Phone is idle.
    pub fn watch_agent_ui(&self, sink: StreamSink<AgentUiStatus>) -> Result<(), ApiFailure> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            let Some(runtime) = kim_desktop_runtime::installed() else {
                return Err(ApiFailure::unavailable("host agent runtime"));
            };
            let mut rx = runtime.ui.subscribe();
            let _guard = super::rt().enter();
            super::rt().spawn(async move {
                while let Some(event) = rx.recv().await {
                    if sink
                        .add(AgentUiStatus {
                            dest: event.dest,
                            phase: event.phase,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            });
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = sink;
            Ok(())
        }
    }
}

pub struct AgentPermissionEvent {
    pub dest: String,
    pub call_id: String,
    pub name: String,
    pub preview: String,
}

pub struct AgentUiStatus {
    pub dest: String,
    pub phase: String,
}
