//! Capability blocks: the assembly unit for agent tools + prompt fragments.

mod blocks;
mod legacy;
pub mod preview;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use goose_agent::tool::ToolProvider;
use goose_provider_types::base::Provider;
use goose_provider_types::model::ModelConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ops::mcp::McpHub;
use crate::profile::{
    AgentProfile, ExtensionSpec, PermissionConfig, PermissionDefault, ToolSet, WorkspaceKind,
};
use crate::skills::{
    build_registry, catalog_prompt_block, read_agents_md, RegistryScan, SkillRegistry,
    SkillResolver,
};
use crate::{HostError, HostSession, DEFAULT_AGENT_NAME};

pub use legacy::from_legacy;
pub use preview::{preview_assembled, AssembledPreview, PreviewTool};

pub(crate) fn extensions_from_capabilities(caps: &[CapabilityRef]) -> Vec<ExtensionSpec> {
    legacy::extensions_from_capabilities(caps)
}

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
    #[allow(dead_code)] // Available to block check/build (workspace, identity).
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
    let mut kinds: Vec<&'static str> = registry().keys().copied().collect();
    kinds.sort_unstable();
    kinds
        .into_iter()
        .filter_map(|k| registry().get(k))
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

    ResolvedParts {
        parts,
        warnings,
        skill_registry,
        skill_resolver,
    }
}

const TOOLS_POLICY_PREAMBLE: &str = "\
# Tools

Your tool list for this session is assembled from the blocks below. Tool
parameters are documented in the tool schemas; the notes here are usage policy.";

/// L1 identity → L2 environment → L3 digest → L4 AGENTS.md → L5 skills → L6 steer.
pub(crate) fn build_prompt_layers(
    profile: &AgentProfile,
    project_root: &Path,
    parts: &[CapabilityPart],
    skill_registry: &SkillRegistry,
) -> Vec<(String, String)> {
    let mut layers = Vec::new();

    let identity = interpolate_identity(profile.effective_identity_prompt(), profile);
    if !identity.trim().is_empty() {
        layers.push(("identity".into(), identity));
    }

    let env = environment_block(profile, project_root);
    if !env.trim().is_empty() {
        layers.push(("environment".into(), env));
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
        layers.push((
            "capability_digest".into(),
            format!("{TOOLS_POLICY_PREAMBLE}\n\n{}", digest_bits.join("\n\n")),
        ));
    }

    let projected = profile.project_toolset();
    let scan_portable =
        projected.fs || projected.fs_write || profile.workspace.kind == WorkspaceKind::Repo;
    if scan_portable {
        if let Some(agents_md) = read_agents_md(project_root) {
            layers.push((
                "workspace_agents_md".into(),
                format!(
                    "AGENTS.md applies to this workspace tree; nested files closer to a path win.\n\
                     User instructions in the current turn override it.\n\n{agents_md}"
                ),
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

pub(crate) fn interpolate_identity(text: &str, profile: &AgentProfile) -> String {
    let mut out = text.to_string();
    if out.contains("{display_name}") {
        let name = profile.display_name.trim();
        let name = if name.is_empty() {
            DEFAULT_AGENT_NAME
        } else {
            name
        };
        out = out.replace("{display_name}", name);
    }
    if out.contains("{model_name}") {
        out = out.replace("{model_name}", &model_label(profile));
    }
    out
}

fn model_label(profile: &AgentProfile) -> String {
    let kind = profile.provider.kind.trim();
    let name = profile.model.name.trim();
    match (kind.is_empty(), name.is_empty()) {
        (false, false) => format!("{kind}/{name}"),
        (true, false) => name.to_string(),
        (false, true) => kind.to_string(),
        (true, true) => "unknown".into(),
    }
}

fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    }
}

fn environment_block(profile: &AgentProfile, project_root: &Path) -> String {
    let path = project_root.display();
    let workspace = match profile.workspace.kind {
        WorkspaceKind::Sandbox => {
            format!("{path} (sandbox) — private directory, free to read/write")
        }
        WorkspaceKind::Repo => {
            format!("{path} (repo) — user's project directory, change carefully")
        }
    };
    let date = format_civil_date(SystemTime::now());
    format!(
        "<env>\nWorkspace: {workspace}\nPlatform: {}\nDate: {date}\nModel: {}\n</env>",
        platform_name(),
        model_label(profile)
    )
}

fn format_civil_date(now: SystemTime) -> String {
    let (y, m, d) = ymd_with_offset(now, local_utc_offset_secs());
    format!("{y:04}-{m:02}-{d:02}")
}

fn ymd_with_offset(now: SystemTime, offset_secs: i64) -> (i32, u32, u32) {
    let secs = match now.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    civil_from_days(secs.saturating_add(offset_secs).div_euclid(86_400))
}

fn local_utc_offset_secs() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(detect_local_utc_offset_secs)
}

fn detect_local_utc_offset_secs() -> i64 {
    #[cfg(unix)]
    {
        if let Some(z) = offset_from_date_z() {
            return z;
        }
    }
    0
}

#[cfg(unix)]
fn offset_from_date_z() -> Option<i64> {
    let output = std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_iso_tz_offset(std::str::from_utf8(&output.stdout).ok()?.trim())
}

fn parse_iso_tz_offset(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.len() < 5 {
        return None;
    }
    let bytes = s.as_bytes();
    let sign = match bytes[0] {
        b'+' => 1i64,
        b'-' => -1,
        _ => return None,
    };
    let hours: i64 = std::str::from_utf8(&bytes[1..3]).ok()?.parse().ok()?;
    let mins: i64 = std::str::from_utf8(&bytes[3..5]).ok()?.parse().ok()?;
    if !(0..=14).contains(&hours) || mins > 59 {
        return None;
    }
    Some(sign * (hours * 3600 + mins * 60))
}

/// Civil YYYY-MM-DD from Unix days (Howard Hinnant).
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_097) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y as i32, m, d)
}

