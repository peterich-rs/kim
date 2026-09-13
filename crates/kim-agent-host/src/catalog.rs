//! KIM vendor catalog: capability surfaces, prefix rules, and model mapping.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::LazyLock;

use goose_provider_types::thinking::ThinkingEffort;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::HostError;

const VENDORS_JSON: &str = include_str!("catalog/vendors.json");

static CATALOG: LazyLock<Result<VendorCatalog, String>> =
    LazyLock::new(|| serde_json::from_str(VENDORS_JSON).map_err(|e| e.to_string()));

fn catalog() -> Result<&'static VendorCatalog, HostError> {
    let cat = CATALOG
        .as_ref()
        .map_err(|e| HostError::Failed(format!("vendors.json: {e}")))?;
    if cat.schema_version != 1 {
        tracing::warn!(version = cat.schema_version, "catalog schema_version");
    }
    Ok(cat)
}

#[derive(Debug, Clone, Deserialize)]
pub struct VendorCatalog {
    #[allow(dead_code)]
    pub schema_version: u32,
    pub vendors: Vec<VendorEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VendorEntry {
    pub id: String,
    pub display_name: String,
    pub group: VendorGroup,
    pub sort_rank: u32,
    #[allow(dead_code)]
    pub protocol: VendorProtocol,
    pub default_base_url: String,
    #[serde(default)]
    pub alt_base_urls: Vec<UrlOption>,
    #[allow(dead_code)]
    pub auth: AuthStyle,
    pub builder: BuilderKind,
    #[serde(default)]
    pub goose_fallback_name: Option<String>,
    #[serde(default)]
    pub dynamic_models: bool,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
    #[serde(default)]
    pub custom_model: bool,
    #[serde(default)]
    pub model_rules: Vec<ModelPrefixRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrlOption {
    pub url: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VendorGroup {
    Primary,
    Gateway,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VendorProtocol {
    OpenaiCompat,
    Anthropic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStyle {
    Bearer,
    AnthropicApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuilderKind {
    Openai,
    Anthropic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPrefixRule {
    pub prefix: String,
    pub surface: ReasoningSurface,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub display_name: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub context: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub supports_tools: bool,
    #[serde(default)]
    pub reasoning: ReasoningSurface,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningSurface {
    #[default]
    None,
    AlwaysOn {
        #[serde(default)]
        note: String,
    },
    Toggle {
        #[serde(default)]
        param: String,
        #[serde(default)]
        default_on: bool,
    },
    EffortEnum {
        #[serde(default)]
        param: String,
        #[serde(default)]
        allowed: Vec<String>,
        #[serde(default)]
        default: String,
    },
    BudgetTokens {
        #[serde(default)]
        min: u32,
        #[serde(default)]
        max: u32,
        #[serde(default)]
        default: u32,
    },
}

impl ReasoningSurface {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AlwaysOn { .. } => "always_on",
            Self::Toggle { .. } => "toggle",
            Self::EffortEnum { .. } => "effort_enum",
            Self::BudgetTokens { .. } => "budget_tokens",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VendorSummary {
    pub id: String,
    pub display_name: String,
    pub group: VendorGroup,
    pub sort_rank: u32,
    pub default_base_url: String,
    pub alt_base_urls: Vec<String>,
    pub dynamic_models: bool,
    pub custom_model: bool,
    pub default_model: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningChoice {
    #[serde(default = "choice_version")]
    pub v: u32,
    #[serde(flatten)]
    pub body: ReasoningChoiceBody,
}

fn choice_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReasoningChoiceBody {
    None,
    AlwaysOn,
    Toggle { on: bool },
    EffortEnum { value: String },
    BudgetTokens { value: u32 },
    Advanced { json: Value },
}

impl ReasoningChoice {
    pub fn kind_name(&self) -> &'static str {
        match self.body {
            ReasoningChoiceBody::None => "none",
            ReasoningChoiceBody::AlwaysOn => "always_on",
            ReasoningChoiceBody::Toggle { .. } => "toggle",
            ReasoningChoiceBody::EffortEnum { .. } => "effort_enum",
            ReasoningChoiceBody::BudgetTokens { .. } => "budget_tokens",
            ReasoningChoiceBody::Advanced { .. } => "advanced",
        }
    }
}

/// Closed alias table: stored / Goose / old `llm_backend` → catalog vendor id.
pub fn normalize_vendor_id(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "custom_deepseek" => "deepseek".into(),
        "alibaba" => "qwen".into(),
        "moonshot" => "moonshot".into(),
        "zhipu" => "zhipu".into(),
        "groq" => "groq".into(),
        "xai" | "grok" => "xai".into(),
        "minimax" => "minimax".into(),
        "openai_compatible" => "openai_compatible".into(),
        "openai" | "responses_http" | "live" | "responses" | "" => "openai".into(),
        "anthropic" | "messages" => "anthropic".into(),
        "deepseek" => "deepseek".into(),
        "qwen" => "qwen".into(),
        "siliconflow" => "siliconflow".into(),
        "openrouter" => "openrouter".into(),
        other => other.to_string(),
    }
}

pub fn vendor_entry(id: &str) -> Result<Option<&'static VendorEntry>, HostError> {
    let want = normalize_vendor_id(id);
    Ok(catalog()?
        .vendors
        .iter()
        .find(|v| v.id.eq_ignore_ascii_case(&want)))
}

pub fn default_base_url(kind: &str) -> Result<Option<&'static str>, HostError> {
    Ok(vendor_entry(kind)?.map(|v| v.default_base_url.as_str()))
}

pub fn goose_fallback_name(kind: &str) -> Result<Option<&'static str>, HostError> {
    Ok(vendor_entry(kind)?.and_then(|v| v.goose_fallback_name.as_deref()))
}

pub fn builder_kind(kind: &str) -> Result<Option<BuilderKind>, HostError> {
    Ok(vendor_entry(kind)?.map(|v| v.builder))
}

pub fn catalog_model_ids(kind: &str) -> Result<Vec<String>, HostError> {
    Ok(vendor_entry(kind)?
        .map(|v| v.models.iter().map(|m| m.id.clone()).collect())
        .unwrap_or_default())
}

pub fn vendor_summaries() -> Result<Vec<VendorSummary>, HostError> {
    let mut out: Vec<VendorSummary> = catalog()?
        .vendors
        .iter()
        .map(|v| {
            let default_model = v
                .models
                .iter()
                .find(|m| m.default)
                .or_else(|| v.models.first())
                .map(|m| m.id.clone())
                .unwrap_or_default();
            VendorSummary {
                id: v.id.clone(),
                display_name: v.display_name.clone(),
                group: v.group,
                sort_rank: v.sort_rank,
                default_base_url: v.default_base_url.clone(),
                alt_base_urls: v.alt_base_urls.iter().map(|u| u.url.clone()).collect(),
                dynamic_models: v.dynamic_models,
                custom_model: v.custom_model,
                default_model,
                models: v.models.iter().map(|m| m.id.clone()).collect(),
            }
        })
        .collect();
    out.sort_by_key(|s| s.sort_rank);
    Ok(out)
}

pub fn catalog_vendors_json() -> Result<String, HostError> {
    serde_json::to_string(&vendor_summaries()?).map_err(|e| HostError::Failed(e.to_string()))
}

pub fn surface_for(vendor: &str, model: &str) -> Result<ReasoningSurface, HostError> {
    let vendor = normalize_vendor_id(vendor);
    let model_trim = model.trim();
    if let Some(entry) = vendor_entry(&vendor)? {
        if let Some(found) = entry
            .models
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(model_trim))
        {
            return Ok(found.reasoning.clone());
        }
        if let Some(surface) = first_prefix_match(model_trim, &entry.model_rules) {
            return Ok(surface);
        }
    }
    Ok(family_surface(model_trim))
}

fn first_prefix_match(model: &str, rules: &[ModelPrefixRule]) -> Option<ReasoningSurface> {
    let mut ranked: Vec<&ModelPrefixRule> = rules.iter().collect();
    ranked.sort_by_key(|r| std::cmp::Reverse(r.prefix.len()));
    ranked
        .into_iter()
        .find(|r| prefix_matches(model, &r.prefix))
        .map(|r| r.surface.clone())
}

fn model_basename(model: &str) -> &str {
    model
        .rsplit(['/', ':'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(model)
}

/// OpenCode-style built-in variants: match the model family, not the vendor.
/// `/v1/models` never returns allowed efforts.
fn family_surface(model: &str) -> ReasoningSurface {
    let full = model.trim();
    let base = model_basename(full);
    let mut rules = family_rules();
    rules.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    for id in [full, base] {
        for (prefix, surface) in &rules {
            if prefix_matches(id, prefix) {
                return surface.clone();
            }
        }
    }
    ReasoningSurface::None
}

fn effort(param: &str, allowed: &[&str], default: &str) -> ReasoningSurface {
    ReasoningSurface::EffortEnum {
        param: param.to_string(),
        allowed: allowed.iter().map(|s| (*s).to_string()).collect(),
        default: default.to_string(),
    }
}

fn family_rules() -> Vec<(&'static str, ReasoningSurface)> {
    vec![
        (
            "gpt-5.1-codex",
            effort(
                "reasoning_effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        ),
        (
            "kimi-k2.7-code",
            ReasoningSurface::AlwaysOn {
                note: "Kimi K2.7 Code always thinks".into(),
            },
        ),
        (
            "minimax-m3",
            ReasoningSurface::Toggle {
                param: "thinking".into(),
                default_on: false,
            },
        ),
        (
            "minimax-m2",
            ReasoningSurface::AlwaysOn {
                note: "MiniMax M2 cannot disable thinking".into(),
            },
        ),
        (
            "kimi-k2.6",
            ReasoningSurface::Toggle {
                param: "thinking.type".into(),
                default_on: true,
            },
        ),
        (
            "kimi-k2.5",
            ReasoningSurface::Toggle {
                param: "thinking.type".into(),
                default_on: true,
            },
        ),
        (
            "glm-5.3",
            effort("reasoning_effort", &["low", "high", "max"], "max"),
        ),
        (
            "glm-5.2",
            effort("reasoning_effort", &["low", "high", "max"], "max"),
        ),
        (
            "grok-4.20-multi-agent",
            effort(
                "reasoning_effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        ),
        (
            "grok-4.6",
            effort(
                "reasoning_effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        ),
        (
            "grok-4.5",
            effort("reasoning_effort", &["low", "medium", "high"], "high"),
        ),
        (
            "grok-4.20",
            effort(
                "reasoning_effort",
                &["low", "medium", "high", "xhigh"],
                "high",
            ),
        ),
        (
            "grok-3-mini",
            effort("reasoning_effort", &["low", "high"], "low"),
        ),
        (
            "deepseek-",
            effort(
                "reasoning_effort",
                &["none", "low", "high", "max"],
                "high",
            ),
        ),
        (
            "claude-",
            effort("thinking_effort", &["low", "high", "max"], "high"),
        ),
        (
            "gpt-5",
            effort(
                "reasoning_effort",
                &["none", "minimal", "low", "medium", "high", "xhigh"],
                "medium",
            ),
        ),
        (
            "kimi-k3",
            effort("reasoning_effort", &["low", "high", "max"], "max"),
        ),
        (
            "glm-5",
            effort("reasoning_effort", &["low", "high", "max"], "max"),
        ),
        (
            "grok-4",
            effort("reasoning_effort", &["low", "high"], "low"),
        ),
        (
            "grok-3",
            effort("reasoning_effort", &["low", "high"], "low"),
        ),
        (
            "qwen",
            ReasoningSurface::Toggle {
                param: "enable_thinking".into(),
                default_on: true,
            },
        ),
        (
            "o1",
            effort("reasoning_effort", &["low", "medium", "high"], "medium"),
        ),
        (
            "o3",
            effort("reasoning_effort", &["low", "medium", "high"], "medium"),
        ),
        (
            "o4",
            effort("reasoning_effort", &["low", "medium", "high"], "medium"),
        ),
        (
            "grok",
            effort("reasoning_effort", &["low", "high"], "low"),
        ),
    ]
}

pub fn catalog_surface_json(vendor: &str, model: &str) -> Result<String, HostError> {
    tracing::info!(vendor, model, "catalog.surface");
    serde_json::to_string(&surface_for(vendor, model)?)
        .map_err(|e| HostError::Failed(e.to_string()))
}

/// Fortify then validate. Ok JSON is `{"choice":…,"dropped":[names]}` — names only.
pub fn catalog_validate(vendor: &str, model: &str, choice_json: &str) -> Result<String, HostError> {
    let choice: ReasoningChoice = serde_json::from_str(choice_json)
        .map_err(|e| HostError::Profile(format!("reasoning: {e}")))?;
    let surface = surface_for(vendor, model)?;
    let (choice, dropped) = fortify_choice(&choice);
    if !matches!(choice.body, ReasoningChoiceBody::Advanced { .. }) {
        validate_choice(&surface, &choice)?;
    }
    serde_json::to_string(&json!({
        "choice": choice,
        "dropped": dropped,
    }))
    .map_err(|e| HostError::Failed(e.to_string()))
}

fn validate_choice(surface: &ReasoningSurface, choice: &ReasoningChoice) -> Result<(), HostError> {
    if matches!(choice.body, ReasoningChoiceBody::Advanced { .. }) {
        return Ok(());
    }
    match (surface, &choice.body) {
        (ReasoningSurface::None, ReasoningChoiceBody::None) => Ok(()),
        (ReasoningSurface::AlwaysOn { .. }, ReasoningChoiceBody::AlwaysOn) => Ok(()),
        (ReasoningSurface::Toggle { .. }, ReasoningChoiceBody::Toggle { .. }) => Ok(()),
        (
            ReasoningSurface::EffortEnum { allowed, .. },
            ReasoningChoiceBody::EffortEnum { value },
        ) => {
            if allowed.iter().any(|a| a.eq_ignore_ascii_case(value)) {
                Ok(())
            } else {
                Err(HostError::Profile(format!(
                    "reasoning value {value} is not allowed"
                )))
            }
        }
        (
            ReasoningSurface::BudgetTokens { min, max, .. },
            ReasoningChoiceBody::BudgetTokens { value },
        ) => {
            if value >= min && value <= max {
                Ok(())
            } else {
                Err(HostError::Profile(format!(
                    "budget {value} out of range {min}..={max}"
                )))
            }
        }
        _ => Err(HostError::Profile(format!(
            "reasoning kind {} does not match surface {}",
            choice.kind_name(),
            surface.kind_name()
        ))),
    }
}

fn prefix_matches(model: &str, prefix: &str) -> bool {
    let model = model.to_ascii_lowercase();
    let pat = prefix.to_ascii_lowercase();
    if pat.contains('*') {
        glob_match(&model, &pat)
    } else {
        model.starts_with(&pat)
    }
}

fn glob_match(text: &str, pat: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return text == pat;
    }
    let mut rest = text;
    if !parts[0].is_empty() {
        if !rest.starts_with(parts[0]) {
            return false;
        }
        rest = &rest[parts[0].len()..];
    }
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(idx) => {
                rest = &rest[idx + part.len()..];
                if i == parts.len() - 1 && !pat.ends_with('*') && !rest.is_empty() {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

const ADVANCED_ALLOWLIST: &[&str] = &[
    "thinking",
    "reasoning_effort",
    "enable_thinking",
    "thinking_budget",
    "budget_tokens",
];

pub struct ModelApply {
    pub thinking_effort: Option<ThinkingEffort>,
    pub extra_params: HashMap<String, Value>,
    pub reasoning: Option<bool>,
}

impl ModelApply {
    fn empty() -> Self {
        Self {
            thinking_effort: None,
            extra_params: HashMap::new(),
            reasoning: None,
        }
    }
}

/// Drop illegal advanced keys. Logs key names only — never values.
pub fn fortify_choice(choice: &ReasoningChoice) -> (ReasoningChoice, Vec<String>) {
    let ReasoningChoiceBody::Advanced {
        json: Value::Object(map),
    } = &choice.body
    else {
        return (choice.clone(), Vec::new());
    };
    let mut kept = serde_json::Map::new();
    let mut dropped = Vec::new();
    for (k, v) in map {
        if ADVANCED_ALLOWLIST.contains(&k.as_str()) {
            kept.insert(k.clone(), v.clone());
        } else {
            dropped.push(k.clone());
        }
    }
    if !dropped.is_empty() {
        tracing::warn!(dropped = dropped.join(","), "catalog.fortify");
    }
    (
        ReasoningChoice {
            v: choice.v,
            body: ReasoningChoiceBody::Advanced {
                json: Value::Object(kept),
            },
        },
        dropped,
    )
}

pub fn to_model_spec(
    vendor: &str,
    model: &str,
    choice: &ReasoningChoice,
) -> Result<ModelApply, HostError> {
    let vendor = normalize_vendor_id(vendor);
    let surface = surface_for(&vendor, model)?;
    let (choice, _dropped) = fortify_choice(choice);
    if !matches!(choice.body, ReasoningChoiceBody::Advanced { .. }) {
        validate_choice(&surface, &choice)?;
    }
    let routed = route_vendor(&vendor, model);
    Ok(map_choice(&routed, model, &surface, &choice))
}

fn route_vendor(vendor: &str, model: &str) -> String {
    let m = model.to_ascii_lowercase();
    match vendor {
        "openrouter" => {
            if m.starts_with("qwen/") {
                "qwen".into()
            } else if m.starts_with("moonshotai/") {
                "moonshot".into()
            } else if m.starts_with("deepseek/") {
                "deepseek".into()
            } else {
                vendor.to_string()
            }
        }
        "siliconflow" => {
            if m.contains("qwen") {
                "qwen".into()
            } else if m.contains("deepseek") {
                "deepseek".into()
            } else if m.contains("glm") {
                "zhipu".into()
            } else {
                vendor.to_string()
            }
        }
        other => other.to_string(),
    }
}

fn map_choice(
    vendor: &str,
    model: &str,
    surface: &ReasoningSurface,
    choice: &ReasoningChoice,
) -> ModelApply {
    if let ReasoningChoiceBody::Advanced { json } = &choice.body {
        let mut apply = ModelApply::empty();
        if let Value::Object(map) = json {
            apply.extra_params = map.clone().into_iter().collect();
        }
        return apply;
    }
    match vendor {
        "deepseek" => map_deepseek(choice),
        "qwen" => map_qwen(choice),
        "moonshot" => map_moonshot(surface, choice),
        "zhipu" => map_zhipu(choice),
        "anthropic" => map_anthropic(choice),
        "minimax" => map_minimax(surface, choice),
        "openai" | "openai_compatible" => map_openai(model, choice),
        _ => match surface {
            ReasoningSurface::EffortEnum { .. } => map_generic_effort(choice),
            ReasoningSurface::Toggle { param, .. } => map_generic_toggle(param, choice),
            ReasoningSurface::AlwaysOn { .. } => ModelApply::empty(),
            ReasoningSurface::BudgetTokens { .. } => map_budget(choice),
            ReasoningSurface::None => ModelApply::empty(),
        },
    }
}

fn map_deepseek(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    let value = match &choice.body {
        ReasoningChoiceBody::EffortEnum { value } => value.as_str(),
        ReasoningChoiceBody::None => "none",
        _ => return apply,
    };
    let enabled = !value.eq_ignore_ascii_case("none");
    apply.extra_params.insert(
        "thinking".into(),
        json!({"type": if enabled { "enabled" } else { "disabled" }}),
    );
    apply
        .extra_params
        .insert("reasoning_effort".into(), json!(value));
    apply
}

fn map_qwen(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    match &choice.body {
        ReasoningChoiceBody::Toggle { on } => {
            apply
                .extra_params
                .insert("enable_thinking".into(), json!(on));
        }
        ReasoningChoiceBody::BudgetTokens { value } => {
            apply
                .extra_params
                .insert("enable_thinking".into(), json!(true));
            apply
                .extra_params
                .insert("thinking_budget".into(), json!(value));
        }
        ReasoningChoiceBody::None => {
            apply
                .extra_params
                .insert("enable_thinking".into(), json!(false));
        }
        _ => {}
    }
    apply
}

fn map_moonshot(surface: &ReasoningSurface, choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    match (surface, &choice.body) {
        (ReasoningSurface::AlwaysOn { .. }, _) => {
            apply
                .extra_params
                .insert("thinking".into(), json!({"type": "enabled"}));
        }
        (ReasoningSurface::Toggle { .. }, ReasoningChoiceBody::Toggle { on }) => {
            apply.extra_params.insert(
                "thinking".into(),
                json!({"type": if *on { "enabled" } else { "disabled" }}),
            );
        }
        (_, ReasoningChoiceBody::EffortEnum { value }) => {
            apply
                .extra_params
                .insert("reasoning_effort".into(), json!(value));
        }
        _ => {}
    }
    apply
}

fn map_zhipu(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    if let ReasoningChoiceBody::EffortEnum { value } = &choice.body {
        apply
            .extra_params
            .insert("reasoning_effort".into(), json!(value));
        apply
            .extra_params
            .insert("thinking".into(), json!({"type": "enabled"}));
    }
    apply
}

fn map_anthropic(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    if let ReasoningChoiceBody::EffortEnum { value } = &choice.body {
        apply.thinking_effort = ThinkingEffort::from_str(value).ok().filter(|e| {
            matches!(
                e,
                ThinkingEffort::Low | ThinkingEffort::High | ThinkingEffort::Max
            )
        });
        if apply.thinking_effort.is_some() {
            apply.reasoning = Some(true);
        }
    }
    apply
}

fn map_minimax(surface: &ReasoningSurface, choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    let on = match (surface, &choice.body) {
        (ReasoningSurface::AlwaysOn { .. }, _) => true,
        (_, ReasoningChoiceBody::Toggle { on }) => *on,
        (_, ReasoningChoiceBody::AlwaysOn) => true,
        (_, ReasoningChoiceBody::None) => false,
        _ => !matches!(
            surface,
            ReasoningSurface::Toggle {
                default_on: false,
                ..
            }
        ),
    };
    if on {
        apply.reasoning = Some(true);
        let budget = match &choice.body {
            ReasoningChoiceBody::BudgetTokens { value } => (*value).max(1024),
            _ => 4096,
        };
        apply
            .extra_params
            .insert("budget_tokens".into(), json!(budget.max(1024)));
    } else {
        apply.reasoning = Some(false);
    }
    apply
}

fn map_openai(model: &str, choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    let ReasoningChoiceBody::EffortEnum { value } = &choice.body else {
        return apply;
    };
    let lower = model.to_ascii_lowercase();
    let is_reasoning =
        lower.contains("gpt-5") || lower.starts_with("o1") || lower.starts_with("o3");
    if is_reasoning {
        match value.as_str() {
            "low" | "medium" | "high" | "max" => {
                apply.thinking_effort = ThinkingEffort::from_str(value).ok();
            }
            "minimal" | "none" | "xhigh" => {
                apply
                    .extra_params
                    .insert("reasoning_effort".into(), json!(value));
            }
            _ => {
                apply
                    .extra_params
                    .insert("reasoning_effort".into(), json!(value));
            }
        }
    } else {
        apply
            .extra_params
            .insert("reasoning_effort".into(), json!(value));
    }
    apply
}

fn map_generic_effort(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    if let ReasoningChoiceBody::EffortEnum { value } = &choice.body {
        apply
            .extra_params
            .insert("reasoning_effort".into(), json!(value));
    }
    apply
}

fn map_generic_toggle(param: &str, choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    let ReasoningChoiceBody::Toggle { on } = &choice.body else {
        return apply;
    };
    if param == "thinking.type" || param == "thinking" {
        apply.extra_params.insert(
            "thinking".into(),
            json!({"type": if *on { "enabled" } else { "disabled" }}),
        );
    } else if !param.is_empty() {
        apply.extra_params.insert(param.to_string(), json!(on));
    }
    apply
}

fn map_budget(choice: &ReasoningChoice) -> ModelApply {
    let mut apply = ModelApply::empty();
    if let ReasoningChoiceBody::BudgetTokens { value } = &choice.body {
        apply
            .extra_params
            .insert("budget_tokens".into(), json!(value));
    }
    apply
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_provider_types::conversation::message::Message;
    use goose_provider_types::formats::anthropic::{self, AnthropicFormatOptions};
    use goose_provider_types::formats::openai;
    use goose_provider_types::images::ImageFormat;
    use goose_provider_types::model::ModelConfig;

    fn surface_kind(vendor: &str, model: &str) -> String {
        let raw = catalog_surface_json(vendor, model).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        v["kind"].as_str().unwrap().to_string()
    }

    #[test]
    fn catalog_parses_schema_version_1() {
        let cat = catalog().unwrap();
        assert_eq!(cat.schema_version, 1);
        assert!(cat.vendors.iter().any(|v| v.id == "deepseek"));
        assert!(cat.vendors.iter().any(|v| v.id == "minimax"));
        assert!(cat.vendors.iter().any(|v| v.id == "xai"));
    }

    #[test]
    fn vendor_summaries_include_group_and_sort_rank() {
        let sums = vendor_summaries().unwrap();
        let ds = sums.iter().find(|s| s.id == "deepseek").unwrap();
        assert_eq!(ds.group, VendorGroup::Primary);
        assert_eq!(ds.default_model, "deepseek-flash");
        assert!(ds.models.iter().any(|m| m == "deepseek-flash"));
        let or = sums.iter().find(|s| s.id == "openrouter").unwrap();
        assert_eq!(or.group, VendorGroup::Gateway);
        let ranks: Vec<_> = sums.iter().map(|s| s.sort_rank).collect();
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted);
    }

    #[test]
    fn catalog_surface_exact_and_alias() {
        let raw = catalog_surface_json("custom_deepseek", "deepseek-flash").unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["kind"], "effort_enum");
        let allowed: Vec<&str> = v["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(allowed, ["none", "low", "high", "max"]);
        assert_eq!(v["default"], "high");
        assert_eq!(surface_kind("alibaba", "qwen-plus"), "toggle");
    }

    #[test]
    fn catalog_surface_prefix_goldens() {
        assert_eq!(
            surface_kind("deepseek", "deepseek-new-flash"),
            "effort_enum"
        );
        assert_eq!(surface_kind("anthropic", "claude-opus-4-9"), "effort_enum");
        assert_eq!(
            surface_kind("moonshot", "kimi-k2.7-code-preview"),
            "always_on"
        );
        assert_eq!(surface_kind("moonshot", "kimi-k2.6-nightly"), "toggle");
        assert_eq!(surface_kind("moonshot", "kimi-k3-preview"), "effort_enum");
        assert_eq!(surface_kind("zhipu", "glm-5.3-air"), "effort_enum");
        assert_eq!(surface_kind("minimax", "MiniMax-M3-test"), "toggle");
        assert_eq!(surface_kind("minimax", "MiniMax-M2.1"), "always_on");
        assert_eq!(surface_kind("qwen", "qwen3-coder"), "toggle");
        assert_eq!(surface_kind("openrouter", "qwen/qwen3-32b"), "toggle");
        assert_eq!(
            surface_kind("openrouter", "deepseek/deepseek-flash"),
            "effort_enum"
        );
        assert_eq!(
            surface_kind("openrouter", "moonshotai/kimi-k2.7-code"),
            "always_on"
        );
        assert_eq!(
            surface_kind("openrouter", "moonshotai/kimi-k2.6-nightly"),
            "toggle"
        );
        assert_eq!(
            surface_kind("openrouter", "moonshotai/kimi-k3-preview"),
            "effort_enum"
        );
        assert_eq!(surface_kind("siliconflow", "Qwen/Qwen3-8B"), "toggle");
        assert_eq!(surface_kind("siliconflow", "glm-5.3-air"), "effort_enum");
        assert_eq!(surface_kind("openai_compatible", "anything"), "none");
        assert_eq!(surface_kind("unknown-vendor", "x"), "none");
        assert_eq!(
            surface_kind("openai_compatible", "claude-sonnet-4-5"),
            "effort_enum"
        );
        assert_eq!(surface_kind("openai", "gpt-5.4"), "effort_enum");
        assert_eq!(surface_kind("openai", "o3-mini"), "effort_enum");
        assert_eq!(surface_kind("openai_compatible", "grok-4.5"), "effort_enum");
        assert_eq!(surface_kind("xai", "grok-4.6"), "effort_enum");
        assert_eq!(surface_kind("openai", "gpt-4o"), "none");
    }

    fn surface_allowed(vendor: &str, model: &str) -> Vec<String> {
        let raw = catalog_surface_json(vendor, model).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        v["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn xai_is_primary_and_grok_alias() {
        assert_eq!(normalize_vendor_id("grok"), "xai");
        assert_eq!(normalize_vendor_id("XAI"), "xai");
        assert_eq!(normalize_vendor_id("groq"), "groq");
        let xai = vendor_summaries()
            .unwrap()
            .into_iter()
            .find(|s| s.id == "xai")
            .unwrap();
        assert_eq!(xai.group, VendorGroup::Primary);
        assert_eq!(xai.default_base_url, "https://api.x.ai/v1");
        assert_eq!(xai.default_model, "grok-4.6");
        assert!(xai.models.iter().any(|m| m == "grok-4.5"));
    }

    #[test]
    fn xai_grok_reasoning_effort_matches_docs() {
        assert_eq!(
            surface_allowed("xai", "grok-4.6"),
            ["low", "medium", "high", "xhigh"]
        );
        assert_eq!(
            surface_allowed("xai", "grok-4.6-latest"),
            ["low", "medium", "high", "xhigh"]
        );
        assert_eq!(
            surface_allowed("xai", "grok-4.5"),
            ["low", "medium", "high"]
        );
        assert!(!surface_allowed("xai", "grok-4.5").contains(&"xhigh".into()));
        assert_eq!(
            surface_allowed("openai_compatible", "grok-4.6"),
            ["low", "medium", "high", "xhigh"]
        );
        let raw = catalog_surface_json("grok", "grok-4.6").unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["default"], "high");
    }

    #[test]
    fn claude_surface_is_effort_not_budget() {
        let raw = catalog_surface_json("anthropic", "claude-sonnet-4-5").unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["kind"], "effort_enum");
        let allowed: Vec<&str> = v["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str())
            .collect();
        assert_eq!(allowed, ["low", "high", "max"]);
        assert_ne!(v["kind"], "budget_tokens");
    }

    #[test]
    fn qwen_default_url_is_china_compatible_mode() {
        let url = default_base_url("qwen").unwrap().unwrap();
        assert_eq!(url, "https://dashscope.aliyuncs.com/compatible-mode/v1");
        let moon = default_base_url("moonshot").unwrap().unwrap();
        assert_eq!(moon, "https://api.moonshot.cn/v1");
        assert!(!moon.contains("chat/completions"));
    }

    #[test]
    fn catalog_validate_rejects_bad_effort() {
        let ok = catalog_validate(
            "deepseek",
            "deepseek-flash",
            r#"{"v":1,"kind":"effort_enum","value":"high"}"#,
        );
        assert!(ok.is_ok());
        let bad = catalog_validate(
            "deepseek",
            "deepseek-flash",
            r#"{"v":1,"kind":"effort_enum","value":"medium"}"#,
        );
        assert!(bad.is_err());
    }

    #[test]
    fn catalog_validate_fortifies_advanced_and_returns_dropped_names() {
        let raw = catalog_validate(
            "openai_compatible",
            "local-model",
            r#"{"v":1,"kind":"advanced","json":{"enable_thinking":true,"api_key":"sk-secret"}}"#,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["dropped"], json!(["api_key"]));
        assert_eq!(v["choice"]["json"]["enable_thinking"], true);
        assert!(v["choice"]["json"].get("api_key").is_none());
        assert!(!raw.contains("sk-secret"));
    }

    #[test]
    fn to_model_spec_deepseek_keeps_kind_keys() {
        let choice: ReasoningChoice =
            serde_json::from_str(r#"{"v":1,"kind":"effort_enum","value":"high"}"#).unwrap();
        let apply = to_model_spec("deepseek", "deepseek-flash", &choice).unwrap();
        assert!(apply.thinking_effort.is_none());
        assert_eq!(apply.extra_params["thinking"], json!({"type":"enabled"}));
        assert_eq!(apply.extra_params["reasoning_effort"], json!("high"));
        let off: ReasoningChoice =
            serde_json::from_str(r#"{"v":1,"kind":"effort_enum","value":"none"}"#).unwrap();
        let apply = to_model_spec("deepseek", "deepseek-flash", &off).unwrap();
        assert_eq!(apply.extra_params["thinking"], json!({"type":"disabled"}));
        assert_eq!(apply.extra_params["reasoning_effort"], json!("none"));
    }

    #[test]
    fn to_model_spec_minimax_on_sets_reasoning_and_budget() {
        let choice: ReasoningChoice =
            serde_json::from_str(r#"{"v":1,"kind":"toggle","on":true}"#).unwrap();
        let apply = to_model_spec("minimax", "MiniMax-M3", &choice).unwrap();
        assert_eq!(apply.reasoning, Some(true));
        assert!(apply.thinking_effort.is_none());
        let budget = apply.extra_params["budget_tokens"].as_u64().unwrap();
        assert!(budget >= 1024);
        let off: ReasoningChoice =
            serde_json::from_str(r#"{"v":1,"kind":"toggle","on":false}"#).unwrap();
        let apply = to_model_spec("minimax", "MiniMax-M3", &off).unwrap();
        assert_eq!(apply.reasoning, Some(false));
        assert!(!apply.extra_params.contains_key("budget_tokens"));
    }

    fn cfg_from(vendor: &str, model: &str, choice_json: &str) -> ModelConfig {
        let choice: ReasoningChoice = serde_json::from_str(choice_json).unwrap();
        let apply = to_model_spec(vendor, model, &choice).unwrap();
        let mut cfg = ModelConfig::new(model);
        if let Some(effort) = apply.thinking_effort {
            cfg = cfg.with_thinking_effort(effort);
        }
        if !apply.extra_params.is_empty() {
            cfg = cfg.with_merged_request_params(apply.extra_params);
        }
        cfg.reasoning = apply.reasoning;
        cfg.with_max_tokens(Some(64_000))
    }

    fn openai_body(cfg: &ModelConfig) -> Value {
        openai::create_request(
            cfg,
            "hi",
            &[Message::user().with_text("hi")],
            &[],
            &ImageFormat::OpenAi,
            false,
        )
        .unwrap()
    }

    fn anthropic_body(provider: &str, cfg: &ModelConfig) -> Value {
        anthropic::create_request(
            provider,
            cfg,
            "hi",
            &[Message::user().with_text("hi")],
            &[],
            AnthropicFormatOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn http_deepseek_thinking_object_and_effort() {
        let cfg = cfg_from(
            "deepseek",
            "deepseek-flash",
            r#"{"v":1,"kind":"effort_enum","value":"high"}"#,
        );
        let body = openai_body(&cfg);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("thinking_effort").is_none());
        let cfg = cfg_from(
            "deepseek",
            "deepseek-flash",
            r#"{"v":1,"kind":"effort_enum","value":"none"}"#,
        );
        let body = openai_body(&cfg);
        assert_eq!(body["thinking"]["type"], "disabled");
        assert_eq!(body["reasoning_effort"], "none");
    }

    #[test]
    fn http_qwen_enable_thinking_top_level() {
        let cfg = cfg_from("qwen", "qwen-plus", r#"{"v":1,"kind":"toggle","on":true}"#);
        let body = openai_body(&cfg);
        assert_eq!(body["enable_thinking"], true);
        assert!(body.get("extra_body").is_none());
        let cfg = cfg_from("qwen", "qwen-plus", r#"{"v":1,"kind":"toggle","on":false}"#);
        let body = openai_body(&cfg);
        assert_eq!(body["enable_thinking"], false);
    }

    #[test]
    fn http_claude_adaptive_and_enabled() {
        let cfg = cfg_from(
            "anthropic",
            "claude-sonnet-4-6",
            r#"{"v":1,"kind":"effort_enum","value":"high"}"#,
        );
        let body = anthropic_body("anthropic", &cfg);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["output_config"]["effort"], "high");
        let cfg = cfg_from(
            "anthropic",
            "claude-sonnet-4-5",
            r#"{"v":1,"kind":"effort_enum","value":"high"}"#,
        );
        let body = anthropic_body("anthropic", &cfg);
        assert_eq!(body["thinking"]["type"], "enabled");
        let budget = body["thinking"]["budget_tokens"].as_u64().unwrap();
        assert_eq!(budget, 16_000);
        assert!(body.get("output_config").is_none());
        let custom: ReasoningChoice =
            serde_json::from_str(r#"{"v":1,"kind":"effort_enum","value":"high"}"#).unwrap();
        let apply = to_model_spec("anthropic", "claude-opus-4-9", &custom).unwrap();
        assert_eq!(apply.reasoning, Some(true));
        assert!(apply.thinking_effort.is_some());
        assert!(!apply.extra_params.contains_key("budget_tokens"));
        let cfg = cfg_from(
            "anthropic",
            "claude-opus-4-9",
            r#"{"v":1,"kind":"effort_enum","value":"high"}"#,
        );
        let body = anthropic_body("anthropic", &cfg);
        assert!(body.get("thinking").is_some(), "{body}");
    }

    #[test]
    fn http_minimax_enabled_budget_not_adaptive() {
        let cfg = cfg_from(
            "minimax",
            "MiniMax-M3",
            r#"{"v":1,"kind":"toggle","on":true}"#,
        );
        let body = anthropic_body("minimax", &cfg);
        assert_eq!(body["thinking"]["type"], "enabled");
        let budget = body["thinking"]["budget_tokens"].as_u64().unwrap();
        assert!(budget >= 1024, "{body}");
        assert_ne!(body["thinking"]["type"], "adaptive");
        let cfg = cfg_from(
            "minimax",
            "MiniMax-M3",
            r#"{"v":1,"kind":"toggle","on":false}"#,
        );
        let body = anthropic_body("minimax", &cfg);
        assert!(body.get("thinking").is_none(), "{body}");
    }

    #[test]
    fn old_thinking_effort_still_reaches_openai_reasoning_models() {
        let cfg = ModelConfig::new("gpt-5").with_thinking_effort(ThinkingEffort::High);
        let body = openai_body(&cfg);
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn fortify_drops_illegal_keys_without_values() {
        let choice: ReasoningChoice = serde_json::from_str(
            r#"{"v":1,"kind":"advanced","json":{"enable_thinking":true,"api_key":"secret"}}"#,
        )
        .unwrap();
        let (kept, dropped) = fortify_choice(&choice);
        assert_eq!(dropped, ["api_key"]);
        match kept.body {
            ReasoningChoiceBody::Advanced { json } => {
                assert!(json.get("enable_thinking").is_some());
                assert!(json.get("api_key").is_none());
            }
            _ => panic!("expected advanced"),
        }
    }
}
