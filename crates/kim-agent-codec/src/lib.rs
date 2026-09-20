//! Thin AgentProfile ↔ `kim.agent.AgentSpec` codec.
//!
//! `kim-agent-host` stays serde-only. `kim-sdk` stores opaque bytes.

use std::collections::HashMap;

use goose_provider_types::goose_mode::GooseMode;
use kim_agent_host::{
    parse_goose_mode, AgentProfile, CapabilityRef, ExtensionSpec, ModelSpec, PermissionConfig,
    PermissionDefault, PermissionMatch, PermissionRule, ProviderSpec, ReasoningChoice,
    ReasoningChoiceBody, SandboxPolicy, SkillClass, SkillRef, ToolSet, WorkspaceKind,
    WorkspaceSpec,
};
use kim_protocol::agent;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("schema_version {0} is newer than {CURRENT_SCHEMA_VERSION}; upgrade the client")]
    UnsupportedSchema(u32),
    #[error("invalid agent spec: {0}")]
    Decode(String),
    #[error("json: {0}")]
    Json(String),
    #[error("profile: {0}")]
    Profile(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Local,
    Cloud,
}

impl Placement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }

    pub fn parse(raw: &str) -> Self {
        if raw.trim().eq_ignore_ascii_case("cloud") {
            Self::Cloud
        } else {
            Self::Local
        }
    }

    fn to_proto(self) -> i32 {
        match self {
            Self::Local => agent::Placement::Local as i32,
            Self::Cloud => agent::Placement::Cloud as i32,
        }
    }

    fn from_proto(raw: i32) -> Self {
        if raw == agent::Placement::Cloud as i32 {
            Self::Cloud
        } else {
            Self::Local
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedSpec {
    pub profile: AgentProfile,
    pub placement: Placement,
    pub updated_at: i64,
    pub schema_version: u32,
    pub account_id: String,
    pub server_account: String,
}

#[derive(Serialize, Deserialize, Default)]
struct ExtraPocket {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    account_id: String,
    #[serde(default)]
    provider: ProviderSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_thinking_effort: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_temperature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_max_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_context_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    model_extra_params: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_reasoning: Option<bool>,
    #[serde(default)]
    tools: ToolSet,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extensions: Vec<ExtensionSpec>,
    #[serde(default)]
    permissions: PermissionConfig,
    #[serde(default)]
    sandbox: SandboxPolicy,
    /// Empty / missing / `"goose"` → Goose. `"codex"` → Codex.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    runtime: String,
}

pub fn encode_spec(
    profile: &AgentProfile,
    updated_at: i64,
    placement: Placement,
) -> Result<Vec<u8>, CodecError> {
    encode_spec_parts(profile, updated_at, placement, "", "")
}

pub fn encode_spec_from_json(
    json: &str,
    updated_at: i64,
    placement: Placement,
) -> Result<Vec<u8>, CodecError> {
    let value: Value = serde_json::from_str(json).map_err(|e| CodecError::Json(e.to_string()))?;
    let profile: AgentProfile =
        serde_json::from_value(value.clone()).map_err(|e| CodecError::Json(e.to_string()))?;
    let account_id = value
        .get("account_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let server_account = value
        .get("server_account")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    encode_spec_parts(
        &profile,
        updated_at,
        placement,
        &account_id,
        &server_account,
    )
}

pub fn decode_spec(bytes: &[u8]) -> Result<DecodedSpec, CodecError> {
    let spec = agent::AgentSpec::decode(bytes).map_err(|e| CodecError::Decode(e.to_string()))?;
    let schema_version = if spec.schema_version == 0 {
        CURRENT_SCHEMA_VERSION
    } else {
        spec.schema_version
    };
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(CodecError::UnsupportedSchema(schema_version));
    }
    profile_from_proto(spec, schema_version)
}

pub fn json_to_blob(json: &str) -> Result<Vec<u8>, CodecError> {
    encode_spec_from_json(json, 0, Placement::Local)
}

pub fn blob_to_json(bytes: &[u8]) -> Result<String, CodecError> {
    let decoded = decode_spec(bytes)?;
    spec_to_json(&decoded)
}

pub fn spec_to_json(decoded: &DecodedSpec) -> Result<String, CodecError> {
    let mut value =
        serde_json::to_value(&decoded.profile).map_err(|e| CodecError::Json(e.to_string()))?;
    if let Value::Object(map) = &mut value {
        if !decoded.account_id.is_empty() {
            map.insert(
                "account_id".into(),
                Value::String(decoded.account_id.clone()),
            );
        }
        if !decoded.server_account.is_empty() {
            map.insert(
                "server_account".into(),
                Value::String(decoded.server_account.clone()),
            );
        }
    }
    serde_json::to_string(&value).map_err(|e| CodecError::Json(e.to_string()))
}

fn encode_spec_parts(
    profile: &AgentProfile,
    updated_at: i64,
    placement: Placement,
    account_id: &str,
    server_account: &str,
) -> Result<Vec<u8>, CodecError> {
    let extra = ExtraPocket {
        account_id: account_id.to_string(),
        provider: profile.provider.clone(),
        model_thinking_effort: profile
            .model
            .thinking_effort
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
        model_temperature: profile.model.temperature.clone(),
        model_max_tokens: profile.model.max_tokens,
        model_context_tokens: profile.model.context_tokens,
        model_extra_params: profile.model.extra_params.clone(),
        model_reasoning: profile.model.reasoning,
        tools: profile.tools.clone(),
        extensions: profile.extensions.clone(),
        permissions: profile.permissions.clone(),
        sandbox: profile.sandbox.clone(),
        runtime: profile.runtime.clone(),
    };
    let extra_json = serde_json::to_vec(&extra).map_err(|e| CodecError::Json(e.to_string()))?;
    let model_ref = agent::ModelRef {
        account_id: account_id.to_string(),
        model: profile.model.name.clone(),
        reasoning: profile.reasoning.as_ref().map(encode_reasoning),
    };
    let spec = agent::AgentSpec {
        schema_version: CURRENT_SCHEMA_VERSION,
        id: profile.id.clone(),
        display_name: profile.display_name.clone(),
        aliases: profile.aliases.clone(),
        enabled: if profile.enabled { None } else { Some(false) },
        placement: placement.to_proto(),
        updated_at,
        system_prompt: profile.system_prompt.clone(),
        server_account: server_account.to_string(),
        model_ref: Some(model_ref),
        capabilities: profile
            .capabilities
            .iter()
            .map(encode_capability)
            .collect::<Result<Vec<_>, _>>()?,
        permission_rules: profile.permission_rules.iter().map(encode_rule).collect(),
        workspace: Some(agent::WorkspaceSpec {
            kind: encode_workspace_kind(profile.workspace.kind),
        }),
        skills: profile.skills.iter().map(encode_skill).collect(),
        portable_denylist: profile.portable_denylist.clone(),
        steer: profile.steer.clone(),
        max_turns: profile.max_turns.unwrap_or(0),
        mode: encode_mode(&profile.mode),
        extra_json,
    };
    Ok(spec.encode_to_vec())
}

fn profile_from_proto(
    spec: agent::AgentSpec,
    schema_version: u32,
) -> Result<DecodedSpec, CodecError> {
    let extra: ExtraPocket = if spec.extra_json.is_empty() {
        ExtraPocket::default()
    } else {
        serde_json::from_slice(&spec.extra_json).map_err(|e| CodecError::Json(e.to_string()))?
    };
    let model_ref = spec.model_ref.unwrap_or_default();
    let account_id = if model_ref.account_id.is_empty() {
        extra.account_id.clone()
    } else {
        model_ref.account_id.clone()
    };
    let reasoning = model_ref.reasoning.as_ref().and_then(decode_reasoning);
    let mut thinking_effort = None;
    if let Some(v) = extra.model_thinking_effort.clone() {
        thinking_effort = serde_json::from_value(v).ok();
    }
    let workspace_kind =
        decode_workspace_kind(spec.workspace.as_ref().map(|w| w.kind).unwrap_or(0));
    let profile = AgentProfile {
        id: spec.id,
        display_name: spec.display_name,
        aliases: spec.aliases,
        provider: extra.provider,
        model: ModelSpec {
            name: model_ref.model,
            thinking_effort,
            temperature: extra.model_temperature,
            max_tokens: extra.model_max_tokens,
            context_tokens: extra.model_context_tokens,
            extra_params: extra.model_extra_params,
            reasoning: extra.model_reasoning,
        },
        reasoning,
        system_prompt: spec.system_prompt,
        mode: decode_mode(spec.mode),
        max_turns: if spec.max_turns == 0 {
            None
        } else {
            Some(spec.max_turns)
        },
        tools: extra.tools,
        capabilities: spec
            .capabilities
            .iter()
            .map(decode_capability)
            .collect::<Result<Vec<_>, _>>()?,
        permission_rules: spec.permission_rules.iter().map(decode_rule).collect(),
        permissions: extra.permissions,
        sandbox: extra.sandbox,
        extensions: extra.extensions,
        enabled: spec.enabled.unwrap_or(true),
        steer: spec.steer,
        workspace: WorkspaceSpec {
            kind: workspace_kind,
            path: String::new(),
        },
        skills: spec.skills.iter().map(decode_skill).collect(),
        portable_denylist: spec.portable_denylist,
        user_agents_skills: String::new(),
        runtime: extra.runtime,
        harness: None,
    };
    Ok(DecodedSpec {
        profile,
        placement: Placement::from_proto(spec.placement),
        updated_at: spec.updated_at,
        schema_version,
        account_id,
        server_account: spec.server_account,
    })
}

fn encode_capability(cap: &CapabilityRef) -> Result<agent::CapabilityRef, CodecError> {
    let params_json = if cap.params.is_null() || cap.params == json!({}) {
        Vec::new()
    } else {
        serde_json::to_vec(&cap.params).map_err(|e| CodecError::Json(e.to_string()))?
    };
    Ok(agent::CapabilityRef {
        kind: cap.kind.clone(),
        id: cap.id.clone(),
        params_json,
        enabled: if cap.enabled { None } else { Some(false) },
    })
}

fn decode_capability(cap: &agent::CapabilityRef) -> Result<CapabilityRef, CodecError> {
    let params = if cap.params_json.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&cap.params_json).map_err(|e| CodecError::Json(e.to_string()))?
    };
    Ok(CapabilityRef {
        kind: cap.kind.clone(),
        id: cap.id.clone(),
        params,
        enabled: cap.enabled.unwrap_or(true),
    })
}

