//! Structured catalog and skill rows. The host does not return JSON here.

use std::path::Path;

use kim_agent_host::{
    catalog_entries, catalog_validate as host_validate, skill_app_catalog as host_app,
    skill_portable_list as host_portable, surface_for, vendor_summaries, ListedSkill,
    ReasoningChoice, ReasoningChoiceBody, ReasoningSurface as HostReasoningSurface, VendorGroup,
    VendorSummary,
};

use super::failure::AgentFailure;

#[derive(Clone)]
pub struct Vendor {
    pub id: String,
    pub display_name: String,
    pub group: String,
    pub sort_rank: u32,
    pub default_base_url: String,
    pub alt_base_urls: Vec<String>,
    pub dynamic_models: bool,
    pub custom_model: bool,
    pub default_model: String,
    pub models: Vec<String>,
}

impl From<VendorSummary> for Vendor {
    fn from(row: VendorSummary) -> Self {
        Self {
            id: row.id,
            display_name: row.display_name,
            group: match row.group {
                VendorGroup::Primary => "primary",
                VendorGroup::Gateway => "gateway",
                VendorGroup::Other => "other",
            }
            .into(),
            sort_rank: row.sort_rank,
            default_base_url: row.default_base_url,
            alt_base_urls: row.alt_base_urls,
            dynamic_models: row.dynamic_models,
            custom_model: row.custom_model,
            default_model: row.default_model,
            models: row.models,
        }
    }
}

#[derive(Clone)]
pub struct ReasoningSurface {
    pub kind: String,
    pub note: String,
    pub param: String,
    pub default_on: bool,
    pub allowed: Vec<String>,
    pub default_value: String,
    pub min: u32,
    pub max: u32,
    pub default_budget: u32,
}

impl From<HostReasoningSurface> for ReasoningSurface {
    fn from(surface: HostReasoningSurface) -> Self {
        match surface {
            HostReasoningSurface::None => Self::empty("none"),
            HostReasoningSurface::AlwaysOn { note } => Self {
                note,
                ..Self::empty("always_on")
            },
            HostReasoningSurface::Toggle { param, default_on } => Self {
                param,
                default_on,
                ..Self::empty("toggle")
            },
            HostReasoningSurface::EffortEnum {
                param,
                allowed,
                default,
            } => Self {
                param,
                allowed,
                default_value: default,
                ..Self::empty("effort_enum")
            },
            HostReasoningSurface::BudgetTokens { min, max, default } => Self {
                min,
                max,
                default_budget: default,
                ..Self::empty("budget_tokens")
            },
        }
    }
}

impl ReasoningSurface {
    fn empty(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            note: String::new(),
            param: String::new(),
            default_on: false,
            allowed: Vec::new(),
            default_value: String::new(),
            min: 0,
            max: 0,
            default_budget: 0,
        }
    }
}

#[derive(Clone)]
pub struct CatalogValidate {
    pub kind: String,
    pub on: bool,
    pub value: String,
    pub budget: u32,
    pub dropped: Vec<String>,
}

#[derive(Clone)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub origin: String,
    pub class_name: String,
    pub dir: String,
}

impl From<ListedSkill> for Skill {
    fn from(row: ListedSkill) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            version: row.version,
            origin: row.origin,
            class_name: row.class_name,
            dir: row.dir,
        }
    }
}

#[derive(Clone)]
pub struct PreviewTool {
    pub name: String,
    pub source: String,
    pub executor: String,
}

