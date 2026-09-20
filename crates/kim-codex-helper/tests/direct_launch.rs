use std::process::Command;

#[test]
fn direct_launch_exits_2_without_codex_home() {
    let home = tempfile::tempdir().expect("tempdir");
    let status = Command::new(env!("CARGO_BIN_EXE_kim_codex_helper"))
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("CODEX_HOME")
        .status()
        .expect("spawn helper");
    assert_eq!(status.code(), Some(2));
    assert!(!home.path().join(".codex").exists());
}
