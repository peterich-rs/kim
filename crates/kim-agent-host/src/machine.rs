use std::path::Path;
use std::sync::Arc;

use goose_agent::inference::InferenceRunner;
use goose_agent::machine::Step;
use goose_agent::tool::ToolOperation;
use goose_provider_types::base::Provider;
use goose_provider_types::model::ModelConfig;

use crate::events::HostEffect;
use crate::ops::bash::BashToolProvider;
use crate::ops::chat_guard::ChatGuardOp;
use crate::ops::compaction::CompactionOp;
use crate::ops::fs::FsToolProvider;
use crate::ops::max_turns::MaxTurnsOp;
use crate::ops::mcp::{McpHub, McpToolProvider};
use crate::ops::permission::PermissionOp;
use crate::ops::skill::SkillOp;
use crate::ops::steer::SteerOp;
use crate::ops::subagent::SubagentOp;
use crate::ops::system_prompt::SystemPromptOp;
use crate::ops::unknown_tool::UnknownToolOp;
use crate::profile::{AgentProfile, ModelSpec, WorkspaceKind};
use crate::skills::{
    build_registry, catalog_prompt_block, read_agents_md, RegistryScan, SkillRegistry,
    SkillResolver,
};
use crate::{HostError, HostSession};

pub struct MachineFactory;

impl MachineFactory {
    pub(crate) fn assemble(
        profile: &AgentProfile,
        provider: Arc<dyn Provider>,
        model: ModelConfig,
        project_root: &Path,
        mcp: Arc<McpHub>,
    ) -> Vec<Step<'static, HostSession, HostEffect>> {
        let mut prompt = profile.effective_system_prompt().to_string();
        if profile.system_prompt.trim().is_empty() {
            tracing::debug!(profile_id = %profile.id, "system_prompt_fallback");
        }

        // S-KD 3: only profiles that can read the workspace inherit the
        // ecosystem shelves. A sandbox IM-only persona must not see
        // `git-commit` in its catalog.
        let scan_portable = profile.tools.fs
            || profile.tools.fs_write
            || profile.workspace.kind == WorkspaceKind::Repo;
        let resolver = Arc::new(SkillResolver::new());
        let registry = Arc::new(if scan_portable || !profile.skills.is_empty() {
            if let Some(agents_md) = read_agents_md(project_root) {
                prompt.push_str("\n\nWorkspace AGENTS.md:\n");
                prompt.push_str(&agents_md);
            }
            build_registry(
                &profile.skills,
                &profile.portable_denylist,
                &RegistryScan {
                    user_root: &profile.user_agents_skills,
                    project_root,
                    enabled: scan_portable,
                },
                &resolver,
            )
        } else {
            SkillRegistry::default()
        });
        // S-KD 13: the catalog goes in, skill bodies never do.
        let catalog = catalog_prompt_block(&registry);
        if !catalog.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str(&catalog);
        }
        tracing::debug!(
            profile_id = %profile.id,
            workspace_kind = ?profile.workspace.kind,
            skills_n = registry.len(),
            "assembled skill catalog"
        );

        let mut steps = vec![Step::Operation(Arc::new(SystemPromptOp { prompt }))];
        if !profile.steer.trim().is_empty() {
            steps.push(Step::Operation(Arc::new(SteerOp {
                steer: profile.steer.clone(),
            })));
        }
        if let Some(max) = profile.max_turns {
            steps.push(Step::Operation(Arc::new(MaxTurnsOp { max })));
        }
        steps.push(Step::Operation(Arc::new(CompactionOp {
            provider: Arc::clone(&provider),
            model: model.model_name.clone(),
        })));
        if profile.mode == goose_provider_types::goose_mode::GooseMode::Chat
            && profile.tools.has_any()
        {
            tracing::warn!(
                profile_id = %profile.id,
                "GooseMode::Chat ignored because ToolSet is non-empty"
            );
        }
        // A catalogued skill needs `activate_skill`, so it also needs the tool
        // pipeline even when no other tool is on.
        let chat_only =
            !profile.tools.has_any() && profile.extensions.is_empty() && registry.is_empty();
        if !chat_only {
            steps.push(Step::Operation(Arc::new(PermissionOp {
                mode: profile.mode,
                config: profile.permissions.clone(),
            })));
            let kim = profile.tools.kim_world_names();
            if !kim.is_empty() {
                steps.push(Step::Operation(Arc::new(
                    crate::ops::deferred_kim::DeferredKimToolOp {
                        names: kim.into_iter().map(str::to_string).collect(),
                    },
                )));
            }
            let mut tools = ToolOperation::new();
            if profile.tools.fs || profile.tools.fs_write {
                tools = tools.with_provider(Arc::new(FsToolProvider {
                    root: project_root.to_path_buf(),
                    writable: profile.tools.fs_write,
                }));
            }
            if profile.tools.bash {
                tools = tools.with_provider(Arc::new(BashToolProvider {
                    root: project_root.to_path_buf(),
                }));
            }
            if !profile.extensions.is_empty() {
                tools = tools.with_provider(Arc::new(McpToolProvider { hub: mcp }));
            }
            if profile.tools.subagent {
                tools = tools.with_provider(Arc::new(SubagentOp {
                    provider: Arc::clone(&provider),
                    model: model.clone(),
                }));
            }
            steps.push(Step::Operation(Arc::new(tools)));
            if !registry.is_empty() {
                steps.push(Step::Operation(Arc::new(SkillOp::new(
                    Arc::clone(&registry),
                    Arc::clone(&resolver),
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
    use crate::{HostSession, DEFAULT_SYSTEM_PROMPT};
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
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            mode: goose_provider_types::goose_mode::GooseMode::Chat,
            max_turns: Some(16),
            tools,
            permissions: crate::profile::PermissionConfig::default(),
            sandbox: crate::profile::SandboxPolicy::default(),
            extensions: Vec::new(),
            enabled: true,
            steer: String::new(),
            workspace: crate::profile::WorkspaceSpec::default(),
            skills: Vec::new(),
            portable_denylist: Vec::new(),
            user_agents_skills: String::new(),
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
        // system + max_turns + compaction + chat_guard + inference
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
        // system + max_turns + compaction + permission + tools + unknown + inference
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
            panic!("expected system prompt operation");
        };
        assert_eq!(op.name(), "system_prompt");
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
        assert_eq!(parts[0].1, DEFAULT_SYSTEM_PROMPT);
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
            panic!("expected system prompt operation");
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
        assert_eq!(parts[0].1, DEFAULT_SYSTEM_PROMPT);
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
            panic!("expected system prompt operation");
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

    async fn system_prompt(steps: &[Step<'static, HostSession, HostEffect>]) -> String {
        let Step::Operation(op) = &steps[0] else {
            panic!("expected system prompt operation");
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
        parts[0].1.clone()
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
        let prompt = system_prompt(&steps).await;
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
        let prompt = system_prompt(&steps).await;
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
        let prompt = system_prompt(&steps).await;
        assert!(!prompt.contains("git-commit"), "{prompt}");
        assert!(!prompt.contains("should stay unread"), "{prompt}");
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
        // system + max_turns + compaction + permission + deferred + tools + unknown + inference
        assert_eq!(steps.len(), 8);
    }
}
