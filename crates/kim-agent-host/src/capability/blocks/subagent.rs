use std::sync::Arc;

use serde_json::{json, Value};

use crate::capability::{
    AssembleCtx, CapabilityBlock, CapabilityPart, PermissionMatch, PermissionRule, PreviewTool,
    RiskTier,
};
use crate::ops::subagent::SubagentOp;
use crate::profile::PermissionDefault;
use crate::HostError;

pub struct SubagentBlock;

impl CapabilityBlock for SubagentBlock {
    fn kind(&self) -> &'static str {
        "subagent"
    }

    fn risk(&self) -> RiskTier {
        RiskTier::Write
    }

    fn param_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn check(&self, _params: &Value, _ctx: &AssembleCtx<'_>) -> Result<(), HostError> {
        Ok(())
    }

    fn build(&self, _params: &Value, ctx: &AssembleCtx<'_>) -> Result<CapabilityPart, HostError> {
        Ok(CapabilityPart {
            kind: "subagent".into(),
            instance_id: "subagent".into(),
            risk: RiskTier::Write,
            prompt_parts: vec![(
                "capability".into(),
                "## Subagent\n\
delegate — run a smaller child agent on a focused task. Gated (user confirms)."
                    .into(),
            )],
            deferred_tool_names: Vec::new(),
            in_process: Some(Arc::new(SubagentOp {
                provider: Arc::clone(&ctx.provider),
                model: ctx.model.clone(),
            })),
            permission_defaults: vec![PermissionRule {
                r#match: PermissionMatch::Tool {
                    name: "delegate".into(),
                },
                effect: PermissionDefault::AskBefore,
            }],
            preview_tools: vec![PreviewTool {
                name: "delegate".into(),
                source: "subagent".into(),
                executor: "rust".into(),
            }],
        })
    }
}
