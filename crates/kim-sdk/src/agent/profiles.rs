use crate::error::SdkError;
use crate::store::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProfileRow {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    pub body_json: String,
    pub body_blob: Vec<u8>,
    pub placement: String,
    pub updated_at: i64,
    pub deleted_at: i64,
}

impl Default for AgentProfileRow {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            nickname: String::new(),
            server_account: String::new(),
            body_json: String::new(),
            body_blob: Vec::new(),
            placement: "local".into(),
            updated_at: 0,
            deleted_at: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderAccountRow {
    pub id: String,
    pub vendor_id: String,
    pub base_url: String,
    pub key_ref: String,
    pub display_name: String,
    pub models_json: String,
    pub updated_at: i64,
    pub deleted_at: i64,
}

impl Default for ProviderAccountRow {
    fn default() -> Self {
        Self {
            id: String::new(),
            vendor_id: String::new(),
            base_url: String::new(),
            key_ref: String::new(),
            display_name: String::new(),
            models_json: "[]".into(),
            updated_at: 0,
            deleted_at: 0,
        }
    }
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
