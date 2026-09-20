use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use goose_agent::inference::InferenceRunner;
use goose_agent::machine::Step;
use goose_agent::tool::ToolOperation;
use goose_provider_types::base::Provider;
use goose_provider_types::model::ModelConfig;

use crate::capability::{
    build_prompt_layers, flatten_deferred, flatten_providers, merge_permission_config,
    resolve_parts,
};
use crate::events::HostEffect;
use crate::ops::chat_guard::ChatGuardOp;
use crate::ops::compaction::CompactionOp;
use crate::ops::max_turns::MaxTurnsOp;
use crate::ops::mcp::McpHub;
use crate::ops::permission::PermissionOp;
use crate::ops::prompt_compose::PromptComposeOp;
use crate::ops::skill::SkillOp;
use crate::ops::unknown_tool::UnknownToolOp;
use crate::profile::{AgentProfile, ModelSpec};
use crate::{HostError, HostSession};

pub struct MachineFactory;

impl MachineFactory {
    #[cfg(test)]
    pub(crate) fn assemble(
        profile: &AgentProfile,
        provider: Arc<dyn Provider>,
        model: ModelConfig,
        project_root: &Path,
        mcp: Arc<McpHub>,
    ) -> Vec<Step<'static, HostSession, HostEffect>> {
        Self::assemble_tracked(
            profile,
            provider,
            model,
            project_root,
            mcp,
            Arc::new(AtomicU64::new(0)),
        )
    }

    pub(crate) fn assemble_tracked(
        profile: &AgentProfile,
        provider: Arc<dyn Provider>,
        model: ModelConfig,
        project_root: &Path,
        mcp: Arc<McpHub>,
        input_tokens: Arc<AtomicU64>,
    ) -> Vec<Step<'static, HostSession, HostEffect>> {
        if profile.system_prompt.trim().is_empty() {
            tracing::debug!(profile_id = %profile.id, "system_prompt_fallback");
        }

        let resolved = resolve_parts(
            profile,
            project_root,
            Arc::clone(&mcp),
            Arc::clone(&provider),
            model.clone(),
        );
        let layers = build_prompt_layers(
            profile,
            project_root,
            &resolved.parts,
            &resolved.skill_registry,
        );
        for warning in &resolved.warnings {
            tracing::warn!(profile_id = %profile.id, %warning, "capability resolve warning");
        }
        tracing::debug!(
            profile_id = %profile.id,
            workspace_kind = ?profile.workspace.kind,
            skills_n = resolved.skill_registry.len(),
            caps_n = resolved.parts.len(),
            "assembled skill catalog"
        );

        let mut steps = vec![Step::Operation(Arc::new(PromptComposeOp { layers }))];
        if let Some(max) = profile.max_turns {
            steps.push(Step::Operation(Arc::new(MaxTurnsOp { max })));
        }
        steps.push(Step::Operation(Arc::new(CompactionOp {
            provider: Arc::clone(&provider),
            model: model.clone(),
            tool_result_bytes: 50 * 1024,
            input_tokens,
        })));

        let projected = profile.project_toolset();
        if profile.mode == goose_provider_types::goose_mode::GooseMode::Chat && projected.has_any()
        {
            tracing::warn!(
                profile_id = %profile.id,
                "GooseMode::Chat ignored because projected ToolSet is non-empty"
            );
        }

        let deferred = flatten_deferred(&resolved.parts);
        let providers = flatten_providers(&resolved.parts);
        let chat_only = deferred.is_empty()
            && providers.is_empty()
            && resolved.skill_registry.is_empty()
            && profile.project_extensions().is_empty();

        if !chat_only {
            let config = merge_permission_config(profile, &resolved.parts);
            steps.push(Step::Operation(Arc::new(PermissionOp {
                mode: profile.mode,
                config,
            })));
            if !deferred.is_empty() {
                steps.push(Step::Operation(Arc::new(
                    crate::ops::deferred_kim::DeferredKimToolOp { names: deferred },
                )));
            }
            let mut tools = ToolOperation::new();
            for provider in providers {
                tools = tools.with_provider(provider);
            }
            steps.push(Step::Operation(Arc::new(tools)));
            if !resolved.skill_registry.is_empty() {
                steps.push(Step::Operation(Arc::new(SkillOp::new(
                    Arc::clone(&resolved.skill_registry),
                    Arc::clone(&resolved.skill_resolver),
                ))));
            }
            steps.push(Step::Operation(Arc::new(UnknownToolOp)));
        } else {
            steps.push(Step::Operation(Arc::new(ChatGuardOp)));
        }
        steps.push(Step::Inference(Arc::new(InferenceRunner::new(
            provider, model,
        ))));
        steps
    }
}

