//! Skill discovery, resolution, and the prompt catalog.
//!
//! Two classes live side by side (S-KD 19). **Portable** skills are plain
//! `SKILL.md` directories on disk that Claude / Cursor / Goose / Codex also
//! read; KIM only scans them, never copies them. **App** skills are bound to
//! this messenger's tools, ship with the binary (or a version cache), and are
//! never written into the user's `~/.agents`.
//!
//! Only the catalog (`id: description`) reaches the system prompt. Bodies are
//! injected on demand by `activate_skill`, so an app skill can be updated
//! without touching any profile JSON.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::truncate_chars;

/// Per-activation body cap. Oversized files are truncated, not rejected.
pub const MAX_SKILL_BODY_BYTES: usize = 64 * 1024;
/// `AGENTS.md` rides in the system prompt every turn, so it is capped harder.
pub const MAX_AGENTS_MD_BYTES: usize = 4 * 1024;
/// Reserved prefix for KIM app skills. Portable directories may not use it.
pub const APP_SKILL_PREFIX: &str = "kim-";

const DEFAULT_DOC: &str = "SKILL.md";
const REFERENCES_DIR: &str = "references/";
const MAX_DESCRIPTION_CHARS: usize = 256;

const BUNDLED: &[(&str, &str)] = &[
    ("kim-im", include_str!("../app-skills/kim-im/SKILL.md")),
    (
        "kim-memory",
        include_str!("../app-skills/kim-memory/SKILL.md"),
    ),
];

fn enabled_true() -> bool {
    true
}

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("skill {0} is not in this session's catalog")]
    NotInCatalog(String),
    #[error("skill {0} is not installed")]
    Unknown(String),
    #[error("skill {id} has no version {version}")]
    Version { id: String, version: String },
    #[error("path escapes the skill directory")]
    PathEscape,
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillClass {
    #[default]
    Portable,
    App,
}

/// What a profile persists. Portable skills are discovered, never listed here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRef {
    pub id: String,
    #[serde(default)]
    pub class: SkillClass,
    /// `bundled` | `cache` | `cloud` — app skills only.
    #[serde(default)]
    pub origin: String,
    /// Empty means latest: the next activation may resolve newer text.
    #[serde(default)]
    pub version: String,
    #[serde(default = "enabled_true")]
    pub enabled: bool,
}

impl Default for SkillRef {
    fn default() -> Self {
        Self {
            id: String::new(),
            class: SkillClass::App,
            origin: "bundled".into(),
            version: String::new(),
            enabled: true,
        }
    }
}

/// Frontmatter fields KIM surfaces. Unknown YAML keys are ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillDoc {
    pub meta: SkillMeta,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableSkill {
    pub id: String,
    pub dir: PathBuf,
    pub meta: SkillMeta,
}

/// One resolved app-skill document plus where references may be read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPackage {
    pub id: String,
    pub version: String,
    /// `bundled` | `cache` | `cloud`.
    pub origin: String,
    pub meta: SkillMeta,
    pub body: String,
    /// Directory backing `references/…`; `None` for text compiled into the binary.
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    Portable {
        dir: PathBuf,
    },
    App {
        origin: String,
        /// The profile's request: empty means latest.
        pin: String,
        /// What that request resolved to when the catalog was built.
        version: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    pub id: String,
    pub class: SkillClass,
    pub meta: SkillMeta,
    pub source: SkillSource,
}

#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    pub app: Vec<SkillEntry>,
    pub portable: Vec<SkillEntry>,
}

impl SkillRegistry {
    pub fn is_empty(&self) -> bool {
        self.app.is_empty() && self.portable.is_empty()
    }

    pub fn len(&self) -> usize {
        self.app.len() + self.portable.len()
    }

    pub fn get(&self, id: &str) -> Option<&SkillEntry> {
        self.app
            .iter()
            .chain(self.portable.iter())
            .find(|e| e.id == id)
    }

    pub fn ids(&self) -> Vec<&str> {
        self.app
            .iter()
            .chain(self.portable.iter())
            .map(|e| e.id.as_str())
            .collect()
    }
}

