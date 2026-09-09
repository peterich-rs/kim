use kim_agent_ffi::api::session::{session_open, SessionOpenOpts};
use std::thread;
use std::time::Duration;

#[test]
fn scripted_prompt_completes() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = dir.path().join("s.sqlite");
    let root = dir.path().join("workspace");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# Agent workspace\nBe helpful.\n").unwrap();
    std::fs::create_dir_all(root.join(".agents/skills")).unwrap();

    let session = session_open(
        sqlite.to_string_lossy().into(),
        root.to_string_lossy().into(),
        SessionOpenOpts {
            resume_on_open: false,
            ..SessionOpenOpts::default()
        },
    )
    .unwrap();

    let op = session.prompt("hello".into()).unwrap();
    assert!(!op.is_empty());

    for _ in 0..100 {
        let snap = session.snapshot().unwrap();
        if !snap.busy {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!session.snapshot().unwrap().busy);
    assert!(sqlite.exists());
    session
        .reconfigure(SessionOpenOpts {
            model: "scripted".into(),
            llm_backend: "scripted".into(),
            resume_on_open: false,
            ..SessionOpenOpts::default()
        })
        .unwrap();
    assert!(sqlite.exists());
    session.close();
}
