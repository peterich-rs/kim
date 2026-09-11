use kim_agent_ffi::api::session::{session_open, SessionOpenOpts};
use std::fs;

// Scripted prompt loops land in PR1 (host) / PR3 (FFI). This test only
// proves session_open with a dummy key, snapshot idle, and close.
#[test]
fn scripted_session_opens_idle_and_closes() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = dir.path().join("s.sqlite");
    let root = dir.path().join("workspace");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("AGENTS.md"), "# Agent workspace\nBe helpful.\n").unwrap();
    fs::create_dir_all(root.join(".agents/skills")).unwrap();

    let session = session_open(
        sqlite.to_string_lossy().into(),
        root.to_string_lossy().into(),
        SessionOpenOpts {
            resume_on_open: false,
            api_key: "sk-dummy".into(),
            ..SessionOpenOpts::default()
        },
    )
    .unwrap();

    assert!(!session.snapshot().unwrap().busy);
    session.close().unwrap();
}
