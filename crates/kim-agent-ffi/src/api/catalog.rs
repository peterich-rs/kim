//! Structured catalog and skill rows. The host does not return JSON here.

use std::path::PathBuf;
use std::sync::OnceLock;

use kim_agent_host::{
    catalog_entries, catalog_validate as host_validate, skill_app_catalog as host_app,
    skill_portable_list as host_portable, surface_for, vendor_summaries, ListedSkill,
    ReasoningChoice, ReasoningChoiceBody, ReasoningSurface as HostReasoningSurface, VendorGroup,
    VendorSummary,
};

use super::failure::AgentFailure;

static SUPPORT_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Same platform fact `platform_bootstrap` hands to `kim_client_ffi`: the
/// app support root. Layout conventions below it are Rust-owned.
pub fn set_platform_support_root(support: String) -> Result<(), AgentFailure> {
    let root = PathBuf::from(support.trim());
    if root.as_os_str().is_empty() {
        return Err(AgentFailure::Failed {
            message: "support root is required".into(),
        });
    }
    let _ = SUPPORT_ROOT.set(root);
    Ok(())
}

fn support_root() -> Result<&'static PathBuf, AgentFailure> {
    SUPPORT_ROOT.get().ok_or(AgentFailure::Failed {
        message: "platform support root not set; call set_platform_support_root".into(),
    })
}

/// Real `~/.agents/skills` (S-KD 23). Desktop Rust resolves `$HOME` itself.
/// Internal: kept off the FFI surface (PathBuf is not a wire type).
#[flutter_rust_bridge::frb(ignore)]
#[must_use]
pub fn user_agents_skills() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let home = PathBuf::from(home);
    if home.as_os_str().is_empty() {
        return None;
    }
    Some(home.join(".agents").join("skills"))
}

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

/// Portable skills: global root is Rust-derived (`$HOME/.agents/skills`);
/// `project_root` comes from the workspace grant, not a per-call path.
pub fn skill_portable_list(project_root: String) -> Vec<Skill> {
    let user_root = user_agents_skills()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    host_portable(&user_root, &project_root)
        .into_iter()
        .map(Skill::from)
        .collect()
}

/// App skill catalog at the layout-derived cache root.
pub fn skill_app_catalog() -> Result<Vec<Skill>, AgentFailure> {
    let path = support_root()?.join("agent").join("app-skills").join("cache");
    Ok(host_app(Some(&path)).into_iter().map(Skill::from).collect())
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