/// Where the portable shelves are, and whether this profile may read them.
pub struct RegistryScan<'a> {
    /// Real `~/.agents/skills`. Empty disables the user shelf (S-KD 23).
    pub user_root: &'a str,
    /// Workspace root; the project shelf is `<root>/.agents/skills`.
    pub project_root: &'a Path,
    /// S-KD 3: a sandbox IM-only profile must not inherit `git-commit`.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activation {
    pub id: String,
    pub version: String,
    pub path: String,
    pub body: String,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    #[serde(default)]
    name: Option<serde_norway::Value>,
    #[serde(default)]
    description: Option<serde_norway::Value>,
    #[serde(default)]
    version: Option<serde_norway::Value>,
}

fn split_frontmatter(text: &str) -> Option<(String, String)> {
    let mut lines = text.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }
    let mut frontmatter_lines = Vec::new();
    let mut found_closing = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        frontmatter_lines.push(line);
    }
    if !found_closing {
        return None;
    }
    Some((
        frontmatter_lines.join("\n"),
        lines.collect::<Vec<_>>().join("\n"),
    ))
}

fn yaml_scalar_line(value: Option<serde_norway::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let raw = match value {
        serde_norway::Value::String(s) => s,
        serde_norway::Value::Number(n) => n.to_string(),
        serde_norway::Value::Bool(b) => b.to_string(),
        serde_norway::Value::Null
        | serde_norway::Value::Sequence(_)
        | serde_norway::Value::Mapping(_) => {
            return String::new();
        }
        serde_norway::Value::Tagged(tagged) => return yaml_scalar_line(Some(tagged.value)),
    };
    sanitize_single_line(&raw)
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Quote unquoted scalars that YAML rejects (`Build for AWS: ECS`, `<duration: e.g. 7d>`).
/// Block scalars (`|`, `>`) are left alone.
fn repair_frontmatter_scalar_fields(frontmatter: &str) -> Option<String> {
    let mut changed = false;
    let mut block_scalar_indent: Option<usize> = None;
    let mut repaired_lines = Vec::new();
    for line in frontmatter.lines() {
        let indent = line
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        if let Some(block_indent) = block_scalar_indent {
            if line.trim().is_empty() || indent > block_indent {
                repaired_lines.push(line.to_string());
                continue;
            }
            block_scalar_indent = None;
        }

        let Some((key, value)) = line.split_once(':') else {
            repaired_lines.push(line.to_string());
            continue;
        };
        if key.trim().is_empty() || !value.chars().next().is_none_or(char::is_whitespace) {
            repaired_lines.push(line.to_string());
            continue;
        }

        let trimmed_start = value.trim_start();
        let leading_whitespace = &value[..value.len() - trimmed_start.len()];
        let mut scalar = trimmed_start;
        let mut comment = "";
        for (index, character) in trimmed_start.char_indices() {
            if character == '#'
                && (index == 0
                    || trimmed_start[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace))
            {
                let comment_start = trimmed_start[..index].trim_end().len();
                scalar = &trimmed_start[..comment_start];
                comment = &trimmed_start[comment_start..];
                break;
            }
        }

        let scalar = scalar.trim_end();
        let Some(first_char) = scalar.chars().next() else {
            repaired_lines.push(line.to_string());
            continue;
        };
        if matches!(first_char, '|' | '>') {
            block_scalar_indent = Some(indent);
            repaired_lines.push(line.to_string());
            continue;
        }
        if matches!(first_char, '\'' | '"') {
            repaired_lines.push(line.to_string());
            continue;
        }
        let mut has_colon_separator = false;
        let mut chars = scalar.chars().peekable();
        while let Some(character) = chars.next() {
            if character == ':'
                && matches!(chars.peek(), Some(next_character) if next_character.is_whitespace())
            {
                has_colon_separator = true;
                break;
            }
        }
        let invalid_flow_like_scalar = matches!(first_char, '[' | '{' | '@' | '`')
            && serde_norway::from_str::<serde_norway::Value>(scalar).is_err();
        if !has_colon_separator && !invalid_flow_like_scalar {
            repaired_lines.push(line.to_string());
            continue;
        }

        let quoted_scalar = format!("'{}'", scalar.replace('\'', "''"));
        repaired_lines.push(format!(
            "{key}:{leading_whitespace}{quoted_scalar}{comment}"
        ));
        changed = true;
    }
    changed.then(|| repaired_lines.join("\n"))
}

fn parse_frontmatter_yaml(block: &str) -> Option<SkillFrontmatter> {
    match serde_norway::from_str(block) {
        Ok(parsed) => Some(parsed),
        Err(_) => {
            let repaired = repair_frontmatter_scalar_fields(block)?;
            serde_norway::from_str(&repaired).ok()
        }
    }
}

/// YAML frontmatter + markdown body.
///
/// Folded/literal scalars and nested maps come from `serde_norway`. Unquoted
/// colons are repaired so wild `~/.agents` skills still parse. Name and
/// description are collapsed to one line for list rows. Invalid YAML keeps
/// the body and empty meta so a scan never dies on one bad skill.
pub fn parse_skill_md(raw: &str) -> SkillDoc {
    let text = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let Some((block, body)) = split_frontmatter(text) else {
        return SkillDoc {
            meta: SkillMeta::default(),
            body: text.trim().to_string(),
        };
    };
    let meta = parse_frontmatter_yaml(&block)
        .map(|parsed| SkillMeta {
            name: yaml_scalar_line(parsed.name),
            description: yaml_scalar_line(parsed.description),
            version: yaml_scalar_line(parsed.version),
        })
        .unwrap_or_default();
    SkillDoc {
        meta,
        body: body.trim().to_string(),
    }
}

/// Scan one portable shelf: `<dir>/<id>/SKILL.md`. Unreadable entries warn and
/// drop out of the catalog instead of failing the session.
pub fn scan_portable(dir: &Path) -> Vec<PortableSkill> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if id.starts_with('.') {
            continue;
        }
        // S-KD 20: an app skill on a portable shelf would teach other harnesses
        // to hallucinate KIM tools, so the id is never honoured from disk.
        if id.starts_with(APP_SKILL_PREFIX) {
            tracing::warn!(skill_id = %id, "portable scan ignored reserved kim- id");
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(entry.path().join(DEFAULT_DOC)) else {
            tracing::warn!(skill_id = %id, "portable skill has no readable SKILL.md");
            continue;
        };
        let mut doc = parse_skill_md(&raw);
        if doc.meta.name.trim().is_empty() {
            doc.meta.name = id.clone();
        }
        out.push(PortableSkill {
            id,
            dir: entry.path(),
            meta: doc.meta,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn bundled_ids() -> Vec<&'static str> {
    BUNDLED.iter().map(|(id, _)| *id).collect()
}

fn latest_dir(root: &Path) -> Option<PathBuf> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.pop().map(|name| root.join(name))
}

/// The only way to get app-skill text: cache first, then the bundled copy.
/// A cloud catalog slots in behind the same call (S-KD 21).
#[derive(Debug, Clone, Default)]
pub struct SkillResolver {
    cache_root: Option<PathBuf>,
}

impl SkillResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// `support/agent/app-skills/cache`, laid out as `<id>/<version>/SKILL.md`.
    pub fn with_cache(root: PathBuf) -> Self {
        Self {
            cache_root: Some(root),
        }
    }

    pub fn resolve(&self, id: &str, version: &str) -> Result<SkillPackage, SkillError> {
        if let Some(package) = self.cached(id, version) {
            return Ok(package);
        }
        let package = bundled(id).ok_or_else(|| SkillError::Unknown(id.to_string()))?;
        let want = version.trim();
        if !want.is_empty() && want != package.version {
            return Err(SkillError::Version {
                id: id.to_string(),
                version: want.to_string(),
            });
        }
        Ok(package)
    }

    fn cached(&self, id: &str, version: &str) -> Option<SkillPackage> {
        let root = self.cache_root.as_ref()?.join(id);
        let want = version.trim();
        let dir = if want.is_empty() {
            latest_dir(&root)?
        } else {
            root.join(want)
        };
        let raw = std::fs::read_to_string(dir.join(DEFAULT_DOC)).ok()?;
        let mut doc = parse_skill_md(&raw);
        if doc.meta.name.trim().is_empty() {
            doc.meta.name = id.to_string();
        }
        let resolved = if doc.meta.version.trim().is_empty() {
            dir.file_name()?.to_string_lossy().into_owned()
        } else {
            doc.meta.version.clone()
        };
        Some(SkillPackage {
            id: id.to_string(),
            version: resolved,
            origin: "cache".into(),
            meta: doc.meta,
            body: doc.body,
            dir: Some(dir),
        })
    }
}

fn bundled(id: &str) -> Option<SkillPackage> {
    let (_, raw) = BUNDLED.iter().find(|(candidate, _)| *candidate == id)?;
    let mut doc = parse_skill_md(raw);
    if doc.meta.name.trim().is_empty() {
        doc.meta.name = id.to_string();
    }
    let version = if doc.meta.version.trim().is_empty() {
        "0".to_string()
    } else {
        doc.meta.version.clone()
    };
    Some(SkillPackage {
        id: id.to_string(),
        version,
        origin: "bundled".into(),
        meta: doc.meta,
        body: doc.body,
        dir: None,
    })
}

/// Assigned app refs plus whatever the portable shelves hold this session.
pub fn build_registry(
    refs: &[SkillRef],
    denylist: &[String],
    scan: &RegistryScan<'_>,
    resolver: &SkillResolver,
) -> SkillRegistry {
    let mut registry = SkillRegistry::default();
    for candidate in refs {
        if !candidate.enabled || candidate.class != SkillClass::App {
            continue;
        }
        if !candidate.id.starts_with(APP_SKILL_PREFIX) {
            tracing::warn!(skill_id = %candidate.id, "app skill id must use the kim- prefix");
            continue;
        }
        if registry.app.iter().any(|e| e.id == candidate.id) {
            continue;
        }
        match resolver.resolve(&candidate.id, &candidate.version) {
            Ok(package) => registry.app.push(SkillEntry {
                id: package.id,
                class: SkillClass::App,
                meta: package.meta,
                source: SkillSource::App {
                    origin: package.origin,
                    pin: candidate.version.trim().to_string(),
                    version: package.version,
                },
            }),
            Err(err) => {
                tracing::warn!(skill_id = %candidate.id, error = %err, "app skill unresolved")
            }
        }
    }

    if scan.enabled {
        let mut found: BTreeMap<String, PortableSkill> = BTreeMap::new();
        let user_root = scan.user_root.trim();
        if !user_root.is_empty() {
            for skill in scan_portable(Path::new(user_root)) {
                found.insert(skill.id.clone(), skill);
            }
        }
        // The project shelf ships with the repo, so it wins on a same-id clash.
        let project = scan.project_root.join(".agents").join("skills");
        for skill in scan_portable(&project) {
            found.insert(skill.id.clone(), skill);
        }
        for (id, skill) in found {
            if denylist.iter().any(|denied| denied.trim() == id) {
                continue;
            }
            if registry.app.iter().any(|e| e.id == id) {
                continue;
            }
            registry.portable.push(SkillEntry {
                id,
                class: SkillClass::Portable,
                meta: skill.meta,
                source: SkillSource::Portable { dir: skill.dir },
            });
        }
    }
    registry
}

fn truncate_char_count(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max).collect()
}