#[derive(Clone)]
pub struct AssembledPreview {
    pub tools: Vec<PreviewTool>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct CapabilityEntry {
    pub kind: String,
    pub risk: String,
}

pub fn catalog_vendors() -> Result<Vec<Vendor>, AgentFailure> {
    Ok(vendor_summaries()?.into_iter().map(Vendor::from).collect())
}

pub fn catalog_surface(vendor: String, model: String) -> Result<ReasoningSurface, AgentFailure> {
    Ok(surface_for(&vendor, &model)?.into())
}

pub fn catalog_validate(
    vendor: String,
    model: String,
    choice_kind: String,
    on: bool,
    value: String,
    budget: u32,
) -> Result<CatalogValidate, AgentFailure> {
    let choice = ReasoningChoice {
        v: 1,
        body: match choice_kind.as_str() {
            "always_on" => ReasoningChoiceBody::AlwaysOn,
            "toggle" => ReasoningChoiceBody::Toggle { on },
            "effort_enum" => ReasoningChoiceBody::EffortEnum { value },
            "budget_tokens" => ReasoningChoiceBody::BudgetTokens { value: budget },
            "advanced" => ReasoningChoiceBody::Advanced {
                json: serde_json::Value::Null,
            },
            _ => ReasoningChoiceBody::None,
        },
    };
    let raw = serde_json::to_string(&choice).map_err(|err| AgentFailure::Failed {
        message: err.to_string(),
    })?;
    let checked = host_validate(&vendor, &model, &raw)?;
    let value: serde_json::Value =
        serde_json::from_str(&checked).map_err(|err| AgentFailure::Failed {
            message: err.to_string(),
        })?;
    let dropped = value
        .get("dropped")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let parsed: ReasoningChoice = serde_json::from_value(
        value
            .get("choice")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .unwrap_or(choice);
    Ok(catalog_validate_from_choice(parsed, dropped))
}

fn catalog_validate_from_choice(choice: ReasoningChoice, dropped: Vec<String>) -> CatalogValidate {
    let (kind, on, value, budget) = match choice.body {
        ReasoningChoiceBody::None => ("none", false, String::new(), 0),
        ReasoningChoiceBody::AlwaysOn => ("always_on", false, String::new(), 0),
        ReasoningChoiceBody::Toggle { on } => ("toggle", on, String::new(), 0),
        ReasoningChoiceBody::EffortEnum { value } => ("effort_enum", false, value, 0),
        ReasoningChoiceBody::BudgetTokens { value } => {
            ("budget_tokens", false, String::new(), value)
        }
        ReasoningChoiceBody::Advanced { .. } => ("advanced", false, String::new(), 0),
    };
    CatalogValidate {
        kind: kind.into(),
        on,
        value,
        budget,
        dropped,
    }
}

pub fn skill_portable_list(user_root: String, project_root: String) -> Vec<Skill> {
    host_portable(&user_root, &project_root)
        .into_iter()
        .map(Skill::from)
        .collect()
}

pub fn skill_app_catalog(cache_root: String) -> Vec<Skill> {
    let root = cache_root.trim();
    let path = if root.is_empty() {
        None
    } else {
        Some(Path::new(root))
    };
    host_app(path).into_iter().map(Skill::from).collect()
}

pub fn preview_assembled(
    profile_json: String,
    project_root: String,
) -> Result<AssembledPreview, AgentFailure> {
    let profile: kim_agent_host::AgentProfile =
        serde_json::from_str(&profile_json).map_err(|err| AgentFailure::Failed {
            message: format!("profile_json: {err}"),
        })?;
    let preview = kim_agent_host::preview_assembled(&profile, Path::new(&project_root))
        .map_err(AgentFailure::from)?;
    Ok(AssembledPreview {
        tools: preview
            .tools
            .into_iter()
            .map(|tool| PreviewTool {
                name: tool.name,
                source: tool.source,
                executor: tool.executor,
            })
            .collect(),
        warnings: preview.warnings,
    })
}

pub fn capability_catalog() -> Vec<CapabilityEntry> {
    catalog_entries()
        .into_iter()
        .filter_map(|value| {
            Some(CapabilityEntry {
                kind: value.get("kind")?.as_str()?.to_string(),
                risk: value.get("risk")?.as_str()?.to_string(),
            })
        })
        .collect()
}
