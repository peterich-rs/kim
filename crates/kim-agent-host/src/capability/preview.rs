//! Assembled preview sharing `resolve_parts` with MachineFactory.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use goose_provider_types::base::{MessageStream, Provider};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::errors::ProviderError;
use goose_provider_types::model::ModelConfig;
use rmcp::model::Tool;
use serde::{Deserialize, Serialize};

use crate::capability::{
    build_prompt_layers, flatten_deferred, merge_permission_config, resolve_parts, CapabilityPart,
};
use crate::machine::model_config;
use crate::ops::mcp::McpHub;
use crate::profile::{AgentProfile, PermissionDefault};
use crate::HostError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewTool {
    pub name: String,
    pub source: String,
    pub executor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssembledPreview {
    pub identity: String,
    pub prompt_layers: Vec<(String, String)>,
    pub full_system_prompt: String,
    pub tools: Vec<PreviewTool>,
    pub permission_matrix: Vec<(String, String)>,
    pub warnings: Vec<String>,
    pub step_names: Vec<String>,
}

struct PreviewProvider;

#[async_trait]
impl Provider for PreviewProvider {
    fn get_name(&self) -> &str {
        "preview"
    }

    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system: &str,
        _messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        Err(ProviderError::ExecutionError(
            "preview provider does not stream".into(),
        ))
    }
}

/// Dry assembly preview: tools + layered prompts + warnings (no network).
pub fn preview_assembled(
    profile: &AgentProfile,
    project_root: &Path,
) -> Result<AssembledPreview, HostError> {
    let mcp = Arc::new(McpHub::new());
    let provider: Arc<dyn Provider> = Arc::new(PreviewProvider);
    let model = model_config(&profile.model)?;
    let resolved = resolve_parts(profile, project_root, Arc::clone(&mcp), provider, model);
    Ok(preview_from_resolved(
        profile,
        project_root,
        &resolved.parts,
        &resolved,
    ))
}

pub(crate) fn preview_from_resolved(
    profile: &AgentProfile,
    project_root: &Path,
    parts: &[CapabilityPart],
    resolved: &crate::capability::ResolvedParts,
) -> AssembledPreview {
    let identity = profile.effective_identity_prompt().to_string();
    let prompt_layers = build_prompt_layers(profile, project_root, parts, &resolved.skill_registry);
    let full_system_prompt = prompt_layers
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut tools = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for part in parts {
        for t in &part.preview_tools {
            if seen.insert(t.name.clone()) {
                tools.push(t.clone());
            }
        }
    }
    if !resolved.skill_registry.is_empty() {
        let name = crate::ops::skill::ACTIVATE_SKILL.to_string();
        if seen.insert(name.clone()) {
            tools.push(PreviewTool {
                name,
                source: "skill".into(),
                executor: "rust".into(),
            });
        }
    }

    let perm = merge_permission_config(profile, parts);
    let mut permission_matrix = Vec::new();
    for t in &tools {
        let effect = perm.resolve(&t.name, profile.mode);
        permission_matrix.push((t.name.clone(), format_effect(effect)));
    }

    let deferred = flatten_deferred(parts);
    let providers = crate::capability::flatten_providers(parts);
    let chat_only = deferred.is_empty()
        && providers.is_empty()
        && resolved.skill_registry.is_empty()
        && profile.project_extensions().is_empty();

    let mut step_names = vec!["prompt_compose".to_string()];
    if profile.max_turns.is_some() {
        step_names.push("max_turns".into());
    }
    step_names.push("compaction".into());
    if chat_only {
        step_names.push("chat_guard".into());
    } else {
        step_names.push("permission".into());
        if !deferred.is_empty() {
            step_names.push("kim_tools".into());
        }
        step_names.push("tools".into());
        if !resolved.skill_registry.is_empty() {
            step_names.push("skills".into());
        }
        step_names.push("unknown_tool".into());
    }
    step_names.push("inference".into());

    AssembledPreview {
        identity,
        prompt_layers,
        full_system_prompt,
        tools,
        permission_matrix,
        warnings: resolved.warnings.clone(),
        step_names,
    }
}

