use crate::error::SdkError;
use crate::model::ProfilePlacement;
use crate::store::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProfileRow {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    /// Filled by `list_agent_profiles` via `blob_to_json`. Empty on write.
    pub document_json: String,
    pub body_blob: Vec<u8>,
    pub placement: ProfilePlacement,
    pub updated_at: i64,
    pub deleted_at: i64,
}

impl Default for AgentProfileRow {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            nickname: String::new(),
            server_account: String::new(),
            document_json: String::new(),
            body_blob: Vec::new(),
            placement: ProfilePlacement::Local,
            updated_at: 0,
            deleted_at: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderAccountRow {
    pub id: String,
    pub vendor_id: String,
    pub base_url: String,
    pub key_ref: String,
    pub display_name: String,
    pub models: Vec<String>,
    pub updated_at: i64,
    pub deleted_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceOverlayRow {
    pub profile_id: String,
    pub workspace_path: String,
    pub workspace_bookmark: String,
    pub user_agents_skills: String,
}

pub async fn profile_id_for_dest(
    store: &Store,
    account: &str,
    dest: &str,
) -> Result<Option<String>, SdkError> {
    if dest.is_empty() {
        return Ok(None);
    }
    store.agent_profile_id_for_dest(account, dest).await
}
