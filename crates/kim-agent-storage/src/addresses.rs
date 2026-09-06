use kim_agent_types::OperationId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValueAddr {
    pub namespace: String,
    pub key: String,
}

pub type ListAddr = ValueAddr;

impl ValueAddr {
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            key: key.into(),
        }
    }

    pub fn branch_tip(lane: &str) -> Self {
        Self::new("pi.branch.tip", lane)
    }

    pub fn lane_config(lane: &str) -> Self {
        Self::new("pi.lane.config", lane)
    }

    pub fn lane_state(lane: &str) -> Self {
        Self::new("pi.lane.state", lane)
    }

    pub fn op_meta(op: OperationId) -> Self {
        Self::new("pi.op.meta", op.to_string())
    }

    pub fn op_state(op: OperationId) -> Self {
        Self::new("pi.op.state", op.to_string())
    }

    pub fn op_tool_args(op: OperationId, step: &str, i: usize) -> Self {
        Self::new("pi.op.tool_args", format!("{op}:{step}:{i}"))
    }

    pub fn pending_entry(id: impl ToString) -> Self {
        Self::new("pi.pending.entry", id.to_string())
    }

    pub fn pending_tool_output(op: OperationId, inv: &str) -> Self {
        Self::new("pi.pending.tool_output", format!("{op}:{inv}"))
    }

    pub fn op_result(op: OperationId) -> Self {
        Self::new("pi.result", op.to_string())
    }
}
