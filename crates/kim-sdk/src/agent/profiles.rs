use crate::error::SdkError;
use crate::store::Store;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProfileRow {
    pub profile_id: String,
    pub nickname: String,
    pub server_account: String,
    pub body_json: String,
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
