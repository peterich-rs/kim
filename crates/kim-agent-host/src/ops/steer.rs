use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Operation;
use goose_provider_types::conversation::Conversation;

use crate::events::HostEffect;
use crate::HostSession;

pub struct SteerOp {
    pub steer: String,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for SteerOp {
    fn name(&self) -> &'static str {
        "steer"
    }

    async fn prompt_parts(
        &self,
        _session: &HostSession,
        _conversation: &Conversation,
    ) -> Result<Vec<(String, String)>> {
        if self.steer.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![("system".into(), self.steer.clone())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HostSession;
    use goose_provider_types::conversation::Conversation;

    #[tokio::test]
    async fn empty_steer_adds_nothing() {
        let op = SteerOp {
            steer: String::new(),
        };
        let parts = op
            .prompt_parts(
                &HostSession {
                    id: "s".into(),
                    conversation: Conversation::empty(),
                },
                &Conversation::empty(),
            )
            .await
            .unwrap();
        assert!(parts.is_empty());
    }

    #[tokio::test]
    async fn steer_appends_system_part() {
        let op = SteerOp {
            steer: "Be terse.".into(),
        };
        let parts = op
            .prompt_parts(
                &HostSession {
                    id: "s".into(),
                    conversation: Conversation::empty(),
                },
                &Conversation::empty(),
            )
            .await
            .unwrap();
        assert_eq!(parts, vec![("system".into(), "Be terse.".into())]);
    }
}
