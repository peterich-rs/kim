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
use crate::ops::steer::SteerOp;
use crate::ops::subagent::SubagentOp;
use crate::ops::system_prompt::SystemPromptOp;
use crate::ops::unknown_tool::UnknownToolOp;
use crate::profile::{AgentProfile, ModelSpec};
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
        let prompt = profile.effective_system_prompt().to_string();
        if profile.system_prompt.trim().is_empty() {
            tracing::debug!(profile_id = %profile.id, "system_prompt_fallback");
        }
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
        let chat_only = !profile.tools.has_any() && profile.extensions.is_empty();
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
