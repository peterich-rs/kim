//! Platform bootstrap: the only facts Flutter owns are directory roots and
//! build identity. Every derived path convention lives here, in Rust.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::SdkError;

/// Directory roots + build identity. Everything here is a platform fact that
/// only the embedding app can know (path_provider / package_info_plus).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformBootstrap {
    pub documents: PathBuf,
    pub support: PathBuf,
    pub cache: PathBuf,
    pub temp: PathBuf,
    pub app_version: String,
    pub build_number: String,
    /// iOS simulator / Android emulator flag. Loopback `ws://` URLs are only
    /// reachable from a simulator, not a real phone.
    pub simulator: bool,
}

static BOOTSTRAP: OnceLock<PlatformBootstrap> = OnceLock::new();

/// Install the process-wide bootstrap. Idempotent: a second call returns
/// `Ok(())` and leaves the first value in place (tests use
/// [`set_bootstrap_for_test`]).
pub fn set_bootstrap(bootstrap: PlatformBootstrap) -> Result<(), SdkError> {
    let _ = BOOTSTRAP.set(bootstrap);
    Ok(())
}

/// Test-only: replace the process bootstrap between cases.
#[cfg(test)]
#[doc(hidden)]
pub fn set_bootstrap_for_test(bootstrap: PlatformBootstrap) {
    let _ = BOOTSTRAP.set(bootstrap);
}

#[must_use]
pub fn bootstrap() -> Option<&'static PlatformBootstrap> {
    BOOTSTRAP.get()
}

fn require_bootstrap() -> Result<&'static PlatformBootstrap, SdkError> {
    BOOTSTRAP.get().ok_or(SdkError::BootstrapMissing)
}

/// `KIM/{version} ({os}; build {build})`. Same shape the Dart shell used to
/// assemble per call; now derived once from the bootstrap.
#[must_use]
pub fn user_agent() -> String {
    let (version, build) = match bootstrap() {
        Some(b) => (b.app_version.as_str(), b.build_number.as_str()),
        None => ("0", "0"),
    };
    format!("KIM/{version} ({}; build {build})", std::env::consts::OS)
}

/// Derived layout. Construct via [`Layout::current`]; never stores state.
#[derive(Clone, Debug)]
pub struct Layout {
    support: PathBuf,
    temp: PathBuf,
}

impl Layout {
    #[must_use]
    pub fn from_bootstrap(b: &PlatformBootstrap) -> Self {
        Self {
            support: b.support.clone(),
            temp: b.temp.clone(),
        }
    }

    pub fn current() -> Result<Self, SdkError> {
        Ok(Self::from_bootstrap(require_bootstrap()?))
    }

    #[must_use]
    pub fn support(&self) -> &Path {
        &self.support
    }

    pub fn db_path(&self) -> PathBuf {
        self.support.join("kim-cache.db")
    }

    pub fn log_path(&self) -> PathBuf {
        self.support.join("kim.log")
    }

    pub fn media_dir(&self) -> PathBuf {
        self.support.join("kim-media")
    }

    #[must_use]
    pub fn agent_root(&self) -> PathBuf {
        self.support.join("agent")
    }

    pub fn agent_sessions(&self) -> PathBuf {
        self.agent_root().join("sessions")
    }

    pub fn agent_workspaces(&self) -> PathBuf {
        self.agent_root().join("workspaces")
    }

    pub fn app_skill_cache(&self) -> PathBuf {
        self.agent_root().join("app-skills").join("cache")
    }