fn push_catalog_line(out: &mut String, entry: &SkillEntry) {
    let description = truncate_char_count(entry.meta.description.trim(), MAX_DESCRIPTION_CHARS);
    let summary = if description.is_empty() {
        entry.meta.name.trim()
    } else {
        description.as_str()
    };
    if summary.is_empty() {
        out.push_str(&format!("- {}\n", entry.id));
    } else {
        out.push_str(&format!("- {}: {}\n", entry.id, summary));
    }
}

/// The only skill text in the system prompt: `id: description`, two shelves
/// kept apart so the model never offers `kim-im` to another harness.
pub fn catalog_prompt_block(registry: &SkillRegistry) -> String {
    let mut out = String::new();
    if !registry.portable.is_empty() {
        out.push_str(
            "Project/user skills (portable; also visible to other agents on this machine):\n",
        );
        for entry in &registry.portable {
            push_catalog_line(&mut out, entry);
        }
    }
    if !registry.app.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("KIM app skills (only work in this messenger; call activate_skill):\n");
        for entry in &registry.app {
            push_catalog_line(&mut out, entry);
        }
    }
    out.truncate(out.trim_end().len());
    out
}

/// Workspace conventions for the model; skill bodies never come this way.
pub fn read_agents_md(project_root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(project_root.join("AGENTS.md")).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(truncate_chars(trimmed, MAX_AGENTS_MD_BYTES))
}

