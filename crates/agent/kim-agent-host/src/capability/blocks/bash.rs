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
            "properties": {},
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
                "## Shell\n\
bash — run a command in the workspace. Prefer dedicated \
file tools over shell for reading, editing, or searching files."
                    .into(),
            )],
            deferred_tool_names: Vec::new(),
            in_process: Some(Arc::new(BashToolProvider {
                root: ctx.project_root.to_path_buf(),
                timeout: crate::harness::bash_timeout(ctx.profile),
            })),
            permission_defaults: vec![PermissionRule {
                r#match: PermissionMatch::Tool {
                    name: "bash".into(),
                },
                effect: PermissionDefault::AlwaysAllow,
            }],
            preview_tools: vec![PreviewTool {
                name: "bash".into(),
                source: "bash".into(),
                executor: "rust".into(),
            }],
        })
    }
}