pub(crate) fn merge_permission_config(
    profile: &AgentProfile,
    parts: &[CapabilityPart],
) -> PermissionConfig {
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

pub(crate) fn flatten_providers(
    parts: &[CapabilityPart],
) -> Vec<Arc<dyn ToolProvider<HostSession>>> {
    parts.iter().filter_map(|p| p.in_process.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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

    fn layer_named<'a>(layers: &'a [(String, String)], name: &str) -> &'a str {
        layers
            .iter()
            .find(|(label, _)| label == name)
            .map(|(_, text)| text.as_str())
            .unwrap_or("")
    }

    #[test]
    fn default_identity_has_chapters_and_zero_tool_names() {
        for name in [
            "send_message",
            "search_contacts",
            "bash",
            "read_file",
            "write_file",
        ] {
            assert!(
                !DEFAULT_IDENTITY_PROMPT.contains(name),
                "default identity lists {name}"
            );
        }
        assert!(DEFAULT_IDENTITY_PROMPT.contains("# How you work"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("## Confirmations"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("## Communication"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("{display_name}"));
        assert!(DEFAULT_IDENTITY_PROMPT.contains("{model_name}"));
    }

    #[test]
    fn empty_caps_omit_digest_but_keep_identity_and_env() {
        let profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let layers = build_prompt_layers(
            &profile,
            Path::new("/tmp/agent/workspaces/goose"),
            &[],
            &SkillRegistry::default(),
        );
        let labels: Vec<&str> = layers.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, vec!["identity", "environment"]);
        let identity = layer_named(&layers, "identity");
        assert!(identity.contains("助手"), "{identity}");
        assert!(identity.contains("# How you work"), "{identity}");
        assert!(identity.contains("openai/gpt-4o"), "{identity}");
        assert!(!identity.contains("send_message"), "{identity}");
        assert!(!identity.contains("{display_name}"), "{identity}");
        let env = layer_named(&layers, "environment");
        assert!(env.contains("<env>"), "{env}");
        assert!(env.contains("Date:"), "{env}");
        assert!(env.contains("Platform:"), "{env}");
        assert!(env.contains("(sandbox)"), "{env}");
        assert!(env.contains("private directory"), "{env}");
        assert!(env.contains("Model: openai/gpt-4o"), "{env}");
    }

    #[test]
    fn sandbox_vs_repo_env_wording_differs() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        let sandbox = build_prompt_layers(
            &profile,
            Path::new("/tmp/ws"),
            &[],
            &SkillRegistry::default(),
        );
        let sandbox_env = layer_named(&sandbox, "environment");
        assert!(sandbox_env.contains("(sandbox)"), "{sandbox_env}");
        assert!(sandbox_env.contains("free to read/write"), "{sandbox_env}");
        assert!(!sandbox_env.contains("(repo)"), "{sandbox_env}");

        profile.workspace.kind = WorkspaceKind::Repo;
        let repo = build_prompt_layers(
            &profile,
            Path::new("/Users/me/proj"),
            &[],
            &SkillRegistry::default(),
        );
        let repo_env = layer_named(&repo, "environment");
        assert!(repo_env.contains("(repo)"), "{repo_env}");
        assert!(repo_env.contains("change carefully"), "{repo_env}");
        assert!(!repo_env.contains("free to read/write"), "{repo_env}");
        assert!(!repo_env.contains("(sandbox)"), "{repo_env}");
    }

    #[test]
    fn capability_digest_starts_with_tools_policy_and_uses_gated() {
        let part = CapabilityPart {
            kind: "im.send_message".into(),
            instance_id: "im.send_message".into(),
            risk: RiskTier::Write,
            prompt_parts: vec![(
                "capability".into(),
                "## send_message\nSend an IM. Gated: user confirms.".into(),
            )],
            deferred_tool_names: Vec::new(),
            in_process: None,
            permission_defaults: Vec::new(),
            preview_tools: Vec::new(),
        };
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.steer = "Be extra brief.".into();
        let layers = build_prompt_layers(
            &profile,
            Path::new("/tmp"),
            &[part],
            &SkillRegistry::default(),
        );
        let labels: Vec<&str> = layers.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            vec!["identity", "environment", "capability_digest", "steer"]
        );
        let digest = layer_named(&layers, "capability_digest");
        assert!(digest.starts_with("# Tools"), "{digest}");
        assert!(digest.to_ascii_lowercase().contains("gated"), "{digest}");
        assert!(digest.contains("send_message"), "{digest}");
        assert!(!digest.contains("requires user confirmation"), "{digest}");
        assert_eq!(
            layers.last().map(|(l, t)| (l.as_str(), t.as_str())),
            Some(("steer", "Be extra brief."))
        );
    }

    #[test]
    fn custom_identity_without_placeholders_is_verbatim() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.system_prompt = "Stay terse.".into();
        let layers =
            build_prompt_layers(&profile, Path::new("/tmp"), &[], &SkillRegistry::default());
        assert_eq!(layer_named(&layers, "identity"), "Stay terse.");
    }

    #[test]
    fn custom_identity_interpolates_placeholders() {
        let mut profile = AgentProfile::from_legacy(&LegacyOpenOpts {
            model: "gpt-4o".into(),
            llm_backend: "openai".into(),
            ..LegacyOpenOpts::default()
        });
        profile.display_name = "Kim".into();
        profile.system_prompt = "Hello {display_name} using {model_name}.".into();
        let layers =
            build_prompt_layers(&profile, Path::new("/tmp"), &[], &SkillRegistry::default());
        assert_eq!(
            layer_named(&layers, "identity"),
            "Hello Kim using openai/gpt-4o."
        );
    }

    #[test]
    fn civil_from_days_known_unix_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
        assert_eq!(civil_from_days(365), (1971, 1, 1));
        assert_eq!(civil_from_days(11_017), (2000, 3, 1));
        assert_eq!(civil_from_days(20_713), (2026, 9, 17));
        assert_eq!(ymd_with_offset(UNIX_EPOCH, 0), (1970, 1, 1));
        let billion = UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        assert_eq!(ymd_with_offset(billion, 0), (2001, 9, 9));
        let twenty_hours = UNIX_EPOCH + std::time::Duration::from_secs(20 * 3600);
        assert_eq!(ymd_with_offset(twenty_hours, 0), (1970, 1, 1));
        assert_eq!(ymd_with_offset(twenty_hours, 8 * 3600), (1970, 1, 2));
        assert_eq!(parse_iso_tz_offset("+0800"), Some(8 * 3600));
        assert_eq!(parse_iso_tz_offset("-0530"), Some(-(5 * 3600 + 30 * 60)));
        assert_eq!(parse_iso_tz_offset("0800"), None);
    }

    #[test]
    fn catalog_entries_are_sorted_by_kind() {
        let kinds: Vec<String> = catalog_entries()
            .iter()
            .filter_map(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string))
            .collect();
        let mut sorted = kinds.clone();
        sorted.sort();
        assert_eq!(kinds, sorted);
        assert!(kinds.contains(&"fs".to_string()));
        assert!(kinds.contains(&"im.send_message".to_string()));
        let fs = catalog_entries()
            .into_iter()
            .find(|v| v.get("kind").and_then(|k| k.as_str()) == Some("fs"))
            .expect("fs catalog");
        assert_eq!(fs.get("risk").and_then(|r| r.as_str()), Some("read"));
    }
}
