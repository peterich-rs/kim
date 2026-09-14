#![allow(clippy::unwrap_used)]

use kim_sdk::{DeviceSettings, KimSdk, PersonRef, SessionUpdate, StartSession};

fn session() -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: "alice".into(),
    }
}

#[tokio::test]
async fn replace_contacts_emits_contacts_changed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    let mut rx = sdk.subscribe_session();
    sdk.replace_contacts(vec![PersonRef {
        account: "bob".into(),
        nickname: "Bob".into(),
        avatar: String::new(),
        bio: String::new(),
        relation: "friend".into(),
        kind: 1,
    }])
    .await
    .unwrap();
    match rx.recv().await {
        Some(SessionUpdate::ContactsChanged { contacts }) => {
            assert_eq!(contacts[0].account, "bob");
            assert_eq!(contacts[0].relation, "friend");
        }
        other => panic!("expected ContactsChanged, got {other:?}"),
    }
    let loaded = sdk.load_contacts().await.unwrap();
    assert_eq!(loaded[0].account, "bob");
}

#[tokio::test]
async fn import_device_settings_is_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    let first = sdk
        .import_device_settings(
            "wss://a.example/".into(),
            "https://a.example".into(),
            "prod".into(),
            "zh".into(),
        )
        .await
        .unwrap();
    assert_eq!(first.ws_url, "wss://a.example/");
    let second = sdk
        .import_device_settings(
            "wss://b.example/".into(),
            "https://b.example".into(),
            "dev".into(),
            "en".into(),
        )
        .await
        .unwrap();
    assert_eq!(second.ws_url, "wss://a.example/");
    let patched = sdk
        .settings_patch(Some("wss://c.example/".into()), None, None, None)
        .await
        .unwrap();
    assert_eq!(patched.ws_url, "wss://c.example/");
    let _ = DeviceSettings::default();
}

#[tokio::test]
async fn agent_profiles_imported_before_login_are_visible_after_start() {
    use kim_sdk::AgentProfileRow;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.import_agent_profiles(vec![AgentProfileRow {
        profile_id: "goose".into(),
        nickname: "助手".into(),
        server_account: "b_bot".into(),
        body_json: "{}".into(),
    }])
    .await
    .unwrap();
    let before = sdk.list_agent_profiles().await.unwrap();
    assert_eq!(before.len(), 1);
    sdk.start_session(session()).await.unwrap();
    let after = sdk.list_agent_profiles().await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].profile_id, "goose");
    assert_eq!(after[0].server_account, "b_bot");
}
