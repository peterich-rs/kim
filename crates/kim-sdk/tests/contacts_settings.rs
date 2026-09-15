#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use kim_sdk::{
    DeviceSettings, KimSdk, PersonRef, ProtocolClient, SdkError, SessionUpdate, StartSession,
};

struct FailingContactsProto;

#[async_trait::async_trait]
impl ProtocolClient for FailingContactsProto {
    async fn send_message(
        &self,
        _dest: &str,
        _kind: i32,
        _body: &str,
        _extra: &str,
        _payload_type: i32,
        _client_id: &str,
    ) -> Result<(i64, i64), SdkError> {
        Err(SdkError::NotConnected)
    }

    async fn ack(&self, _message_id: i64) -> Result<(), SdkError> {
        Ok(())
    }

    async fn ack_batch(&self, _ids: &[i64]) -> Result<(), SdkError> {
        Ok(())
    }

    async fn mark_read(&self, _dest: &str, _kind: i32, _message_id: i64) -> Result<(), SdkError> {
        Ok(())
    }

    async fn history(
        &self,
        _dest: &str,
        _kind: i32,
        _before_id: i64,
        _limit: i32,
    ) -> Result<Vec<kim_client::HistoryItem>, SdkError> {
        Ok(Vec::new())
    }
}

fn session() -> StartSession {
    session_for("alice")
}

fn session_for(account: &str) -> StartSession {
    StartSession {
        url: "ws://127.0.0.1:1/".into(),
        token: "t".into(),
        user_agent: "test".into(),
        account: account.into(),
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
    let contacts = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            match rx.recv().await {
                Some(SessionUpdate::ContactsChanged { contacts }) => return contacts,
                Some(_) => continue,
                None => panic!("session update channel closed before ContactsChanged"),
            }
        }
    })
    .await
    .expect("ContactsChanged must be delivered");
    assert_eq!(contacts[0].account, "bob");
    assert_eq!(contacts[0].relation, "friend");
    let loaded = sdk.load_contacts().await.unwrap();
    assert_eq!(loaded[0].account, "bob");
}

#[tokio::test]
async fn contacts_offline_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
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

    let mut rx = sdk.subscribe_contacts();
    let snapshot = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let snapshot = rx.borrow().clone();
            if snapshot
                .contacts
                .iter()
                .any(|person| person.account == "bob")
            {
                return snapshot;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("contacts snapshot must rebuild from SQLite");
    assert_eq!(snapshot.contacts[0].nickname, "Bob");
    assert!(snapshot.sync_error.is_none());
}

#[tokio::test]
async fn contacts_refresh_error_keeps_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    let mut rx = sdk.subscribe_contacts();
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
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            if rx
                .borrow()
                .contacts
                .iter()
                .any(|person| person.account == "bob")
            {
                return;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("cache must reach contacts watch");

    sdk.install_protocol(Arc::new(FailingContactsProto));
    assert!(matches!(
        sdk.refresh_contacts().await,
        Err(SdkError::NotConnected)
    ));
    let snapshot = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let snapshot = rx.borrow().clone();
            if snapshot.sync_error.is_some() {
                return snapshot;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("failed refresh must publish sync error");
    assert!(snapshot
        .contacts
        .iter()
        .any(|person| person.account == "bob"));

    sdk.mark_outgoing_contact("carol".into()).await.unwrap();
    let still_err = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let snapshot = rx.borrow().clone();
            if snapshot
                .contacts
                .iter()
                .any(|person| person.account == "carol")
            {
                return snapshot;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("outgoing upsert must rebuild contacts");
    assert!(still_err.sync_error.is_some());
    assert!(still_err
        .contacts
        .iter()
        .any(|person| person.account == "bob"));
}

#[tokio::test]
async fn replace_contacts_keeps_unmatched_outgoing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    sdk.mark_outgoing_contact("bob".into()).await.unwrap();
    sdk.replace_contacts(vec![PersonRef {
        account: "erin".into(),
        nickname: "Erin".into(),
        avatar: String::new(),
        bio: String::new(),
        relation: "friend".into(),
        kind: 1,
    }])
    .await
    .unwrap();
    let contacts = sdk.load_contacts().await.unwrap();
    assert!(
        contacts
            .iter()
            .any(|person| person.account == "bob" && person.relation == "outgoing")
    );
    assert!(
        contacts
            .iter()
            .any(|person| person.account == "erin" && person.relation == "friend")
    );
}

#[tokio::test]
async fn contacts_switch_account_does_not_show_previous_account() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kim-cache.db");
    let sdk = KimSdk::open(path.to_string_lossy().into_owned())
        .await
        .unwrap();
    sdk.start_session(session()).await.unwrap();
    let mut rx = sdk.subscribe_contacts();
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
    tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            if rx
                .borrow()
                .contacts
                .iter()
                .any(|person| person.account == "bob")
            {
                return;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("alice contacts must reach the watch");

    sdk.start_session(session_for("carol")).await.unwrap();
    let snapshot = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            let snapshot = rx.borrow().clone();
            if !snapshot
                .contacts
                .iter()
                .any(|person| person.account == "bob")
            {
                return snapshot;
            }
            rx.changed().await.unwrap();
        }
    })
    .await
    .expect("account switch must clear the previous contacts snapshot");
    assert!(snapshot.contacts.is_empty());
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
