//! Persisted Agent configuration. This crate does not read the filesystem,
//! start providers, or link Goose, Codex, MCP, or a subprocess.

mod profile;

pub use profile::{
    extensions_from_capabilities, is_legacy_tool_laundry_identity, parse_approval_mode,
    parse_reasoning_effort, ApprovalMode, CapabilityRef, ExtensionSpec, HarnessSpec,
    LegacyOpenOpts, ModelSpec, PermissionConfig, PermissionDefault, PermissionMatch,
    PermissionRule, ProviderSpec, ReasoningChoice, ReasoningChoiceBody, ReasoningEffort,
    RuntimeKind, SandboxMode, SandboxPolicy, SkillClass, SkillRef, ToolSet, WorkspaceKind,
    WorkspaceSpec, LEGACY_TOOL_LAUNDRY_IDENTITY,
};
pub use profile::{AgentProfile, DEFAULT_AGENT_ID, DEFAULT_AGENT_NAME, DEFAULT_IDENTITY_PROMPT};