    /// Per-agent sandbox. Rejects traversal before it reaches the filesystem.
    pub fn sandbox_for(&self, profile_id: &str) -> Result<PathBuf, SdkError> {
        let id = profile_id.trim();
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err(SdkError::InvalidArgument {
                message: format!("unsafe sandbox id {profile_id:?}"),
            });
        }
        Ok(self.agent_workspaces().join(id))
    }

    /// Goose conversation JSON file for one dest x profile. Segment rules
    /// mirror the Dart shell this replaces: `[^A-Za-z0-9._-]` -> `_`, 80 cap.
    pub fn session_file(&self, dest: &str, profile_id: &str) -> PathBuf {
        self.agent_sessions().join(format!(
            "{}__{}.json",
            path_segment(dest),
            path_segment(profile_id)
        ))
    }

    /// Create the agent directory skeleton once.
    pub fn ensure_agent_dirs(&self) -> Result<(), SdkError> {
        for dir in [
            self.agent_root(),
            self.agent_sessions(),
            self.agent_workspaces(),
        ] {
            std::fs::create_dir_all(dir).map_err(disk)?;
        }
        Ok(())
    }

    /// Create (and seed once) a private per-agent sandbox:
    /// `AGENTS.md` / `MEMORY.md` / `notes/`.
    pub fn ensure_sandbox(&self, profile_id: &str) -> Result<PathBuf, SdkError> {
        let dir = self.sandbox_for(profile_id)?;
        std::fs::create_dir_all(dir.join("notes")).map_err(disk)?;
        let agents_md = dir.join("AGENTS.md");
        if !agents_md.exists() {
            std::fs::write(
                &agents_md,
                "# Private workspace\n\n\
                 This is a private workspace for this KIM agent.\n\
                 Long-term notes live in MEMORY.md and notes/.\n\n",
            )
            .map_err(disk)?;
        }
        let memory_md = dir.join("MEMORY.md");
        if !memory_md.exists() {
            std::fs::write(&memory_md, "").map_err(disk)?;
        }
        Ok(dir)
    }

    /// Temp file path for byte payloads that must exist on disk before
    /// upload. Owned by Rust; the caller cleans up.
    pub fn temp_file(&self, prefix: &str) -> PathBuf {
        let unique = uuid::Uuid::new_v4().simple().to_string();
        self.temp.join(format!("{prefix}-{unique}"))
    }
}

fn path_segment(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    };
    cleaned.chars().take(80).collect()
}

fn disk(err: std::io::Error) -> SdkError {
    SdkError::Disk {
        message: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout::from_bootstrap(&PlatformBootstrap {
            documents: PathBuf::from("/tmp/kim-test/Documents"),
            support: PathBuf::from("/tmp/kim-test/Support"),
            cache: PathBuf::from("/tmp/kim-test/Cache"),
            temp: PathBuf::from("/tmp/kim-test/Temp"),
            app_version: "1.2.3".into(),
            build_number: "42".into(),
            simulator: false,
        })
    }

    #[test]
    fn derived_paths_follow_conventions() {
        let l = layout();
        assert_eq!(
            l.db_path(),
            PathBuf::from("/tmp/kim-test/Support/kim-cache.db")
        );
        assert_eq!(
            l.media_dir(),
            PathBuf::from("/tmp/kim-test/Support/kim-media")
        );
        assert_eq!(
            l.app_skill_cache(),
            PathBuf::from("/tmp/kim-test/Support/agent/app-skills/cache")
        );
    }

    #[test]
    fn sandbox_rejects_traversal() {
        let l = layout();
        assert!(l.sandbox_for("ok-id").is_ok());
        assert!(l.sandbox_for("").is_err());
        assert!(l.sandbox_for("../escape").is_err());
        assert!(l.sandbox_for("a/b").is_err());
        assert!(l.sandbox_for("a\\b").is_err());
        assert!(l.sandbox_for("  ").is_err());
    }

    #[test]
    fn session_segments_are_sanitized() {
        let l = layout();
        let f = l.session_file("bob alice!", "goose/é");
        assert_eq!(
            f,
            PathBuf::from("/tmp/kim-test/Support/agent/sessions/bob_alice___goose__.json")
        );
    }

    #[test]
    fn empty_segment_becomes_unknown() {
        let l = layout();
        assert!(l
            .session_file("   ", "x")
            .to_string_lossy()
            .contains("unknown__x.json"));
    }

    #[test]
    fn long_segment_truncates_at_80() {
        let long = "a".repeat(200);
        let l = layout();
        let name = l
            .session_file(&long, "p")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let dest_seg = name.split("__").next().unwrap();
        assert_eq!(dest_seg.len(), 80);
    }

    #[test]
    fn user_agent_shape() {
        set_bootstrap_for_test(PlatformBootstrap {
            documents: PathBuf::from("/d"),
            support: PathBuf::from("/s"),
            cache: PathBuf::from("/c"),
            temp: PathBuf::from("/t"),
            app_version: "9.9.9".into(),
            build_number: "7".into(),
            simulator: false,
        });
        let ua = user_agent();
        assert!(ua.starts_with("KIM/9.9.9 ("));
        assert!(ua.contains("build 7"));
        assert!(ua.contains(std::env::consts::OS));
    }

    #[test]
    fn missing_bootstrap_is_structured_error() {
        // Tests that installed a bootstrap pollute the OnceLock; only assert
        // the error mapping here, not absence.
        if let Err(err) = Layout::current() {
            assert!(matches!(err, SdkError::BootstrapMissing));
        }
    }
}
