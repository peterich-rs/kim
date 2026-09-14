//! Capability blocks: the assembly unit for agent tools + prompt fragments.

mod blocks;
mod legacy;
pub mod preview;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use goose_agent::tool::ToolProvider;
use goose_provider_types::base::Provider;
use goose_provider_types::model::ModelConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ops::mcp::McpHub;
use crate::profile::{
    AgentProfile, ExtensionSpec, PermissionDefault, PermissionConfig, ToolSet, WorkspaceKind,
};
use crate::skills::{
    build_registry, catalog_prompt_block, read_agents_md, RegistryScan, SkillRegistry, SkillResolver,
};
use crate::{HostError, HostSession};

pub use legacy::from_legacy;
pub use preview::{preview_assembled, AssembledPreview, PreviewTool};

fn enabled_true() -> bool {
    true
}

/// Persisted reference. `kind` selects the registered block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityRef {
    pub kind: String,
    /// Optional instance id (`mcp:github`). Empty → kind is enough for singletons.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    Read,
    Write,
    External,
    Destructive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "match", rename_all = "snake_case")]
pub enum PermissionMatch {
    Tool { name: String },
    FsPath { prefix: String },
    BashArgv { prefix: String },
    McpTool { extension: String, name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub r#match: PermissionMatch,
    pub effect: PermissionDefault,
}

/// What one enabled block contributes after `build`.
pub(crate) struct CapabilityPart {
    pub kind: String,
    pub instance_id: String,
    #[allow(dead_code)] // Used by catalog / future permission UX.
    pub risk: RiskTier,
    /// Injected as system prompt_parts; must stay in sync with tools.
    pub prompt_parts: Vec<(String, String)>,
    pub deferred_tool_names: Vec<String>,
    pub in_process: Option<Arc<dyn ToolProvider<HostSession>>>,
    pub permission_defaults: Vec<PermissionRule>,
    /// Static tool advertisement for preview (avoids live MCP listing).
    pub preview_tools: Vec<PreviewTool>,
}

pub(crate) struct AssembleCtx<'a> {
    pub profile: &'a AgentProfile,
    pub project_root: &'a Path,
    pub mcp: Arc<McpHub>,
    pub provider: Arc<dyn Provider>,
    pub model: ModelConfig,
}

