use std::sync::Arc;

use serde_json::{json, Value};

use crate::capability::{
    AssembleCtx, CapabilityBlock, CapabilityPart, PermissionMatch, PermissionRule, PreviewTool,
    RiskTier,
};
use crate::ops::fs::FsToolProvider;
use crate::profile::PermissionDefault;
use crate::HostError;

pub struct FsBlock;

impl CapabilityBlock for FsBlock {
    fn kind(&self) -> &'static str {
        "fs"
    }

    fn risk(&self) -> RiskTier {
        RiskTier::Read
    }

    fn param_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "writable": { "type": "boolean", "default": false }
            },
            "additionalProperties": false
        })
    }

    fn check(&self, _params: &Value, _ctx: &AssembleCtx<'_>) -> Result<(), HostError> {
        Ok(())
    }

    fn build(&self, params: &Value, ctx: &AssembleCtx<'_>) -> Result<CapabilityPart, HostError> {
        let writable = params
            .get("writable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let risk = if writable {
            RiskTier::Write
        } else {
            RiskTier::Read
        };
        let fragment = if writable {
            "## Workspace files\n\
read_file / list_dir — read files inside the workspace. write_file additionally \
creates and modifies files; it is gated (user confirms)."
        } else {
            "## Workspace files\n\
read_file / list_dir — read files inside the workspace (read-only). Do not claim \
write_file or bash unless they were separately granted."
        };
        let mut preview_tools = vec![
            PreviewTool {
                name: "read_file".into(),
                source: "fs".into(),
                executor: "rust".into(),
            },
            PreviewTool {
                name: "list_dir".into(),
                source: "fs".into(),
                executor: "rust".into(),
            },
        ];
        let mut permission_defaults = Vec::new();
        if writable {
            preview_tools.push(PreviewTool {
                name: "write_file".into(),
                source: "fs".into(),
                executor: "rust".into(),
            });
            permission_defaults.push(PermissionRule {
                r#match: PermissionMatch::Tool {
                    name: "write_file".into(),
                },
                effect: PermissionDefault::AlwaysAllow,
            });
        }
        Ok(CapabilityPart {
            kind: "fs".into(),
            instance_id: "fs".into(),
            risk,
            prompt_parts: vec![("capability".into(), fragment.into())],
            deferred_tool_names: Vec::new(),
            in_process: Some(Arc::new(FsToolProvider {
                root: ctx.project_root.to_path_buf(),
                writable,
            })),
            permission_defaults,
            preview_tools,
        })
    }
}
