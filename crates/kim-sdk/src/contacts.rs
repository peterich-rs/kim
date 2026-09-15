use std::collections::BTreeMap;

use tokio::sync::watch;

use crate::session::lock;
use crate::store::changes::CommitEffect;
use crate::{map_client, ContactsSnapshot, KimSdk, PersonRef, SdkError};

pub(crate) enum ContactsErrorOp {
    Keep,
    Clear,
    Set(String),
}

impl KimSdk {
    /// Refreshes the locally cached contacts. UI consumers must read the
    /// resulting [`ContactsSnapshot`] from [`Self::subscribe_contacts`].
    pub async fn refresh_contacts(&self) -> Result<(), SdkError> {
        let epoch = self.current_epoch().0;
        let account = self.session_snapshot()?.account;
        let protocol = match self.protocol() {
            Ok(protocol) => protocol,
            Err(error) => {
                self.send_contacts_error(&account, epoch, error.to_string())
                    .await;
                return Err(error);
            }
        };

        let friends = match protocol.friend_list().await {
            Ok(friends) => friends,
            Err(error) => {
                let message = error.to_string();
                self.send_contacts_error(&account, epoch, message).await;
                return Err(map_client(error, ""));
            }
        };
        let incoming = match protocol.friend_incoming().await {
            Ok(incoming) => incoming,
            Err(error) => {
                let message = error.to_string();
                self.send_contacts_error(&account, epoch, message).await;
                return Err(map_client(error, ""));
            }
        };

        let current = self.current_epoch().0;
        if current != epoch {
            return Err(SdkError::StaleEpoch {
                expected: epoch,
                actual: current,
            });
        }
        self.replace_contacts(map_person_refs(friends, incoming))
            .await
    }

    /// Records a locally pending outbound friend request. Server friend lists
    /// do not include this row; `replace_contacts` keeps unmatched outgoing.
    pub async fn mark_outgoing_contact(&self, peer: String) -> Result<(), SdkError> {
        if peer.is_empty() {
            return Err(SdkError::InvalidArgument {
                message: "peer is required".into(),
            });
        }
        self.upsert_contact(
            peer.clone(),
            Some("outgoing".into()),
            peer,
            String::new(),
            None,
            None,
        )
        .await
    }

    /// Subscribes to the account-stamped contacts view and schedules its
    /// reconstruction from SQLite. The single sender is deliberately reused
    /// so FFI streams survive reconnects.
    pub fn subscribe_contacts(&self) -> watch::Receiver<ContactsSnapshot> {
        let account = self
            .session_snapshot()
            .map(|session| session.account)
            .unwrap_or_default();
        let epoch = self.current_epoch().0;
        let rx = self.inner.contacts_watch.subscribe();
        let account_changed = {
            let mut stamped_account = lock(&self.inner.contacts_account);
            // First subscribe stamps "" → account; that is not a switch and
            // must not flash an empty snapshot before the SQLite rebuild.
            let changed = !stamped_account.is_empty() && *stamped_account != account;
            *stamped_account = account.clone();
            changed
        };
        self.inner
            .contacts_epoch
            .store(epoch, std::sync::atomic::Ordering::SeqCst);
        if account_changed {
            self.publish_contacts_snapshot(Vec::new(), ContactsErrorOp::Clear);
        }
        if let Some(changes) = lock(&self.inner.changes).clone() {
            changes.record(account, epoch, CommitEffect::contacts());
        }
        rx
    }

    /// Removes a contact only after its remote removal command succeeded.
    pub async fn remove_contact(&self, peer: String) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), sequence) = store.delete_contact(epoch, session.account, peer).await?;
        self.after_command(sequence).await;
        Ok(())
    }

    pub(crate) async fn persist_contact_event(
        &self,
        event: &kim_client::SessionEvent,
    ) -> Result<(), SdkError> {
        match event {
            kim_client::SessionEvent::FriendRequest { from, nickname } => {
                self.upsert_contact(
                    from.clone(),
                    Some("incoming".into()),
                    nickname.clone(),
                    String::new(),
                    None,
                    None,
                )
                .await
            }
            kim_client::SessionEvent::FriendAccepted { from, nickname } => {
                // Contacts are keyed by (account, peer), so this changes an
                // existing incoming row into a friend row rather than leaving
                // a second incoming entry to delete.
                self.upsert_contact(
                    from.clone(),
                    Some("friend".into()),
                    nickname.clone(),
                    String::new(),
                    None,
                    None,
                )
                .await
            }
            kim_client::SessionEvent::ProfileUpdated { profile } => {
                self.upsert_contact(
                    profile.account.clone(),
                    None,
                    profile.nickname.clone(),
                    profile.avatar.clone(),
                    None,
                    None,
                )
                .await
            }
            _ => Ok(()),
        }
    }

    async fn send_contacts_error(&self, account: &str, epoch: u64, message: String) {
        if self.current_epoch().0 != epoch {
            return;
        }
        let contacts = match self.store() {
            Ok(store) => store.load_contacts(account).await.unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        if self.current_epoch().0 != epoch
            || lock(&self.inner.contacts_account).as_str() != account
            || self
                .inner
                .contacts_epoch
                .load(std::sync::atomic::Ordering::SeqCst)
                != epoch
        {
            return;
        }
        self.publish_contacts_snapshot(contacts, ContactsErrorOp::Set(message));
    }

    pub(crate) fn publish_contacts_snapshot(
        &self,
        contacts: Vec<PersonRef>,
        error: ContactsErrorOp,
    ) {
        let mut sync_error = lock(&self.inner.contacts_sync_error);
        match error {
            ContactsErrorOp::Keep => {}
            ContactsErrorOp::Clear => *sync_error = None,
            ContactsErrorOp::Set(message) => *sync_error = Some(message),
        }
        let snapshot = ContactsSnapshot {
            version: self.next_contacts_version(),
            contacts,
            sync_error: sync_error.clone(),
        };
        drop(sync_error);
        self.inner.contacts_watch.send_replace(snapshot);
    }

    async fn upsert_contact(
        &self,
        peer: String,
        relation: Option<String>,
        nickname: String,
        avatar: String,
        bio: Option<String>,
        kind: Option<i32>,
    ) -> Result<(), SdkError> {
        let store = self.store()?;
        let session = self.session_snapshot()?;
        let epoch = self.current_epoch().0;
        let ((), _sequence) = store
            .upsert_contact(
                epoch,
                session.account,
                peer,
                relation,
                nickname,
                avatar,
                bio,
                kind,
            )
            .await?;
        Ok(())
    }
}

fn map_person_refs(
    friends: Vec<kim_client::Profile>,
    incoming: Vec<kim_client::Profile>,
) -> Vec<PersonRef> {
    let mut rows = BTreeMap::new();
    for profile in incoming {
        rows.insert(profile.account.clone(), person_ref(profile, "incoming"));
    }
    for profile in friends {
        rows.insert(profile.account.clone(), person_ref(profile, "friend"));
    }
    rows.into_values().collect()
}

fn person_ref(profile: kim_client::Profile, relation: &str) -> PersonRef {
    PersonRef {
        account: profile.account,
        nickname: profile.nickname,
        avatar: profile.avatar,
        bio: profile.bio,
        relation: relation.into(),
        kind: profile.kind,
    }
}