pub(crate) trait CapabilityBlock: Send + Sync {
    fn kind(&self) -> &'static str;
    fn risk(&self) -> RiskTier;
    fn param_schema(&self) -> Value;
    fn check(&self, params: &Value, ctx: &AssembleCtx<'_>) -> Result<(), HostError>;
    fn build(&self, params: &Value, ctx: &AssembleCtx<'_>) -> Result<CapabilityPart, HostError>;
}

/// Shared resolve path for `MachineFactory::assemble` and `preview_assembled`.
pub(crate) struct ResolvedParts {
    pub parts: Vec<CapabilityPart>,
    pub warnings: Vec<String>,
    pub skill_registry: Arc<SkillRegistry>,
    pub skill_resolver: Arc<SkillResolver>,
}

pub(crate) fn registry() -> &'static HashMap<&'static str, Arc<dyn CapabilityBlock>> {
    static REG: OnceLock<HashMap<&'static str, Arc<dyn CapabilityBlock>>> = OnceLock::new();
    REG.get_or_init(blocks::build_registry)
}

pub fn catalog_entries() -> Vec<Value> {
    registry()
        .values()
        .map(|b| {
            serde_json::json!({
                "kind": b.kind(),
                "risk": format!("{:?}", b.risk()).to_ascii_lowercase(),
                "param_schema": b.param_schema(),
            })
        })
        .collect()
}

impl CapabilityRef {
    pub fn from_legacy(tools: &ToolSet, extensions: &[ExtensionSpec]) -> Vec<Self> {
        from_legacy(tools, extensions)
    }
}

pub(crate) fn resolve_parts(
    profile: &AgentProfile,
    project_root: &Path,
    mcp: Arc<McpHub>,
    provider: Arc<dyn Provider>,
    model: ModelConfig,
) -> ResolvedParts {
    let caps: Vec<CapabilityRef> = profile
        .resolve_capabilities()
        .into_iter()
        .filter(|c| c.enabled)
        .collect();

    let projected = ToolSet::from_capabilities(&caps);
    let scan_portable =
        projected.fs || projected.fs_write || profile.workspace.kind == WorkspaceKind::Repo;

    let skill_resolver = Arc::new(SkillResolver::new());
    let skill_registry = Arc::new(if scan_portable || !profile.skills.is_empty() {
        build_registry(
            &profile.skills,
            &profile.portable_denylist,
            &RegistryScan {
                user_root: &profile.user_agents_skills,
                project_root,
                enabled: scan_portable,
            },
            &skill_resolver,
        )
    } else {
        SkillRegistry::default()
    });

    let ctx = AssembleCtx {
        profile,
        project_root,
        mcp,
        provider,
        model,
    };

    let reg = registry();
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    let mut seen_mcp_provider = false;

    for cap in &caps {
        let Some(block) = reg.get(cap.kind.as_str()) else {
            warnings.push(format!("unknown capability kind: {}", cap.kind));
            continue;
        };
        if let Err(err) = block.check(&cap.params, &ctx) {
            warnings.push(format!("{}: {err}", cap.kind));
            continue;
        }
        match block.build(&cap.params, &ctx) {
            Ok(mut part) => {
                if !cap.id.is_empty() {
                    part.instance_id = cap.id.clone();
                }
                // Multiple MCP refs share one hub provider.
                if part.kind == "mcp" {
                    if seen_mcp_provider {
                        part.in_process = None;
                    } else {
                        seen_mcp_provider = true;
                    }
                }
                parts.push(part);
            }
            Err(err) => warnings.push(format!("{}: {err}", cap.kind)),
        }
    }

    // Legacy extensions without capability refs still need the MCP provider.
    if !seen_mcp_provider && !profile.extensions.is_empty() {
        parts.push(CapabilityPart {
            kind: "mcp".into(),
            instance_id: "mcp".into(),
            risk: RiskTier::External,
            prompt_parts: vec![(
                "capability".into(),
                format!(
                    "You have MCP tools from: {} (names look like ext__tool).",
                    profile
                        .extensions
                        .iter()
                        .map(|e| e.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )],
            deferred_tool_names: Vec::new(),
            in_process: Some(Arc::new(crate::ops::mcp::McpToolProvider {
                hub: Arc::clone(&ctx.mcp),
            })),
            permission_defaults: Vec::new(),
            preview_tools: profile
                .extensions
                .iter()
                .map(|e| PreviewTool {
                    name: format!("{}__*", e.name),
                    source: format!("mcp:{}", e.name),
                    executor: "rust".into(),
                })
                .collect(),
        });
    }

    ResolvedParts {
        parts,
        warnings,
        skill_registry,
        skill_resolver,
    }
}

/// L1 identity + L2 digest + L3 AGENTS.md + L4 skill catalog + L5 steer.
pub(crate) fn build_prompt_layers(
    profile: &AgentProfile,
    project_root: &Path,
    parts: &[CapabilityPart],
    skill_registry: &SkillRegistry,
) -> Vec<(String, String)> {
    let mut layers = Vec::new();

    let identity = profile.effective_identity_prompt().to_string();
    if !identity.trim().is_empty() {
        layers.push(("identity".into(), identity));
    }

    let mut digest_bits: Vec<String> = Vec::new();
    for part in parts {
        for (_label, text) in &part.prompt_parts {
            if !text.trim().is_empty() {
                digest_bits.push(text.trim().to_string());
            }
        }
    }
    if !digest_bits.is_empty() {
        layers.push(("capability_digest".into(), digest_bits.join(" ")));
    }

    let projected = profile.project_toolset();
    let scan_portable =
        projected.fs || projected.fs_write || profile.workspace.kind == WorkspaceKind::Repo;
    if scan_portable {
        if let Some(agents_md) = read_agents_md(project_root) {
            layers.push((
                "workspace_agents_md".into(),
                format!("Workspace AGENTS.md:\n{agents_md}"),
            ));
        }
    }

    let catalog = catalog_prompt_block(skill_registry);
    if !catalog.is_empty() {
        layers.push(("skill_catalog".into(), catalog));
    }

    let steer = profile.steer.trim();
    if !steer.is_empty() {
        layers.push(("steer".into(), steer.to_string()));
    }

    layers
}

pub(crate) fn merge_permission_config(profile: &AgentProfile, parts: &[CapabilityPart]) -> PermissionConfig {
    let mut config = profile.permissions.clone();
    for part in parts {
        for rule in &part.permission_defaults {
            if let PermissionMatch::Tool { name } = &rule.r#match {
                config.tools.entry(name.clone()).or_insert(rule.effect);
            }
        }
    }
    for rule in &profile.permission_rules {
        if let PermissionMatch::Tool { name } = &rule.r#match {
            config.tools.insert(name.clone(), rule.effect);
        }
    }
    config
}

pub(crate) fn flatten_deferred(parts: &[CapabilityPart]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for part in parts {
        for name in &part.deferred_tool_names {
            if seen.insert(name.clone()) {
                names.push(name.clone());
            }
        }
    }
    names
}

pub(crate) fn flatten_providers(parts: &[CapabilityPart]) -> Vec<Arc<dyn ToolProvider<HostSession>>> {
    parts.iter().filter_map(|p| p.in_process.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::LegacyOpenOpts;
    use crate::DEFAULT_IDENTITY_PROMPT;

    #[test]
    fn legacy_goose_fixture_resolves_im_caps() {
        let raw = include_str!("../../tests/fixtures/capability_legacy_goose.json");
        let profile: AgentProfile = serde_json::from_str(raw).unwrap();
        let caps = profile.resolve_capabilities();
        assert!(caps.iter().any(|c| c.kind == "im.send_message"));
        assert!(caps.iter().any(|c| c.kind == "im.read_clipboard"));
        assert!(!caps.iter().any(|c| c.kind == "fs"));
        let projected = ToolSet::from_capabilities(&caps);
        assert!(projected.send_message);
        assert!(!projected.fs);
    }

    #[test]
    fn coder_fixture_projects_fs_writable_and_bash() {
        let raw = include_str!("../../tests/fixtures/capability_coder_fs_bash.json");
        let profile: AgentProfile = serde_json::from_str(raw).unwrap();
        let caps = profile.resolve_capabilities();
        assert!(caps.iter().any(|c| {
            c.kind == "fs" && c.params.get("writable") == Some(&serde_json::json!(true))
        }));
        assert!(caps.iter().any(|c| c.kind == "bash"));
        assert!(!caps.iter().any(|c| c.kind.starts_with("im.")));
    }

    #[test]
    fn empty_caps_identity_has_no_send_message_claim() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        assert!(!profile.project_toolset().has_any());
        let identity = profile.effective_identity_prompt();
        assert_eq!(identity, DEFAULT_IDENTITY_PROMPT);
        assert!(!identity.contains("send_message"));
    }
}
