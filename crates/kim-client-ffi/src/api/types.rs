use kim_sdk as sdk;
use kim_sdk::{AgentProfileRow, LinkStateView};

pub use kim_sdk::{
    AgentCard, AgentCardState, AgentCardType, MediaKind, OutgoingContent, ProfileKind,
    ProfilePlacement, Relation, ThreadKind, ThreadPreview,
};

pub enum SendStatus {
    Pending,
    Uploading,
    Sending,
    Sent,
    Failed,
    Cancelled,
}

pub enum LinkState {
    Connecting,
    Online,
    Reconnecting { attempt: u32 },
    Offline,
}

pub struct MessageView {
    pub key: String,
    pub dest: String,
    pub sender: String,
    pub body: String,
    pub local_path: Option<String>,
    pub at: i64,
    pub sys: bool,
    pub kind: MediaKind,
    pub card: Option<AgentCard>,
    pub width: i32,
    pub height: i32,
    pub message_id: i64,
    pub batch_id: Option<String>,
    pub send_status: SendStatus,
}

pub struct ThreadView {
    pub id: String,
    pub kind: ThreadKind,
    pub title: String,
    pub avatar: String,
    pub last_body: String,
    pub preview: ThreadPreview,
    pub last_at: i64,
    pub unread: i32,
}

pub struct TimelineSnapshot {
    pub dest: String,
    pub version: u64,
    pub messages: Vec<MessageView>,
    pub pending: Vec<MessageView>,
    pub unread: i32,
    pub last_read_message_id: i64,
    pub has_more: bool,
    pub loading_older: bool,
    pub history_error: Option<String>,
}

pub struct TimelineDelta {
    pub dest: String,
    pub from_version: u64,
    pub to_version: u64,
    pub upserts: Vec<MessageView>,
    pub deleted_keys: Vec<String>,
    pub unread: Option<i32>,
    pub last_read_message_id: Option<i64>,
}

pub enum TimelineUpdate {
    Snapshot { snapshot: TimelineSnapshot },
    Delta { delta: TimelineDelta },
    Resync { dest: String, reason: String },
}

pub struct Person {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub relation: Relation,
    pub kind: ProfileKind,
}

pub struct ContactsSnapshot {
    pub version: u64,
    pub contacts: Vec<Person>,
    pub sync_error: Option<String>,
}

pub struct RoomMember {
    pub account: String,
    pub status: i32,
    pub last_seen: i64,
}

pub struct Profile {
    pub account: String,
    pub nickname: String,
    pub avatar: String,
    pub bio: String,
    pub kind: ProfileKind,
}

pub struct SessionSnapshot {
    pub link: LinkState,
    pub last_error: Option<String>,
    pub threads: Vec<ThreadView>,
    pub unread_total: i32,
}

pub enum SessionUpdate {
    Link {
        state: LinkState,
        last_error: Option<String>,
    },
    Inbox {
        threads: Vec<ThreadView>,
    },
    ThreadUpsert {
        thread: ThreadView,
    },
    SyncProgress {
        pulled: u64,
        catching_up: bool,
    },
    Kickout {
        channel_id: String,
    },
    AuthExpired {
        reason: String,
    },
    FriendRequest {
        from: String,
        nickname: String,
    },
    FriendAccepted {
        from: String,
        nickname: String,
    },
    ProfileUpdated {
        account: String,
        nickname: String,
        avatar: String,
    },
    Presence {
        account: String,
        status: i32,
        last_seen: i64,
    },
    Typing {
        typer: String,
        dest: String,
        kind: ThreadKind,
        active: bool,
    },
    ReceiptRead {
        reader: String,
        dest: String,
        kind: ThreadKind,
        message_id: i64,
    },
    GroupCreate {
        group_id: String,
        members: Vec<String>,
    },
    ContactsChanged {
        contacts: Vec<Person>,
    },
    AgentTurn {
        dest: String,
        state: AgentTurnState,
        text: String,
    },
    AgentCard {
        dest: String,
        card: AgentCard,
    },
    RustPanic {
        message: String,
    },
}

