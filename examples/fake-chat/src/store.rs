use std::sync::{Arc, RwLock, RwLockWriteGuard};

#[cfg(test)]
use std::sync::RwLockReadGuard;

use async_trait::async_trait;

use crate::idgen::{IdError, IdGenerator};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("idgen: {0}")]
    Id(#[from] IdError),
    #[error("{0}")]
    Backend(String),
}

pub struct InsertMessage {
    pub sender: String,
    pub dest: String,
    pub send_time: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

pub struct InsertResult {
    pub message_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    User,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredMessage {
    pub message_id: i64,
    pub app: String,
    pub kind: MessageKind,
    pub sender: String,
    pub dest: String,
    pub send_time: i64,
    pub msg_type: i32,
    pub body: String,
    pub extra: String,
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn insert_user(&self, app: &str, req: &InsertMessage)
        -> Result<InsertResult, StoreError>;
    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError>;
}

/// In-process message log. No disk, no capacity cap; process exit drops all.
pub struct MemoryMessageStore {
    idgen: Arc<dyn IdGenerator>,
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    records: Vec<StoredMessage>,
}

impl MemoryMessageStore {
    pub fn new(idgen: Arc<dyn IdGenerator>) -> Self {
        Self {
            idgen,
            inner: RwLock::new(Inner::default()),
        }
    }

    #[cfg(test)]
    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    fn insert(
        &self,
        app: &str,
        kind: MessageKind,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        let message_id = self.idgen.next_id()?;
        let rec = StoredMessage {
            message_id,
            app: app.to_string(),
            kind,
            sender: req.sender.clone(),
            dest: req.dest.clone(),
            send_time: req.send_time,
            msg_type: req.msg_type,
            body: req.body.clone(),
            extra: req.extra.clone(),
        };
        self.write().records.push(rec);
        Ok(InsertResult { message_id })
    }

    #[cfg(test)]
    pub fn recorded(&self) -> Vec<StoredMessage> {
        self.read().records.clone()
    }
}

#[async_trait]
impl MessageStore for MemoryMessageStore {
    async fn insert_user(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        self.insert(app, MessageKind::User, req)
    }

    async fn insert_group(
        &self,
        app: &str,
        req: &InsertMessage,
    ) -> Result<InsertResult, StoreError> {
        self.insert(app, MessageKind::Group, req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idgen::SequenceIdGen;

    #[tokio::test]
    async fn insert_user_and_group_are_recorded_with_kind() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let store = MemoryMessageStore::new(idgen);
        store
            .insert_user(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "bob".into(),
                    send_time: 1,
                    msg_type: 1,
                    body: "hi".into(),
                    extra: String::new(),
                },
            )
            .await
            .unwrap();
        store
            .insert_group(
                "kim",
                &InsertMessage {
                    sender: "alice".into(),
                    dest: "g1".into(),
                    send_time: 2,
                    msg_type: 1,
                    body: "hey".into(),
                    extra: String::new(),
                },
            )
            .await
            .unwrap();
        let rec = store.recorded();
        assert_eq!(rec.len(), 2);
        assert_eq!(rec[0].kind, MessageKind::User);
        assert_eq!(rec[1].kind, MessageKind::Group);
        assert!(rec[0].message_id > 10_000);
        assert_eq!(rec[1].message_id, rec[0].message_id + 1);
    }
}
