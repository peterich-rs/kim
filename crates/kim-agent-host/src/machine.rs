use std::path::Path;
use std::sync::Arc;

use goose_agent::inference::InferenceRunner;
use goose_agent::machine::Step;
use goose_agent::tool::ToolOperation;
use goose_provider_types::base::Provider;
use goose_provider_types::model::ModelConfig;

use crate::events::HostEffect;
use crate::ops::chat_guard::ChatGuardOp;
use crate::ops::fs::FsToolProvider;
use crate::ops::max_turns::MaxTurnsOp;
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
    ) -> Vec<Step<'static, HostSession, HostEffect>> {
        let mut steps = vec![Step::Operation(Arc::new(SystemPromptOp {
            prompt: profile.system_prompt.clone(),
        }))];
        if let Some(max) = profile.max_turns {
            steps.push(Step::Operation(Arc::new(MaxTurnsOp { max })));
        }
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
            let mut tools = ToolOperation::new();
            if profile.tools.fs || profile.tools.fs_write {
                tools = tools.with_provider(Arc::new(FsToolProvider {
                    root: project_root.to_path_buf(),
                    writable: profile.tools.fs_write,
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
    use crate::DEFAULT_SYSTEM_PROMPT;
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
                thinking_effort: None,
                temperature: None,
                max_tokens: None,
            },
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            mode: goose_provider_types::goose_mode::GooseMode::Chat,
            max_turns: Some(16),
            tools,
            permissions: crate::profile::PermissionConfig::default(),
            sandbox: crate::profile::SandboxPolicy::default(),
            extensions: Vec::new(),
            enabled: true,
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
        );
        // system + max_turns + chat_guard + inference
        assert_eq!(steps.len(), 4);
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
        );
        // system + max_turns + tools + unknown + inference
        assert_eq!(steps.len(), 5);
    }
}
