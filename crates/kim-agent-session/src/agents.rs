//! Default discovery of `AGENTS.md` and `.agents/**` (on by default).
//!
//! Does not break existing `.agents/skills/` layouts — all `*.md` under `.agents`
//! are included (stable path sort), including skills.

use std::fs;
use std::path::{Path, PathBuf};

/// Soft cap for injected instruction text (chars).
pub const DEFAULT_AGENTS_CHAR_CAP: usize = 64 * 1024;

#[derive(Debug, Clone, Default)]
pub struct AgentsDiscovery {
    pub root_agents_md: Option<PathBuf>,
    pub agents_dir_files: Vec<PathBuf>,
    pub truncated: bool,
}

/// Discover and load project agent instructions.
///
/// Order: walk parents to git root for `AGENTS.md`, then `.agents/` fragments.
/// Missing files are a no-op.
pub fn discover_agents_context(
    start: impl AsRef<Path>,
    char_cap: usize,
) -> (String, AgentsDiscovery) {
    let start = start.as_ref();
    let mut meta = AgentsDiscovery::default();
    let mut parts: Vec<(String, String)> = Vec::new(); // (label, text)

    if let Some(agents_md) = find_agents_md(start) {
        if let Ok(text) = fs::read_to_string(&agents_md) {
            meta.root_agents_md = Some(agents_md.clone());
            parts.push((format!("AGENTS.md ({})", agents_md.display()), text));
        }
    }

    if let Some(agents_root) = find_agents_dir(start) {
        // Prefer `.agents/AGENTS.md` / `.agents/agents.md` first, then other md.
        let mut files = collect_agents_md_files(&agents_root);
        files.sort();
        for f in files {
            if let Ok(text) = fs::read_to_string(&f) {
                let label = f
                    .strip_prefix(&agents_root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| f.display().to_string());
                meta.agents_dir_files.push(f);
                parts.push((format!(".agents/{label}"), text));
            }
        }
    }

    let mut out = String::new();
    for (label, text) in parts {
        let chunk = format!("### {label}\n{text}\n\n");
        if out.len() + chunk.len() > char_cap {
            let remain = char_cap.saturating_sub(out.len());
            if remain > 64 {
                out.push_str(&chunk[..remain]);
                out.push_str("\n\n[truncated: agents context exceeded cap]\n");
            } else {
                out.push_str("\n\n[truncated: agents context exceeded cap]\n");
            }
            meta.truncated = true;
            break;
        }
        out.push_str(&chunk);
    }

    (out, meta)
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(p) = cur {
        if p.join(".git").exists() {
            return Some(p.to_path_buf());
        }
        cur = p.parent();
    }
    None
}

fn find_agents_md(start: &Path) -> Option<PathBuf> {
    let stop = find_git_root(start);
    let mut cur = Some(start);
    while let Some(p) = cur {
        let candidate = p.join("AGENTS.md");
        if candidate.is_file() {
            return Some(candidate);
        }
        if stop.as_ref() == Some(&p.to_path_buf()) {
            break;
        }
        cur = p.parent();
    }
    None
}

fn find_agents_dir(start: &Path) -> Option<PathBuf> {
    let stop = find_git_root(start);
    let mut cur = Some(start);
    while let Some(p) = cur {
        let candidate = p.join(".agents");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if stop.as_ref() == Some(&p.to_path_buf()) {
            break;
        }
        cur = p.parent();
    }
    None
}

fn collect_agents_md_files(agents_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                // Include `.agents/skills/**` and any nested md.
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
    walk(agents_root, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_agents_md_and_skills_without_breaking() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "root instructions").unwrap();
        fs::create_dir_all(dir.path().join(".agents/skills")).unwrap();
        fs::write(
            dir.path().join(".agents/AGENTS.md"),
            "agents dir instructions",
        )
        .unwrap();
        fs::write(dir.path().join(".agents/skills/foo.md"), "skill foo").unwrap();

        let (text, meta) = discover_agents_context(dir.path(), DEFAULT_AGENTS_CHAR_CAP);
        assert!(meta.root_agents_md.is_some());
        assert!(text.contains("root instructions"));
        assert!(text.contains("agents dir instructions"));
        assert!(text.contains("skill foo"));
        assert!(meta.agents_dir_files.len() >= 2);
    }

    #[test]
    fn missing_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let (text, meta) = discover_agents_context(dir.path(), DEFAULT_AGENTS_CHAR_CAP);
        assert!(text.is_empty());
        assert!(meta.root_agents_md.is_none());
        assert!(meta.agents_dir_files.is_empty());
    }
}