fn resolve_in_dir(dir: &Path, rel: &str) -> Result<PathBuf, SkillError> {
    if Path::new(rel).is_absolute() {
        return Err(SkillError::PathEscape);
    }
    let root = std::fs::canonicalize(dir).map_err(|e| SkillError::Io(e.to_string()))?;
    let canon = std::fs::canonicalize(root.join(rel)).map_err(|e| SkillError::Io(e.to_string()))?;
    if !canon.starts_with(&root) {
        return Err(SkillError::PathEscape);
    }
    Ok(canon)
}

fn finish(id: &str, version: &str, path: &str, body: String) -> Activation {
    let version = version.trim();
    let truncated = body.len() > MAX_SKILL_BODY_BYTES;
    let body = if truncated {
        truncate_chars(&body, MAX_SKILL_BODY_BYTES)
    } else {
        body
    };
    Activation {
        id: id.to_string(),
        version: if version.is_empty() {
            "0".to_string()
        } else {
            version.to_string()
        },
        path: path.to_string(),
        body,
        truncated,
    }
}

/// JSON list for the plaza / skills page: discovered portable packages only.
pub fn skill_portable_list_json(user_root: &str, project_root: &str) -> String {
    let mut found: BTreeMap<String, PortableSkill> = BTreeMap::new();
    let user = user_root.trim();
    if !user.is_empty() {
        for skill in scan_portable(Path::new(user)) {
            found.insert(skill.id.clone(), skill);
        }
    }
    let project = Path::new(project_root).join(".agents").join("skills");
    for skill in scan_portable(&project) {
        found.insert(skill.id.clone(), skill);
    }
    let items: Vec<Value> = found
        .into_values()
        .map(|skill| {
            json!({
                "id": skill.id,
                "name": skill.meta.name,
                "description": skill.meta.description,
                "version": skill.meta.version,
                "class": "portable",
                "dir": skill.dir.to_string_lossy(),
            })
        })
        .collect();
    json!({ "skills": items }).to_string()
}

