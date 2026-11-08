use std::ffi::OsStr;
use tokio::process::Command;

#[cfg(unix)]
const PASSTHROUGH: &[&str] = &["PATH", "HOME", "LANG", "LC_ALL", "TMPDIR", "TERM"];

#[cfg(not(unix))]
const PASSTHROUGH: &[&str] = &[
    "PATH",
    "Path",
    "HOME",
    "LANG",
    "LC_ALL",
    "TMPDIR",
    "TERM",
    "TMP",
    "TEMP",
    "USERPROFILE",
];

pub fn env_allowed(key: &str) -> bool {
    if key.ends_with("_API_KEY") || key == "API_KEY" || key.ends_with("_TOKEN") {
        return false;
    }
    PASSTHROUGH.contains(&key)
}

pub fn prepare(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.kill_on_drop(true);
    cmd.env_clear();
    for key in PASSTHROUGH {
        if env_allowed(key) {
            if let Ok(value) = std::env::var(key) {
                cmd.env(key, value);
            }
        }
    }
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd
}

pub fn kill_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

pub fn kill_child_process(pid: Option<u32>, child: Option<&mut tokio::process::Child>) {
    if let Some(pid) = pid {
        kill_group(pid);
    }
    if let Some(child) = child {
        let _ = child.start_kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_env_does_not_inherit_api_keys() {
        assert!(!env_allowed("OPENAI_API_KEY"));
        assert!(!env_allowed("ANTHROPIC_API_KEY"));
        assert!(!env_allowed("API_KEY"));
        assert!(env_allowed("PATH") || env_allowed("Path"));
        assert!(env_allowed("HOME"));
        assert!(!env_allowed("SECRET_TOKEN"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_child_process_terminates_sleep() {
        let mut cmd = prepare("sleep");
        cmd.arg("30")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut child = cmd.spawn().expect("spawn sleep");
        let pid = child.id();
        kill_child_process(pid, Some(&mut child));
        let waited = tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await;
        assert!(waited.is_ok(), "child should exit after start_kill");
    }
}
