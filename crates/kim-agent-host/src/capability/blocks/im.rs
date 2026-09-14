//! IM deferred capability blocks (Dart executor).

use std::sync::Arc;

use serde_json::{json, Value};

use crate::capability::{
    AssembleCtx, CapabilityBlock, CapabilityPart, PermissionMatch, PermissionRule, PreviewTool,
    RiskTier,
};
use crate::ops::deferred_kim::kim_tool_schema;
use crate::profile::PermissionDefault;
use crate::HostError;

struct ImBlock {
    kind: &'static str,
    tool: &'static str,
    risk: RiskTier,
    ask: bool,
    fragment: &'static str,
}

impl CapabilityBlock for ImBlock {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn risk(&self) -> RiskTier {
        self.risk
    }

    fn param_schema(&self) -> Value {
        json!({ "type": "object", "properties": {}, "additionalProperties": false })
    }

    fn check(&self, _params: &Value, _ctx: &AssembleCtx<'_>) -> Result<(), HostError> {
        if kim_tool_schema(self.tool).is_none() {
            return Err(HostError::Failed(format!(
                "unknown IM tool schema: {}",
                self.tool
            )));
        }
        Ok(())
    }

    fn build(&self, _params: &Value, _ctx: &AssembleCtx<'_>) -> Result<CapabilityPart, HostError> {
        let effect = if self.ask {
            PermissionDefault::AskBefore
        } else {
            PermissionDefault::AlwaysAllow
        };
        Ok(CapabilityPart {
            kind: self.kind.into(),
            instance_id: self.kind.into(),
            risk: self.risk,
            prompt_parts: vec![("capability".into(), self.fragment.into())],
            deferred_tool_names: vec![self.tool.into()],
            in_process: None,
            permission_defaults: vec![PermissionRule {
                r#match: PermissionMatch::Tool {
                    name: self.tool.into(),
                },
                effect,
            }],
            preview_tools: vec![PreviewTool {
                name: self.tool.into(),
                source: self.kind.into(),
                executor: "dart".into(),
            }],
        })
    }
}

pub fn all() -> Vec<Arc<dyn CapabilityBlock>> {
    vec![
        Arc::new(ImBlock {
            kind: "im.send_message",
            tool: "send_message",
            risk: RiskTier::Write,
            ask: true,
            fragment: "You have send_message (requires user confirmation).",
        }),
        Arc::new(ImBlock {
            kind: "im.search_contacts",
            tool: "search_contacts",
            risk: RiskTier::Read,
            ask: false,
            fragment: "You have search_contacts.",
        }),
        Arc::new(ImBlock {
            kind: "im.search_messages",
            tool: "search_messages",
            risk: RiskTier::Read,
            ask: false,
            fragment: "You have search_messages.",
        }),
        Arc::new(ImBlock {
            kind: "im.get_conversation_context",
            tool: "get_conversation_context",
            risk: RiskTier::Read,
            ask: false,
            fragment: "You have get_conversation_context.",
        }),
        Arc::new(ImBlock {
            kind: "im.read_clipboard",
            tool: "read_clipboard",
            risk: RiskTier::External,
            ask: true,
            fragment: "You have read_clipboard (requires user confirmation).",
        }),
        Arc::new(ImBlock {
            kind: "im.list_profiles",
            tool: "list_profiles",
            risk: RiskTier::Read,
            ask: false,
            fragment: "You have list_profiles.",
        }),
    ]
}
