use kim_agent_storage::{Storage, StorageError, ValueAddr, Write};
use kim_agent_types::{AgentMessage, Entry, EntryBase, EntryId, LaneState, StopReason};
use serde_json::json;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("invalid tip")]
    InvalidTip,
    #[error("serde: {0}")]
    Serde(String),
}

pub struct SessionFacade {
    storage: Arc<dyn Storage>,
    lane: String,
}

impl SessionFacade {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self {
            storage,
            lane: "main".into(),
        }
    }

    pub fn lane(&self) -> &str {
        &self.lane
    }

    pub fn storage(&self) -> &Arc<dyn Storage> {
        &self.storage
    }

    pub async fn branch_tip(&self) -> Result<Option<EntryId>, SessionError> {
        let v = self
            .storage
            .get_value(&ValueAddr::branch_tip(&self.lane))
            .await?;
        Ok(match v {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) => {
                let u = Uuid::parse_str(&s).map_err(|_| SessionError::InvalidTip)?;
                Some(EntryId::from_uuid(u))
            }
            _ => return Err(SessionError::InvalidTip),
        })
    }

    pub async fn lane_state(&self) -> Result<LaneState, SessionError> {
        let v = self
            .storage
            .get_value(&ValueAddr::lane_state(&self.lane))
            .await?;
        match v {
            None => Ok(LaneState::default()),
            Some(val) => {
                serde_json::from_value(val).map_err(|e| SessionError::Serde(e.to_string()))
            }
        }
    }

    pub async fn set_lane_state_writes(&self, state: &LaneState) -> Result<Write, SessionError> {
        Ok(Write::SetValue {
            address: ValueAddr::lane_state(&self.lane),
            value: serde_json::to_value(state).map_err(|e| SessionError::Serde(e.to_string()))?,
        })
    }

    pub fn message_entry(parent: Option<EntryId>, message: AgentMessage) -> Entry {
        Entry::Message {
            base: EntryBase {
                id: EntryId::new(),
                parent_id: parent,
                seq: 0,
                timestamp_ms: 0,
            },
            message,
        }
    }

    pub fn tip_write(&self, id: EntryId) -> Write {
        Write::SetValue {
            address: ValueAddr::branch_tip(&self.lane),
            value: json!(id.to_string()),
        }
    }

    /// Walk tip → root, reverse to oldest-first; drop error/aborted assistants.
    pub async fn project_context(
        &self,
        max_entries: usize,
    ) -> Result<Vec<AgentMessage>, SessionError> {
        let tip = self.branch_tip().await?;
        let mut chain = Vec::new();
        let mut cur = tip;
        while let Some(id) = cur {
            if chain.len() >= max_entries {
                break;
            }
            let Some(entry) = self.storage.get_entry(id).await? else {
                break;
            };
            cur = entry.parent_id();
            if let Some(msg) = entry.message().cloned() {
                let drop = matches!(
                    &msg,
                    AgentMessage::Assistant {
                        stop_reason: StopReason::Error | StopReason::Aborted,
                        ..
                    }
                );
                if !drop {
                    chain.push(msg);
                }
            }
        }
        chain.reverse();
        Ok(chain)
    }
}
