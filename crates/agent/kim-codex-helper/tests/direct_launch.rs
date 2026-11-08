use std::process::Command;

#[test]
fn direct_launch_exits_2_without_codex_home() {
    let home = tempfile::tempdir().expect("tempdir");
    // Cargo 1.95 exports `CARGO_BIN_EXE_<bin-name>` with the hyphenated
    // target name, and only on the running test — not to `env!`.
    let exe = std::env::var("CARGO_BIN_EXE_kim-codex-helper")
        .expect("cargo sets CARGO_BIN_EXE_kim-codex-helper for this test");
    let status = Command::new(exe)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("CODEX_HOME")
        .status()
        .expect("spawn helper");
    assert_eq!(status.code(), Some(2));
    assert!(!home.path().join(".codex").exists());
}