fn format_effect(effect: PermissionDefault) -> String {
    match effect {
        PermissionDefault::AlwaysAllow => "always_allow".into(),
        PermissionDefault::AskBefore => "ask_before".into(),
        PermissionDefault::NeverAllow => "never_allow".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{LegacyOpenOpts, ToolSet};

    #[test]
    fn preview_never_claims_send_message_without_cap() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let preview = preview_assembled(&profile, Path::new("/tmp")).unwrap();
        assert!(preview.tools.iter().any(|t| t.name == "read_file"));
        assert!(!preview.tools.iter().any(|t| t.name == "send_message"));
        assert!(!preview.full_system_prompt.contains("send_message"));
        assert!(!preview.identity.contains("send_message"));
    }

    #[test]
    fn preview_lists_send_message_when_enabled() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.tools = ToolSet {
            send_message: true,
            ..ToolSet::default()
        };
        // Force capability-authoritative path.
        profile.capabilities = crate::capability::CapabilityRef::from_legacy(&profile.tools, &[]);
        let preview = preview_assembled(&profile, Path::new("/tmp")).unwrap();
        assert!(preview.tools.iter().any(|t| t.name == "send_message"));
        let digest = preview
            .prompt_layers
            .iter()
            .find(|(l, _)| l == "capability_digest")
            .map(|(_, t)| t.as_str())
            .unwrap_or("");
        assert!(digest.starts_with("# Tools"), "{digest}");
        assert!(digest.contains("send_message"), "{digest}");
        assert!(!digest.to_ascii_lowercase().contains("gated"), "{digest}");
        assert_eq!(preview.prompt_layers[0].0, "identity");
        assert_eq!(preview.prompt_layers[1].0, "environment");
    }

    #[test]
    fn preview_empty_caps_keep_identity_and_env() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let preview = preview_assembled(&profile, Path::new("/tmp")).unwrap();
        let labels: Vec<&str> = preview
            .prompt_layers
            .iter()
            .map(|(l, _)| l.as_str())
            .collect();
        assert_eq!(labels, vec!["identity", "environment"]);
        assert!(preview.prompt_layers[0].1.contains("# How you work"));
        assert!(preview.prompt_layers[1].1.contains("Date:"));
        assert!(preview.prompt_layers[1].1.contains("Platform:"));
        assert!(!preview.full_system_prompt.contains("send_message"));
    }

    #[test]
    fn preview_agents_md_has_scope_prefix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "project notes").unwrap();
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let preview = preview_assembled(&profile, dir.path()).unwrap();
        let md = preview
            .prompt_layers
            .iter()
            .find(|(l, _)| l == "workspace_agents_md")
            .map(|(_, t)| t.as_str())
            .unwrap_or("");
        assert!(md.contains("nested files closer to a path win"), "{md}");
        assert!(md.contains("current turn override"), "{md}");
        assert!(md.contains("project notes"), "{md}");
        let digest = preview
            .prompt_layers
            .iter()
            .find(|(l, _)| l == "capability_digest")
            .map(|(_, t)| t.as_str())
            .unwrap_or("");
        assert!(digest.starts_with("# Tools"), "{digest}");
        assert!(digest.contains("read_file"), "{digest}");
    }

    #[test]
    fn legacy_profile_still_assembles() {
        let raw = include_str!("../../tests/fixtures/capability_legacy_goose.json");
        let profile: AgentProfile = serde_json::from_str(raw).unwrap();
        let preview = preview_assembled(&profile, Path::new("/tmp")).unwrap();
        assert!(preview.tools.iter().any(|t| t.name == "send_message"));
        assert!(preview.tools.iter().any(|t| t.name == "search_contacts"));
        assert!(!preview.warnings.iter().any(|w| w.contains("unknown")));
    }

    #[test]
    fn leftover_extensions_ignored_when_capabilities_authoritative() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.capabilities = crate::capability::CapabilityRef::from_legacy(&profile.tools, &[]);
        profile.extensions = vec![crate::profile::ExtensionSpec {
            name: "github".into(),
            transport: "stdio".into(),
            command: vec!["npx".into(), "mcp".into()],
            url: String::new(),
            env: Default::default(),
        }];
        assert!(profile.project_extensions().is_empty());
        let preview = preview_assembled(&profile, Path::new("/tmp")).unwrap();
        assert!(!preview.tools.iter().any(|t| t.source.contains("mcp")));
        assert!(!preview.full_system_prompt.contains("github"));
    }

    #[test]
    fn preview_rejects_bad_temperature() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.model.temperature = Some("nope".into());
        let err = preview_assembled(&profile, Path::new("/tmp")).unwrap_err();
        assert!(matches!(err, HostError::Profile(_)));
    }
}