fn encode_skill(skill: &SkillRef) -> agent::SkillRef {
    agent::SkillRef {
        id: skill.id.clone(),
        class: match skill.class {
            SkillClass::App => "app".into(),
            SkillClass::Portable => "portable".into(),
        },
        origin: skill.origin.clone(),
        version: skill.version.clone(),
        enabled: if skill.enabled { None } else { Some(false) },
    }
}

fn decode_skill(skill: &agent::SkillRef) -> SkillRef {
    SkillRef {
        id: skill.id.clone(),
        class: if skill.class.eq_ignore_ascii_case("app") {
            SkillClass::App
        } else {
            SkillClass::Portable
        },
        origin: skill.origin.clone(),
        version: skill.version.clone(),
        enabled: skill.enabled.unwrap_or(true),
    }
}

fn encode_rule(rule: &PermissionRule) -> agent::PermissionRule {
    let r#match = match &rule.r#match {
        PermissionMatch::Tool { name } => {
            Some(agent::permission_rule::Match::ToolName(name.clone()))
        }
        PermissionMatch::FsPath { prefix } => {
            Some(agent::permission_rule::Match::FsPathPrefix(prefix.clone()))
        }
        PermissionMatch::BashArgv { prefix } => Some(
            agent::permission_rule::Match::BashArgvPrefix(prefix.clone()),
        ),
        PermissionMatch::McpTool { extension, name } => Some(
            agent::permission_rule::Match::McpTool(agent::McpToolMatch {
                extension: extension.clone(),
                name: name.clone(),
            }),
        ),
    };
    agent::PermissionRule {
        r#match,
        effect: encode_effect(rule.effect),
    }
}