/// Bundled (+ cache, when a root is passed) app-skill summaries for assignment UI.
pub fn skill_app_catalog_json(cache_root: Option<&Path>) -> String {
    let resolver = match cache_root {
        Some(root) => SkillResolver::with_cache(root.to_path_buf()),
        None => SkillResolver::new(),
    };
    let mut items = Vec::new();
    for id in bundled_ids() {
        match resolver.resolve(id, "") {
            Ok(package) => items.push(json!({
                "id": package.id,
                "name": package.meta.name,
                "description": package.meta.description,
                "version": package.version,
                "origin": package.origin,
                "class": "app",
            })),
            Err(err) => tracing::warn!(skill_id = %id, error = %err, "app catalog skip"),
        }
    }
    json!({ "skills": items }).to_string()
}

/// Read one catalogued skill's text. `path` defaults to `SKILL.md`; anything
/// else must stay inside the skill directory (canonicalize + prefix).
pub fn activate(
    registry: &SkillRegistry,
    resolver: &SkillResolver,
    id: &str,
    path: Option<&str>,
) -> Result<Activation, SkillError> {
    let entry = registry
        .get(id)
        .ok_or_else(|| SkillError::NotInCatalog(id.to_string()))?;
    let rel = path
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .unwrap_or(DEFAULT_DOC);
    match &entry.source {
        SkillSource::Portable { dir } => {
            let file = resolve_in_dir(dir, rel)?;
            let raw = std::fs::read_to_string(&file).map_err(|e| SkillError::Io(e.to_string()))?;
            let body = if rel == DEFAULT_DOC {
                parse_skill_md(&raw).body
            } else {
                raw
            };
            Ok(finish(id, &entry.meta.version, rel, body))
        }
        SkillSource::App { pin, .. } => {
            // Resolve again: an empty pin must pick up a newer package.
            let package = resolver.resolve(id, pin)?;
            if rel == DEFAULT_DOC {
                return Ok(finish(id, &package.version, rel, package.body));
            }
            if !rel.starts_with(REFERENCES_DIR) {
                return Err(SkillError::PathEscape);
            }
            let dir = package
                .dir
                .as_deref()
                .ok_or_else(|| SkillError::Io(format!("{id} has no {rel}")))?;
            let file = resolve_in_dir(dir, rel)?;
            let raw = std::fs::read_to_string(&file).map_err(|e| SkillError::Io(e.to_string()))?;
            Ok(finish(id, &package.version, rel, raw))
        }
    }
}

