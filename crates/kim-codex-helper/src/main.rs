//! Re-exec target for Codex sandbox helpers.
//!
//! The embedded agent does not ship the Codex CLI. Exec-server spawns this
//! binary by absolute path and passes one of the upstream helper arguments.
//! A direct launch exits 2 and does not read `~/.codex`.

use std::ffi::OsStr;
use std::path::Path;
use std::process::exit;

use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_exec_server::CODEX_ARG0_EXEC_HELPER_ARG1;
use codex_exec_server::CODEX_FS_HELPER_ARG1;
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
#[cfg(windows)]
use codex_windows_sandbox::CODEX_WINDOWS_SANDBOX_ARG1;

/// Private argv0 names in `codex-arg0` (`arg0/src/lib.rs`). They are not
/// re-exported. Keep these in sync with `arg0_dispatch` on pin upgrades.
const EXECVE_WRAPPER_ARG0: &str = "codex-execve-wrapper";
const APPLY_PATCH_ARG0: &str = "apply_patch";
const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";

fn main() {
    if !is_helper_invocation() {
        exit(2);
    }
    // Helper branches call `process::exit` and do not return. The non-helper
    // tail of `arg0_dispatch` reads `~/.codex/.env`, so it must not run.
    let _guard = codex_arg0::arg0_dispatch();
    exit(2);
}

fn is_helper_invocation() -> bool {
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap_or_default();
    let exe_name = Path::new(&argv0).file_name().unwrap_or(OsStr::new(""));
    let argv1 = args.next().unwrap_or_default();

    if exe_name == APPLY_PATCH_ARG0 || exe_name == MISSPELLED_APPLY_PATCH_ARG0 {
        return true;
    }
    if exe_name == CODEX_LINUX_SANDBOX_ARG0 {
        return true;
    }
    #[cfg(unix)]
    if exe_name == EXECVE_WRAPPER_ARG0 {
        return true;
    }
    #[cfg(unix)]
    if argv1 == CODEX_ARG0_EXEC_HELPER_ARG1 {
        return true;
    }
    if argv1 == CODEX_FS_HELPER_ARG1 || argv1 == CODEX_CORE_APPLY_PATCH_ARG1 {
        return true;
    }
    #[cfg(windows)]
    if argv1 == CODEX_WINDOWS_SANDBOX_ARG1 {
        return true;
    }
    false
}