fn decode_rule(rule: &agent::PermissionRule) -> PermissionRule {
    let r#match = match &rule.r#match {
        Some(agent::permission_rule::Match::ToolName(name)) => {
            PermissionMatch::Tool { name: name.clone() }
        }
        Some(agent::permission_rule::Match::FsPathPrefix(prefix)) => PermissionMatch::FsPath {
            prefix: prefix.clone(),
        },
        Some(agent::permission_rule::Match::BashArgvPrefix(prefix)) => PermissionMatch::BashArgv {
            prefix: prefix.clone(),
        },
        Some(agent::permission_rule::Match::McpTool(m)) => PermissionMatch::McpTool {
            extension: m.extension.clone(),
            name: m.name.clone(),
        },
        None => PermissionMatch::Tool {
            name: String::new(),
        },
    };
    PermissionRule {
        r#match,
        effect: decode_effect(rule.effect),
    }
}

fn encode_effect(effect: PermissionDefault) -> i32 {
    match effect {
        PermissionDefault::AlwaysAllow => agent::PermissionEffect::AlwaysAllow as i32,
        PermissionDefault::AskBefore => agent::PermissionEffect::AskBefore as i32,
        PermissionDefault::NeverAllow => agent::PermissionEffect::NeverAllow as i32,
    }
}

