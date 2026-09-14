use std::sync::Arc;

use serde_json::{json, Value};

use crate::capability::{
    AssembleCtx, CapabilityBlock, CapabilityPart, PermissionMatch, PermissionRule, PreviewTool,
    RiskTier,
};
use crate::ops::bash::BashToolProvider;
use crate::profile::PermissionDefault;
use crate::HostError;

pub struct BashBlock;

impl CapabilityBlock for BashBlock {
    fn kind(&self) -> &'static str {
        "bash"
    }

    fn risk(&self) -> RiskTier {
        RiskTier::Destructive
    }

    fn param_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "allow_argv_prefixes": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "additionalProperties": false
        })
    }

    fn check(&self, _params: &Value, _ctx: &AssembleCtx<'_>) -> Result<(), HostError> {
        Ok(())
    }

    fn build(&self, _params: &Value, ctx: &AssembleCtx<'_>) -> Result<CapabilityPart, HostError> {
        Ok(CapabilityPart {
            kind: "bash".into(),
            instance_id: "bash".into(),
            risk: RiskTier::Destructive,
            prompt_parts: vec![(
                "capability".into(),
                "You have bash (argv process in the workspace; always requires user confirmation)."
                    .into(),
            )],
            deferred_tool_names: Vec::new(),
            in_process: Some(Arc::new(BashToolProvider {
                root: ctx.project_root.to_path_buf(),
            })),
            permission_defaults: vec![PermissionRule {
                r#match: PermissionMatch::Tool {
                    name: "bash".into(),
                },
                effect: PermissionDefault::AskBefore,
            }],
            preview_tools: vec![PreviewTool {
                name: "bash".into(),
                source: "bash".into(),
                executor: "rust".into(),
            }],
        })
    }
}