pub fn model_config(spec: &ModelSpec) -> Result<ModelConfig, HostError> {
    let name = if spec.name.is_empty() || spec.name == "scripted" {
        "gpt-4o"
    } else {
        spec.name.as_str()
    };
    let mut cfg = ModelConfig::new(name);
    if let Some(effort) = spec.thinking_effort {
        cfg = cfg.with_thinking_effort(effort);
    }
    if !spec.extra_params.is_empty() {
        cfg = cfg.with_merged_request_params(spec.extra_params.clone());
    }
    if spec.reasoning.is_some() {
        cfg.reasoning = spec.reasoning;
    }
    if let Some(max) = spec.max_tokens {
        cfg = cfg.with_max_tokens(Some(max));
    }
    let context = spec
        .context_tokens
        .filter(|n| *n > 0)
        .unwrap_or_else(|| crate::catalog::default_context_tokens(name));
    cfg = cfg.with_context_limit(Some(context as usize));
    if let Some(raw) = spec.temperature.as_deref() {
        let t: f32 = raw
            .parse()
            .map_err(|_| HostError::Profile("bad temperature".into()))?;
        cfg = cfg.with_temperature(Some(t));
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{LegacyOpenOpts, ProviderSpec, ToolSet};
    use crate::{HostSession, DEFAULT_IDENTITY_PROMPT};
    use async_trait::async_trait;
    use goose_provider_types::base::{MessageStream, Provider};
    use goose_provider_types::conversation::message::Message;
    use goose_provider_types::errors::ProviderError;
    use rmcp::model::Tool;

    struct DummyProvider;

    #[async_trait]
    impl Provider for DummyProvider {
        fn get_name(&self) -> &str {
            "dummy"
        }

        async fn stream(
            &self,
            _model_config: &ModelConfig,
            _system: &str,
            _messages: &[Message],
            _tools: &[Tool],
        ) -> Result<MessageStream, ProviderError> {
            Err(ProviderError::ExecutionError("dummy".into()))
        }
    }

    fn profile_with(tools: ToolSet) -> AgentProfile {
        AgentProfile {
            id: "goose".into(),
            display_name: "助手".into(),
            aliases: Vec::new(),
            provider: ProviderSpec {
                kind: "openai".into(),
                base_url: String::new(),
                key_ref: "agent.api_key.goose".into(),
            },
            model: ModelSpec {
                name: "gpt-4o".into(),
                ..Default::default()
            },
            reasoning: None,
            system_prompt: DEFAULT_IDENTITY_PROMPT.into(),
            mode: goose_provider_types::goose_mode::GooseMode::Chat,
            max_turns: Some(16),
            tools,
            capabilities: Vec::new(),
            permission_rules: Vec::new(),
            permissions: crate::profile::PermissionConfig::default(),
            sandbox: crate::profile::SandboxPolicy::default(),
            extensions: Vec::new(),
            enabled: true,
            steer: String::new(),
            workspace: crate::profile::WorkspaceSpec::default(),
            skills: Vec::new(),
            portable_denylist: Vec::new(),
            user_agents_skills: String::new(),
            harness: None,
        }
    }

    #[test]
    fn empty_tools_is_two_steps() {
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile_with(ToolSet::default()),
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        // prompt_compose + max_turns + compaction + chat_guard + inference
        assert_eq!(steps.len(), 5);
    }

    #[test]
    fn from_legacy_fs_adds_tool_operation() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            ..LegacyOpenOpts::default()
        });
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        // prompt_compose + max_turns + compaction + permission + tools + unknown + inference
        assert_eq!(steps.len(), 7);
    }

    #[tokio::test]
    async fn empty_system_prompt_assembles_default() {
        let mut profile = profile_with(ToolSet::default());
        profile.system_prompt = String::new();
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        let Step::Operation(op) = &steps[0] else {
            panic!("expected prompt compose operation");
        };
        assert_eq!(op.name(), "prompt_compose");
        let session = HostSession {
            id: "t".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let parts = op
            .prompt_parts(
                &session,
                &goose_provider_types::conversation::Conversation::empty(),
            )
            .await
            .unwrap();
        assert!(parts[0].1.contains("助手"), "{}", parts[0].1);
        assert!(parts[0].1.contains("# How you work"), "{}", parts[0].1);
        assert!(parts[0].1.contains("## Confirmations"), "{}", parts[0].1);
        assert!(!parts[0].1.contains("send_message"));
        assert!(
            parts.iter().any(|(_, t)| t.contains("<env>")
                && t.contains("Date:")
                && t.contains("Platform:")),
            "{parts:?}"
        );
        assert!(!parts.iter().any(|(_, t)| t.contains("send_message")));
    }

    #[tokio::test]
    async fn missing_system_prompt_json_assembles_default() {
        let json = r#"{
            "id": "goose",
            "display_name": "助手",
            "model": {"name": "gpt-4o"}
        }"#;
        let profile: AgentProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.system_prompt, "");
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        let Step::Operation(op) = &steps[0] else {
            panic!("expected prompt compose operation");
        };
        let session = HostSession {
            id: "t".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let parts = op
            .prompt_parts(
                &session,
                &goose_provider_types::conversation::Conversation::empty(),
            )
            .await
            .unwrap();
        assert!(parts[0].1.contains("助手"), "{}", parts[0].1);
        assert!(parts[0].1.contains("# How you work"), "{}", parts[0].1);
        assert!(!parts[0].1.contains("send_message"));
    }

    #[tokio::test]
    async fn non_empty_system_prompt_is_not_overwritten() {
        let mut profile = profile_with(ToolSet::default());
        profile.system_prompt = "Stay terse.".into();
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        let Step::Operation(op) = &steps[0] else {
            panic!("expected prompt compose operation");
        };
        let session = HostSession {
            id: "t".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let parts = op
            .prompt_parts(
                &session,
                &goose_provider_types::conversation::Conversation::empty(),
            )
            .await
            .unwrap();
        assert_eq!(parts[0].1, "Stay terse.");
    }

    fn step_names(steps: &[Step<'static, HostSession, HostEffect>]) -> Vec<&'static str> {
        steps
            .iter()
            .filter_map(|step| match step {
                Step::Operation(op) => Some(op.name()),
                _ => None,
            })
            .collect()
    }

    async fn assembled_prompt(steps: &[Step<'static, HostSession, HostEffect>]) -> String {
        let Step::Operation(op) = &steps[0] else {
            panic!("expected prompt compose operation");
        };
        let session = HostSession {
            id: "t".into(),
            conversation: goose_provider_types::conversation::Conversation::empty(),
        };
        let parts = op
            .prompt_parts(
                &session,
                &goose_provider_types::conversation::Conversation::empty(),
            )
            .await
            .unwrap();
        parts
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn write_portable_skill(project_root: &Path, id: &str) {
        let dir = project_root.join(".agents").join("skills").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {id}\ndescription: Create Conventional Commits\n---\n\nbody\n"),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn assigned_app_skill_adds_skill_op_and_catalog() {
        let mut profile = profile_with(ToolSet::default());
        profile.skills = vec![crate::skills::SkillRef {
            id: "kim-im".into(),
            ..crate::skills::SkillRef::default()
        }];
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        let names = step_names(&steps);
        assert!(names.contains(&"skills"), "{names:?}");
        assert!(!names.contains(&"chat_guard"), "{names:?}");
        assert_eq!(
            names.iter().position(|n| *n == "skills"),
            names
                .iter()
                .position(|n| *n == "unknown_tool")
                .map(|i| i - 1),
            "skills runs before unknown_tool: {names:?}"
        );
        let prompt = assembled_prompt(&steps).await;
        assert!(prompt.contains("KIM app skills"), "{prompt}");
        assert!(prompt.contains("- kim-im: "), "{prompt}");
        assert!(!prompt.contains("Project/user skills"), "{prompt}");
    }

    #[tokio::test]
    async fn fs_profile_discovers_project_portable_skills() {
        let dir = tempfile::tempdir().unwrap();
        write_portable_skill(dir.path(), "git-commit");
        std::fs::write(dir.path().join("AGENTS.md"), "notes live in MEMORY.md").unwrap();
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_fs_tools: true,
            ..LegacyOpenOpts::default()
        });
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            dir.path(),
            Arc::new(McpHub::new()),
        );
        assert!(step_names(&steps).contains(&"skills"));
        let prompt = assembled_prompt(&steps).await;
        assert!(
            prompt.contains("- git-commit: Create Conventional Commits"),
            "{prompt}"
        );
        assert!(prompt.contains("notes live in MEMORY.md"), "{prompt}");
    }

    #[tokio::test]
    async fn sandbox_im_only_profile_never_scans_portable_shelves() {
        let dir = tempfile::tempdir().unwrap();
        write_portable_skill(dir.path(), "git-commit");
        std::fs::write(dir.path().join("AGENTS.md"), "should stay unread").unwrap();
        let profile = profile_with(ToolSet {
            send_message: true,
            ..ToolSet::default()
        });
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            dir.path(),
            Arc::new(McpHub::new()),
        );
        assert!(!step_names(&steps).contains(&"skills"));
        let prompt = assembled_prompt(&steps).await;
        assert!(!prompt.contains("git-commit"), "{prompt}");
        assert!(!prompt.contains("should stay unread"), "{prompt}");
        assert!(prompt.contains("send_message"), "{prompt}");
    }

    #[test]
    fn kim_tools_include_permission_and_deferred() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            enable_kim_tools: true,
            enable_approvals: true,
            ..LegacyOpenOpts::default()
        });
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        // prompt_compose + max_turns + compaction + permission + deferred + tools + unknown + inference
        assert_eq!(steps.len(), 8);
    }

    #[tokio::test]
    async fn digest_omits_send_message_when_disabled() {
        let profile = profile_with(ToolSet {
            search_contacts: true,
            ..ToolSet::default()
        });
        let provider: Arc<dyn Provider> = Arc::new(DummyProvider);
        let steps = MachineFactory::assemble(
            &profile,
            provider,
            ModelConfig::new("gpt-4o"),
            Path::new("/tmp"),
            Arc::new(McpHub::new()),
        );
        let prompt = assembled_prompt(&steps).await;
        assert!(prompt.contains("search_contacts"), "{prompt}");
        assert!(!prompt.contains("send_message"), "{prompt}");
    }

    #[test]
    fn model_config_applies_context_tokens_or_published_default() {
        let mut spec = ModelSpec {
            name: "gpt-4o".into(),
            ..Default::default()
        };
        let cfg = model_config(&spec).unwrap();
        assert_eq!(cfg.context_limit, Some(128_000));
        spec.context_tokens = Some(64_000);
        let cfg = model_config(&spec).unwrap();
        assert_eq!(cfg.context_limit, Some(64_000));
        spec.name = "local-mystery".into();
        spec.context_tokens = None;
        let cfg = model_config(&spec).unwrap();
        assert_eq!(cfg.context_limit, Some(256_000));
    }
}