pub enum AgentTurnState {
    Queued,
    Running,
    WaitingPermission,
    Done,
    Error,
    Empty,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct AgentProfile {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    pub document_json: String,
    pub placement: ProfilePlacement,
    pub updated_at: i64,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct ProviderAccount {
    pub id: String,
    pub vendor_id: String,
    pub base_url: String,
    pub key_ref: String,
    pub display_name: String,
    pub models: Vec<String>,
    pub updated_at: i64,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct DeviceOverlay {
    pub profile_id: String,
    pub workspace_path: String,
    pub workspace_bookmark: String,
    pub user_agents_skills: String,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct AgentFlags {
    pub multi_profile: bool,
    pub server_identity: bool,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct Settings {
    pub ws_url: String,
    pub http_origin: String,
    pub env: String,
    pub locale: String,
    pub account: String,
}

impl From<kim_sdk::DeviceSettings> for Settings {
    fn from(row: kim_sdk::DeviceSettings) -> Self {
        Self {
            ws_url: row.ws_url,
            http_origin: row.http_origin,
            env: row.env,
            locale: row.locale,
            account: row.account,
        }
    }
}

pub enum SettingsPreset {
    Local,
    Prod,
}

impl From<SettingsPreset> for kim_sdk::SettingsPreset {
    fn from(p: SettingsPreset) -> Self {
        match p {
            SettingsPreset::Local => Self::Local,
            SettingsPreset::Prod => Self::Prod,
        }
    }
}

#[flutter_rust_bridge::frb(unignore)]
pub struct LocalMedia {
    pub local_path: String,
    pub byte_size: i64,
    pub width: i32,
    pub height: i32,
}

#[flutter_rust_bridge::frb(unignore)]
pub struct Metrics {
    pub enqueue_total: u64,
    pub persist_talk_total: u64,
    pub epoch_drop_total: u64,
    pub store_wipe_total: u64,
}

impl From<sdk::SendStatus> for SendStatus {
    fn from(v: sdk::SendStatus) -> Self {
        match v {
            sdk::SendStatus::Pending => Self::Pending,
            sdk::SendStatus::Uploading => Self::Uploading,
            sdk::SendStatus::Sending => Self::Sending,
            sdk::SendStatus::Sent => Self::Sent,
            sdk::SendStatus::Failed => Self::Failed,
            sdk::SendStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<LinkStateView> for LinkState {
    fn from(v: LinkStateView) -> Self {
        match v {
            LinkStateView::Connecting => Self::Connecting,
            LinkStateView::Online => Self::Online,
            LinkStateView::Reconnecting { attempt } => Self::Reconnecting { attempt },
            LinkStateView::Offline => Self::Offline,
        }
    }
}

impl From<sdk::MessageView> for MessageView {
    fn from(v: sdk::MessageView) -> Self {
        Self {
            key: v.key,
            dest: v.dest,
            sender: v.sender,
            body: v.body,
            local_path: v.local_path,
            at: v.at,
            sys: v.sys,
            kind: v.kind,
            card: v.card,
            width: v.width,
            height: v.height,
            message_id: v.message_id,
            batch_id: v.batch_id,
            send_status: v.send_status.into(),
        }
    }
}

impl From<sdk::ThreadView> for ThreadView {
    fn from(v: sdk::ThreadView) -> Self {
        Self {
            id: v.id,
            kind: v.kind,
            title: v.title,
            avatar: v.avatar,
            last_body: v.last_body,
            preview: v.preview,
            last_at: v.last_at,
            unread: v.unread,
        }
    }
}

impl From<sdk::TimelineSnapshot> for TimelineSnapshot {
    fn from(v: sdk::TimelineSnapshot) -> Self {
        Self {
            dest: v.dest,
            version: v.version,
            messages: v.messages.into_iter().map(Into::into).collect(),
            pending: v.pending.into_iter().map(Into::into).collect(),
            unread: v.unread,
            last_read_message_id: v.last_read_message_id,
            has_more: v.has_more,
            loading_older: v.loading_older,
            history_error: v.history_error,
        }
    }
}

impl From<sdk::TimelineDelta> for TimelineDelta {
    fn from(v: sdk::TimelineDelta) -> Self {
        Self {
            dest: v.dest,
            from_version: v.from_version,
            to_version: v.to_version,
            upserts: v.upserts.into_iter().map(Into::into).collect(),
            deleted_keys: v.deleted_keys,
            unread: v.unread,
            last_read_message_id: v.last_read_message_id,
        }
    }
}

impl From<sdk::TimelineUpdate> for TimelineUpdate {
    fn from(v: sdk::TimelineUpdate) -> Self {
        match v {
            sdk::TimelineUpdate::Snapshot { snapshot } => Self::Snapshot {
                snapshot: snapshot.into(),
            },
            sdk::TimelineUpdate::Delta { delta } => Self::Delta {
                delta: delta.into(),
            },
            sdk::TimelineUpdate::Resync { dest, reason } => Self::Resync { dest, reason },
        }
    }
}

impl From<sdk::SessionSnapshot> for SessionSnapshot {
    fn from(v: sdk::SessionSnapshot) -> Self {
        Self {
            link: v.link.into(),
            last_error: v.last_error,
            threads: v.threads.into_iter().map(Into::into).collect(),
            unread_total: v.unread_total,
        }
    }
}

impl From<sdk::SessionUpdate> for SessionUpdate {
    fn from(v: sdk::SessionUpdate) -> Self {
        match v {
            sdk::SessionUpdate::Link { state, last_error } => Self::Link {
                state: state.into(),
                last_error,
            },
            sdk::SessionUpdate::Inbox { threads } => Self::Inbox {
                threads: threads.into_iter().map(Into::into).collect(),
            },
            sdk::SessionUpdate::ThreadUpsert { thread } => Self::ThreadUpsert {
                thread: thread.into(),
            },
            sdk::SessionUpdate::SyncProgress {
                pulled,
                catching_up,
            } => Self::SyncProgress {
                pulled,
                catching_up,
            },
            sdk::SessionUpdate::Kickout { channel_id } => Self::Kickout { channel_id },
            sdk::SessionUpdate::AuthExpired { reason } => Self::AuthExpired { reason },
            sdk::SessionUpdate::FriendRequest { from, nickname } => {
                Self::FriendRequest { from, nickname }
            }
            sdk::SessionUpdate::FriendAccepted { from, nickname } => {
                Self::FriendAccepted { from, nickname }
            }
            sdk::SessionUpdate::ProfileUpdated {
                account,
                nickname,
                avatar,
            } => Self::ProfileUpdated {
                account,
                nickname,
                avatar,
            },
            sdk::SessionUpdate::Presence {
                account,
                status,
                last_seen,
            } => Self::Presence {
                account,
                status,
                last_seen,
            },
            sdk::SessionUpdate::Typing {
                typer,
                dest,
                kind,
                active,
                ..
            } => Self::Typing {
                typer,
                dest,
                kind,
                active,
            },
            sdk::SessionUpdate::ReceiptRead {
                reader,
                dest,
                kind,
                message_id,
            } => Self::ReceiptRead {
                reader,
                dest,
                kind,
                message_id,
            },
            sdk::SessionUpdate::GroupCreate { group_id, members } => {
                Self::GroupCreate { group_id, members }
            }
            sdk::SessionUpdate::ContactsChanged { contacts } => Self::ContactsChanged {
                contacts: contacts
                    .into_iter()
                    .map(|p| Person {
                        account: p.account,
                        nickname: p.nickname,
                        avatar: p.avatar,
                        bio: p.bio,
                        relation: p.relation,
                        kind: p.kind,
                    })
                    .collect(),
            },
            sdk::SessionUpdate::AgentTurn { dest, state, text } => Self::AgentTurn {
                dest,
                state: state.into(),
                text,
            },
            sdk::SessionUpdate::AgentCard { dest, card } => Self::AgentCard { dest, card },
            sdk::SessionUpdate::RustPanic { message } => Self::RustPanic { message },
        }
    }
}

impl From<sdk::AgentTurnState> for AgentTurnState {
    fn from(v: sdk::AgentTurnState) -> Self {
        match v {
            sdk::AgentTurnState::Queued => Self::Queued,
            sdk::AgentTurnState::Running => Self::Running,
            sdk::AgentTurnState::WaitingPermission => Self::WaitingPermission,
            sdk::AgentTurnState::Done => Self::Done,
            sdk::AgentTurnState::Error => Self::Error,
            sdk::AgentTurnState::Empty => Self::Empty,
        }
    }
}

impl From<AgentProfileRow> for AgentProfile {
    fn from(r: AgentProfileRow) -> Self {
        Self {
            profile_id: r.profile_id,
            nickname: r.nickname,
            server_account: r.server_account,
            document_json: r.document_json,
            placement: r.placement,
            updated_at: r.updated_at,
        }
    }
}

impl From<kim_sdk::ProviderAccountRow> for ProviderAccount {
    fn from(r: kim_sdk::ProviderAccountRow) -> Self {
        Self {
            id: r.id,
            vendor_id: r.vendor_id,
            base_url: r.base_url,
            key_ref: r.key_ref,
            display_name: r.display_name,
            models: r.models,
            updated_at: r.updated_at,
        }
    }
}

impl From<ProviderAccount> for kim_sdk::ProviderAccountRow {
    fn from(r: ProviderAccount) -> Self {
        Self {
            id: r.id,
            vendor_id: r.vendor_id,
            base_url: r.base_url,
            key_ref: r.key_ref,
            display_name: r.display_name,
            models: r.models,
            updated_at: r.updated_at,
            deleted_at: 0,
        }
    }
}

impl From<kim_sdk::DeviceOverlayRow> for DeviceOverlay {
    fn from(r: kim_sdk::DeviceOverlayRow) -> Self {
        Self {
            profile_id: r.profile_id,
            workspace_path: r.workspace_path,
            workspace_bookmark: r.workspace_bookmark,
            user_agents_skills: r.user_agents_skills,
        }
    }
}

impl From<DeviceOverlay> for kim_sdk::DeviceOverlayRow {
    fn from(r: DeviceOverlay) -> Self {
        Self {
            profile_id: r.profile_id,
            workspace_path: r.workspace_path,
            workspace_bookmark: r.workspace_bookmark,
            user_agents_skills: r.user_agents_skills,
        }
    }
}

impl From<kim_sdk::PersonRef> for Person {
    fn from(p: kim_sdk::PersonRef) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            relation: p.relation,
            kind: p.kind,
        }
    }
}

impl From<sdk::ContactsSnapshot> for ContactsSnapshot {
    fn from(snapshot: sdk::ContactsSnapshot) -> Self {
        Self {
            version: snapshot.version,
            contacts: snapshot.contacts.into_iter().map(Into::into).collect(),
            sync_error: snapshot.sync_error,
        }
    }
}

impl From<Person> for kim_sdk::PersonRef {
    fn from(p: Person) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            relation: p.relation,
            kind: p.kind,
        }
    }
}

impl Person {
    pub(crate) fn from_profile(p: kim_client::Profile, relation: Relation) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            relation,
            kind: ProfileKind::from_wire(p.kind),
        }
    }
}

impl From<kim_client::Profile> for Profile {
    fn from(p: kim_client::Profile) -> Self {
        Self {
            account: p.account,
            nickname: p.nickname,
            avatar: p.avatar,
            bio: p.bio,
            kind: ProfileKind::from_wire(p.kind),
        }
    }
}

impl From<kim_client::Profile> for Person {
    fn from(p: kim_client::Profile) -> Self {
        Self::from_profile(p, Relation::Friend)
    }
}

#[flutter_rust_bridge::frb(mirror(ThreadKind))]
#[allow(dead_code)]
pub enum _ThreadKind {
    User,
    Group,
}

#[flutter_rust_bridge::frb(mirror(MediaKind))]
#[allow(dead_code)]
pub enum _MediaKind {
    Text,
    Image,
    Video,
    Voice,
    Card,
}

#[flutter_rust_bridge::frb(mirror(Relation))]
#[allow(dead_code)]
pub enum _Relation {
    Friend,
    Incoming,
    Outgoing,
}

#[flutter_rust_bridge::frb(mirror(ProfileKind))]
#[allow(dead_code)]
pub enum _ProfileKind {
    User,
    Bot,
}

#[flutter_rust_bridge::frb(mirror(ProfilePlacement))]
#[allow(dead_code)]
pub enum _ProfilePlacement {
    Local,
    Cloud,
}

#[flutter_rust_bridge::frb(mirror(AgentCardType))]
#[allow(dead_code)]
pub enum _AgentCardType {
    Tool,
    ActionRequired,
}

#[flutter_rust_bridge::frb(mirror(AgentCardState))]
#[allow(dead_code)]
pub enum _AgentCardState {
    Pending,
    Ok,
    Error,
}

#[flutter_rust_bridge::frb(mirror(AgentCard))]
#[allow(dead_code)]
pub struct _AgentCard {
    pub call_id: String,
    pub name: String,
    pub card_type: AgentCardType,
    pub state: AgentCardState,
    pub preview: String,
    pub ok: bool,
}

#[flutter_rust_bridge::frb(mirror(ThreadPreview))]
#[allow(dead_code)]
pub enum _ThreadPreview {
    Text { snippet: String },
    Media { kind: MediaKind },
    System { text: String },
}

#[flutter_rust_bridge::frb(mirror(OutgoingContent))]
#[allow(dead_code)]
pub enum _OutgoingContent {
    Text {
        body: String,
    },
    Image {
        path: String,
        mime: String,
        width: i32,
        height: i32,
        byte_size: i64,
    },
    Video {
        path: String,
        byte_size: i64,
    },
    Voice {
        path: String,
        byte_size: i64,
    },
}
