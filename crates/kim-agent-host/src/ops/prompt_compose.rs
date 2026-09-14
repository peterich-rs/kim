//! Layered system prompt parts (identity, digest, workspace, skills, steer).

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::Operation;
use goose_provider_types::conversation::Conversation;

use crate::events::HostEffect;
use crate::HostSession;

pub struct PromptComposeOp {
    /// `(label, text)` — labels are for preview; inference uses role `"system"`.
    pub layers: Vec<(String, String)>,
}

#[async_trait]
impl Operation<HostSession, HostEffect> for PromptComposeOp {
    fn name(&self) -> &'static str {
        "prompt_compose"
    }

    async fn prompt_parts(
        &self,
        _session: &HostSession,
        _conversation: &Conversation,
    ) -> Result<Vec<(String, String)>> {
        Ok(self
            .layers
            .iter()
            .filter(|(_, text)| !text.trim().is_empty())
            .map(|(_, text)| ("system".into(), text.clone()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_provider_types::conversation::Conversation;

    #[tokio::test]
    async fn layer_order_stable() {
        let op = PromptComposeOp {
            layers: vec![
                ("identity".into(), "I".into()),
                ("capability_digest".into(), "D".into()),
                ("steer".into(), "S".into()),
            ],
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
        assert_eq!(
            parts,
            vec![
                ("system".into(), "I".into()),
                ("system".into(), "D".into()),
                ("system".into(), "S".into()),
            ]
        );
    }

    #[tokio::test]
    async fn empty_layers_skipped() {
        let op = PromptComposeOp {
            layers: vec![
                ("identity".into(), "I".into()),
                ("steer".into(), "  ".into()),
            ],
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
        assert_eq!(parts, vec![("system".into(), "I".into())]);
    }
}