/// The hidden message text. The `skill:` line makes an update visible in logs
/// and lets the model tell two activations of one id apart.
pub fn activation_message(activation: &Activation) -> String {
    let mut out = format!("skill: {}@{}", activation.id, activation.version);
    if activation.path != DEFAULT_DOC {
        out.push_str(&format!(" ({})", activation.path));
    }
    out.push_str("\n\n");
    out.push_str(&activation.body);
    if activation.truncated {
        out.push_str("\n\n[truncated]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, id: &str, body: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join(DEFAULT_DOC), body).expect("write");
    }

    fn portable_doc(name: &str, description: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n")
    }

    fn scan_of<'a>(user_root: &'a str, project_root: &'a Path) -> RegistryScan<'a> {
        RegistryScan {
            user_root,
            project_root,
            enabled: true,
        }
    }

    #[test]
    fn frontmatter_reads_three_keys_and_body() {
        let doc = parse_skill_md(
            "---\nname: git-commit\ndescription: \"Conventional Commits\"\nversion: 2.1\nmetadata:\n  kim:\n    x: 1\n---\n\n# Body\ntext\n",
        );
        assert_eq!(doc.meta.name, "git-commit");
        assert_eq!(doc.meta.description, "Conventional Commits");
        assert_eq!(doc.meta.version, "2.1");
        assert_eq!(doc.body, "# Body\ntext");
    }

    #[test]
    fn frontmatter_folded_description_joins_indented_lines() {
        let doc = parse_skill_md(
            "---\nname: codegraph\ndescription: >\n  Query this repo's local CodeGraph index\n  instead of grep.\nlicense: MIT\n---\n\n# Body\n",
        );
        assert_eq!(doc.meta.name, "codegraph");
        assert_eq!(
            doc.meta.description,
            "Query this repo's local CodeGraph index instead of grep."
        );
        assert_eq!(doc.body, "# Body");
    }

    #[test]
    fn frontmatter_repairs_unquoted_colons_and_keeps_block_scalars() {
        let colon = parse_skill_md(
            "---\nname: deploy\ndescription: Build for AWS: ECS\nargument-hint: <duration: e.g. 7d>\n---\n\n# Body\n",
        );
        assert_eq!(colon.meta.description, "Build for AWS: ECS");
        assert_eq!(colon.body, "# Body");

        let block = parse_skill_md(
            "---\nname: block\ndescription: |-\n  Build for AWS: ECS\nargument-hint: <duration: e.g. 7d>\n---\n\n# Body\n",
        );
        assert_eq!(block.meta.description, "Build for AWS: ECS");
    }

    #[test]
    fn missing_frontmatter_keeps_whole_file_as_body() {
        let doc = parse_skill_md("# Just markdown\n");
        assert_eq!(doc.meta, SkillMeta::default());
        assert_eq!(doc.body, "# Just markdown");
    }

    #[test]
    fn scan_skips_kim_prefix_and_missing_doc() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        write_skill(
            root,
            "git-commit",
            &portable_doc("git-commit", "commits", "b"),
        );
        write_skill(root, "kim-im", &portable_doc("kim-im", "evil", "b"));
        std::fs::create_dir_all(root.join("empty-skill")).expect("mkdir");
        let found = scan_portable(root);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].id, "git-commit");
    }

    #[test]
    fn project_shelf_overrides_user_shelf_for_same_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("home-agents-skills");
        let project = dir.path().join("repo");
        let project_shelf = project.join(".agents").join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        std::fs::create_dir_all(&project_shelf).expect("mkdir");
        write_skill(&user, "git-commit", &portable_doc("user", "from user", "b"));
        write_skill(
            &project_shelf,
            "git-commit",
            &portable_doc("project", "from project", "b"),
        );
        write_skill(&user, "only-user", &portable_doc("only", "user only", "b"));

        let registry = build_registry(
            &[],
            &[],
            &scan_of(&user.to_string_lossy(), &project),
            &SkillResolver::new(),
        );
        assert_eq!(registry.portable.len(), 2);
        let entry = registry.get("git-commit").expect("git-commit");
        assert_eq!(entry.meta.description, "from project");
        assert_eq!(
            entry.source,
            SkillSource::Portable {
                dir: project_shelf.join("git-commit")
            }
        );
    }

    #[test]
    fn denylist_drops_a_portable_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        write_skill(&user, "noisy-skill", &portable_doc("noisy", "noise", "b"));
        write_skill(&user, "quiet-skill", &portable_doc("quiet", "calm", "b"));

        let registry = build_registry(
            &[],
            &["noisy-skill".to_string()],
            &scan_of(&user.to_string_lossy(), dir.path()),
            &SkillResolver::new(),
        );
        assert_eq!(registry.ids(), vec!["quiet-skill"]);
    }

    #[test]
    fn scan_disabled_keeps_app_refs_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        write_skill(&user, "git-commit", &portable_doc("g", "commits", "b"));

        let registry = build_registry(
            &[SkillRef {
                id: "kim-im".into(),
                ..SkillRef::default()
            }],
            &[],
            &RegistryScan {
                user_root: &user.to_string_lossy(),
                project_root: dir.path(),
                enabled: false,
            },
            &SkillResolver::new(),
        );
        assert_eq!(registry.ids(), vec!["kim-im"]);
    }

    #[test]
    fn bundled_app_skills_resolve_with_metadata() {
        let resolver = SkillResolver::new();
        assert_eq!(bundled_ids(), vec!["kim-im", "kim-memory"]);
        for id in bundled_ids() {
            let package = resolver.resolve(id, "").expect("resolve");
            assert_eq!(package.origin, "bundled");
            assert_eq!(package.meta.name, id);
            assert!(!package.meta.description.is_empty(), "{id}");
            assert!(!package.version.is_empty(), "{id}");
            assert!(!package.body.contains("description:"), "{id}");
        }
        assert!(matches!(
            resolver.resolve("kim-im", "99"),
            Err(SkillError::Version { .. })
        ));
        assert!(matches!(
            resolver.resolve("git-commit", ""),
            Err(SkillError::Unknown(_))
        ));
    }

    #[test]
    fn cache_version_wins_over_bundled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = dir.path().join("cache");
        let versioned = cache.join("kim-im").join("7");
        std::fs::create_dir_all(&versioned).expect("mkdir");
        std::fs::write(
            versioned.join(DEFAULT_DOC),
            "---\nname: kim-im\ndescription: newer\nversion: 7\n---\n\nnewer body\n",
        )
        .expect("write");
        let resolver = SkillResolver::with_cache(cache);
        let package = resolver.resolve("kim-im", "").expect("resolve");
        assert_eq!(package.version, "7");
        assert_eq!(package.origin, "cache");
        assert_eq!(package.body, "newer body");
    }

    #[test]
    fn catalog_block_separates_the_two_shelves() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        write_skill(
            &user,
            "git-commit",
            &portable_doc("git-commit", "Create Conventional Commits", "b"),
        );
        let registry = build_registry(
            &[SkillRef {
                id: "kim-memory".into(),
                ..SkillRef::default()
            }],
            &[],
            &scan_of(&user.to_string_lossy(), dir.path()),
            &SkillResolver::new(),
        );
        let block = catalog_prompt_block(&registry);
        assert!(
            block.starts_with("Project/user skills (portable;"),
            "{block}"
        );
        assert!(
            block.contains("- git-commit: Create Conventional Commits"),
            "{block}"
        );
        assert!(
            block.contains("KIM app skills (only work in this messenger; call activate_skill):"),
            "{block}"
        );
        assert!(block.contains("- kim-memory: "), "{block}");
        assert!(catalog_prompt_block(&SkillRegistry::default()).is_empty());
    }

    #[test]
    fn agents_md_is_truncated_to_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(read_agents_md(dir.path()).is_none());
        std::fs::write(dir.path().join("AGENTS.md"), "   \n").expect("write");
        assert!(read_agents_md(dir.path()).is_none());
        std::fs::write(dir.path().join("AGENTS.md"), "x".repeat(9000)).expect("write");
        let text = read_agents_md(dir.path()).expect("agents.md");
        assert_eq!(text.len(), MAX_AGENTS_MD_BYTES);
    }

    #[test]
    fn activate_portable_returns_body_without_frontmatter() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        write_skill(
            &user,
            "git-commit",
            "---\nname: git-commit\ndescription: commits\nversion: 3\n---\n\ndo the thing\n",
        );
        let registry = build_registry(
            &[],
            &[],
            &scan_of(&user.to_string_lossy(), dir.path()),
            &SkillResolver::new(),
        );
        let activation =
            activate(&registry, &SkillResolver::new(), "git-commit", None).expect("activate");
        assert_eq!(activation.body, "do the thing");
        assert_eq!(activation.version, "3");
        assert_eq!(activation.path, DEFAULT_DOC);
        assert!(activation_message(&activation).starts_with("skill: git-commit@3\n\n"));
    }

    #[test]
    fn activate_path_escape_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("skills");
        std::fs::create_dir_all(&user).expect("mkdir");
        write_skill(&user, "git-commit", &portable_doc("g", "commits", "b"));
        std::fs::write(user.join("secret.txt"), "nope").expect("write");
        let registry = build_registry(
            &[],
            &[],
            &scan_of(&user.to_string_lossy(), dir.path()),
            &SkillResolver::new(),
        );
        let resolver = SkillResolver::new();
        assert!(matches!(
            activate(&registry, &resolver, "git-commit", Some("../secret.txt")),
            Err(SkillError::PathEscape)
        ));
        assert!(matches!(
            activate(&registry, &resolver, "git-commit", Some("/etc/hosts")),
            Err(SkillError::PathEscape)
        ));
    }

    #[test]
    fn activate_app_rejects_paths_outside_references() {
        let registry = build_registry(
            &[SkillRef {
                id: "kim-im".into(),
                ..SkillRef::default()
            }],
            &[],
            &RegistryScan {
                user_root: "",
                project_root: Path::new("/tmp"),
                enabled: false,
            },
            &SkillResolver::new(),
        );
        let resolver = SkillResolver::new();
        assert!(matches!(
            activate(&registry, &resolver, "kim-im", Some("../../etc/hosts")),
            Err(SkillError::PathEscape)
        ));
        let activation = activate(&registry, &resolver, "kim-im", None).expect("activate");
        assert!(activation.body.contains("send_message"));
    }

    #[test]
    fn activate_unlisted_id_is_not_in_catalog() {
        let registry = SkillRegistry::default();
        assert!(matches!(
            activate(&registry, &SkillResolver::new(), "git-commit", None),
            Err(SkillError::NotInCatalog(_))
        ));
    }

    #[test]
    fn portable_list_json_prefers_project_shelf() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user");
        let project = dir.path().join("proj");
        std::fs::create_dir_all(&user).expect("mkdir");
        std::fs::create_dir_all(project.join(".agents").join("skills")).expect("mkdir");
        write_skill(&user, "git-commit", &portable_doc("u", "user copy", "b"));
        write_skill(
            &project.join(".agents").join("skills"),
            "git-commit",
            &portable_doc("p", "project copy", "b"),
        );
        let raw = skill_portable_list_json(&user.to_string_lossy(), &project.to_string_lossy());
        let value: Value = serde_json::from_str(&raw).expect("json");
        let skills = value["skills"].as_array().expect("skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["description"], "project copy");
    }

    #[test]
    fn app_catalog_json_lists_bundled_kim_skills() {
        let raw = skill_app_catalog_json(None);
        let value: Value = serde_json::from_str(&raw).expect("json");
        let skills = value["skills"].as_array().expect("skills");
        let ids: Vec<&str> = skills.iter().filter_map(|s| s["id"].as_str()).collect();
        assert!(ids.contains(&"kim-im"), "{ids:?}");
        assert!(ids.contains(&"kim-memory"), "{ids:?}");
    }
}
