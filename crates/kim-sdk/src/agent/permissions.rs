#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct AgentPermissionRow {
    pub profile_id: String,
    pub tool: String,
    pub decision: String,
}