fn decode_effect(raw: i32) -> PermissionDefault {
    if raw == agent::PermissionEffect::NeverAllow as i32 {
        PermissionDefault::NeverAllow
    } else if raw == agent::PermissionEffect::AskBefore as i32 {
        PermissionDefault::AskBefore
    } else {
        PermissionDefault::AlwaysAllow
    }
}

fn encode_workspace_kind(kind: WorkspaceKind) -> i32 {
    match kind {
        WorkspaceKind::Repo => agent::WorkspaceKind::Repo as i32,
        WorkspaceKind::Sandbox => agent::WorkspaceKind::Sandbox as i32,
    }
}

fn decode_workspace_kind(raw: i32) -> WorkspaceKind {
    if raw == agent::WorkspaceKind::Repo as i32 {
        WorkspaceKind::Repo
    } else {
        WorkspaceKind::Sandbox
    }
}

fn encode_mode(mode: &GooseMode) -> i32 {
    let name = serde_json::to_value(mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    match name.as_str() {
        "chat" => agent::GooseMode::Chat as i32,
        "auto" => agent::GooseMode::Auto as i32,
        "approve" => agent::GooseMode::Approve as i32,
        _ => agent::GooseMode::SmartApprove as i32,
    }
}

fn decode_mode(raw: i32) -> GooseMode {
    let name = if raw == agent::GooseMode::Chat as i32 {
        "chat"
    } else if raw == agent::GooseMode::Auto as i32 {
        "auto"
    } else if raw == agent::GooseMode::Approve as i32 {
        "approve"
    } else {
        "smart_approve"
    };
    parse_goose_mode(name).unwrap_or(GooseMode::SmartApprove)
}

fn encode_reasoning(r: &ReasoningChoice) -> agent::ReasoningChoice {
    let mut out = agent::ReasoningChoice {
        v: r.v,
        kind: r.kind_name().to_string(),
        on: None,
        value: None,
        budget: None,
        advanced_json: Vec::new(),
    };
    match &r.body {
        ReasoningChoiceBody::None | ReasoningChoiceBody::AlwaysOn => {}
        ReasoningChoiceBody::Toggle { on } => out.on = Some(*on),
        ReasoningChoiceBody::EffortEnum { value } => out.value = Some(value.clone()),
        ReasoningChoiceBody::BudgetTokens { value } => {
            out.budget = i32::try_from(*value).ok();
            out.kind = "budget".into();
        }
        ReasoningChoiceBody::Advanced { json } => {
            out.advanced_json = serde_json::to_vec(json).unwrap_or_default();
        }
    }
    out
}

fn decode_reasoning(r: &agent::ReasoningChoice) -> Option<ReasoningChoice> {
    let kind = r.kind.trim();
    let body = match kind {
        "" | "none" => ReasoningChoiceBody::None,
        "always_on" => ReasoningChoiceBody::AlwaysOn,
        "toggle" => ReasoningChoiceBody::Toggle {
            on: r.on.unwrap_or(false),
        },
        "effort_enum" => ReasoningChoiceBody::EffortEnum {
            value: r.value.clone().unwrap_or_default(),
        },
        "budget" | "budget_tokens" => ReasoningChoiceBody::BudgetTokens {
            value: r.budget.unwrap_or(0).max(0) as u32,
        },
        "advanced" => {
            let json = if r.advanced_json.is_empty() {
                json!({})
            } else {
                serde_json::from_slice(&r.advanced_json).unwrap_or_else(|_| json!({}))
            };
            ReasoningChoiceBody::Advanced { json }
        }
        _ => return None,
    };
    Some(ReasoningChoice {
        v: if r.v == 0 { 1 } else { r.v },
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::agent;
    use prost::Message;

    fn closed_profile_json() -> &'static str {
        r#"{
            "id": "goose",
            "display_name": "助手",
            "model": {"name": "gpt-4o"},
            "system_prompt": "be concise",
            "mode": "smart_approve",
            "capabilities": [{"kind": "im.send_message", "enabled": true}],
            "skills": [{"id": "kim-im", "class": "app", "origin": "bundled"}]
        }"#
    }

    #[test]
    fn closed_field_roundtrip() {
        let blob = json_to_blob(closed_profile_json()).expect("encode");
        let decoded = decode_spec(&blob).expect("decode");
        assert_eq!(decoded.profile.id, "goose");
        assert_eq!(decoded.profile.system_prompt, "be concise");
        assert_eq!(decoded.profile.capabilities.len(), 1);
        assert_eq!(decoded.profile.capabilities[0].kind, "im.send_message");
        assert_eq!(decoded.profile.skills.len(), 1);
        assert_eq!(decoded.profile.skills[0].id, "kim-im");
        assert_eq!(decoded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(decoded.placement, Placement::Local);
        let again = encode_spec(&decoded.profile, decoded.updated_at, decoded.placement)
            .expect("re-encode");
        let twice = decode_spec(&again).expect("re-decode");
        assert_eq!(twice.profile.id, "goose");
        assert_eq!(twice.profile.capabilities[0].kind, "im.send_message");
        assert_eq!(twice.profile.skills[0].id, "kim-im");
    }

    #[test]
    fn old_json_fixture_decodes_equivalent() {
        let json = r#"{
            "id": "p-1",
            "display_name": "coder",
            "aliases": ["码农"],
            "account_id": "acct-goose",
            "server_account": "b_bot",
            "provider": {"kind": "openai", "base_url": "https://api.openai.com/v1", "key_ref": "agent.api_key.goose"},
            "model": {"name": "gpt-4o"},
            "system_prompt": "ship it",
            "mode": "chat",
            "max_turns": 8,
            "capabilities": [{"kind": "bash", "params": {"timeout": 30}}],
            "skills": [{"id": "kim-im", "class": "app"}],
            "enabled": true
        }"#;
        let blob = json_to_blob(json).expect("encode");
        let decoded = decode_spec(&blob).expect("decode");
        assert_eq!(decoded.profile.id, "p-1");
        assert_eq!(decoded.account_id, "acct-goose");
        assert_eq!(decoded.server_account, "b_bot");
        assert_eq!(decoded.profile.provider.kind, "openai");
        assert_eq!(decoded.profile.model.name, "gpt-4o");
        assert_eq!(decoded.profile.system_prompt, "ship it");
        assert_eq!(decoded.profile.capabilities[0].kind, "bash");
        assert_eq!(decoded.profile.workspace.path, "");
        let back = blob_to_json(&blob).expect("json");
        let map: Value = serde_json::from_str(&back).expect("parse");
        assert_eq!(map["id"], "p-1");
        assert_eq!(map["account_id"], "acct-goose");
        assert_eq!(map["system_prompt"], "ship it");
    }

    #[test]
    fn schema_version_99_refuses() {
        let spec = agent::AgentSpec {
            schema_version: 99,
            id: "goose".into(),
            ..Default::default()
        };
        let bytes = spec.encode_to_vec();
        match decode_spec(&bytes) {
            Err(CodecError::UnsupportedSchema(99)) => {}
            other => panic!("expected UnsupportedSchema(99), got {other:?}"),
        }
    }

    #[test]
    fn unset_enabled_is_true() {
        let spec = agent::AgentSpec {
            schema_version: 1,
            id: "goose".into(),
            enabled: None,
            ..Default::default()
        };
        let decoded = decode_spec(&spec.encode_to_vec()).expect("decode");
        assert!(decoded.profile.enabled);
    }

    #[test]
    fn missing_provider_stays_empty() {
        let blob = json_to_blob(
            r#"{
            "id": "p-1",
            "display_name": "coder",
            "model": {"name": "custom-model"},
            "system_prompt": "hi"
        }"#,
        )
        .expect("encode");
        let decoded = decode_spec(&blob).expect("decode");
        assert!(decoded.profile.provider.kind.is_empty());
        assert_eq!(decoded.profile.model.name, "custom-model");
        assert!(decoded.profile.user_agents_skills.is_empty());
    }

    #[test]
    fn old_json_user_agents_skills_not_in_blob() {
        let blob = json_to_blob(
            r#"{
            "id": "p-1",
            "display_name": "coder",
            "model": {"name": "m"},
            "user_agents_skills": "/Users/alice/.agents/skills"
        }"#,
        )
        .expect("encode");
        let decoded = decode_spec(&blob).expect("decode");
        assert!(decoded.profile.user_agents_skills.is_empty());
        let spec = agent::AgentSpec::decode(blob.as_slice()).expect("proto");
        let extra: Value = serde_json::from_slice(&spec.extra_json).expect("extra");
        assert!(extra.get("user_agents_skills").is_none());
    }
}
