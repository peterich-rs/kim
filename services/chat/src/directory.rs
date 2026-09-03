use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;

use crate::idgen::{IdError, IdGenerator};

#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "postgres")]
pub use postgres::PostgresGroupDirectory;

#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    #[error("idgen: {0}")]
    Id(#[from] IdError),
    #[error("group not found")]
    NotFound,
    #[error("{0}")]
    Backend(String),
}

pub struct CreateGroup {
    pub name: String,
    pub avatar: String,
    pub introduction: String,
    pub owner: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub avatar: String,
    pub introduction: String,
    pub owner: String,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupSummary {
    pub id: String,
    pub name: String,
    pub avatar: String,
}

#[async_trait]
pub trait GroupDirectory: Send + Sync {
    async fn create(&self, app: &str, req: &CreateGroup) -> Result<String, GroupError>;
    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError>;
    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError>;
    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError>;
    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError>;
    async fn summaries(
        &self,
        app: &str,
        group_ids: &[String],
    ) -> Result<Vec<GroupSummary>, GroupError> {
        let mut out = Vec::with_capacity(group_ids.len());
        for id in group_ids {
            match self.detail(app, id).await {
                Ok(g) => out.push(GroupSummary {
                    id: g.id,
                    name: g.name,
                    avatar: g.avatar,
                }),
                Err(GroupError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }
}

struct Group {
    name: String,
    avatar: String,
    introduction: String,
    owner: String,
    members: Vec<String>,
}

pub struct MemoryGroupDirectory {
    idgen: Arc<dyn IdGenerator>,
    inner: RwLock<Inner>,
}

#[derive(Default)]
struct Inner {
    groups: HashMap<(String, String), Group>,
}

impl MemoryGroupDirectory {
    pub fn new(idgen: Arc<dyn IdGenerator>) -> Self {
        Self {
            idgen,
            inner: RwLock::new(Inner::default()),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Skip snowflake; used by talk unit tests.
    pub fn seed(&self, app: &str, group_id: &str, members: Vec<String>) {
        let mut inner = self.write();
        inner.groups.insert(
            (app.to_string(), group_id.to_string()),
            Group {
                name: String::new(),
                avatar: String::new(),
                introduction: String::new(),
                owner: String::new(),
                members,
            },
        );
    }
}

pub(crate) fn normalize_members(owner: &str, members: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in members {
        if seen.insert(m.as_str()) {
            out.push(m.clone());
        }
    }
    if !owner.is_empty() && !seen.contains(owner) {
        out.insert(0, owner.to_string());
    }
    out
}

pub(crate) fn base36_upper(n: i64) -> String {
    const ALPH: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if n == 0 {
        return "0".to_string();
    }
    let mut v = n.unsigned_abs();
    let mut buf = Vec::new();
    while v > 0 {
        buf.push(ALPH[(v % 36) as usize]);
        v /= 36;
    }
    buf.reverse();
    buf.into_iter().map(char::from).collect()
}

#[async_trait]
impl GroupDirectory for MemoryGroupDirectory {
    async fn create(&self, app: &str, req: &CreateGroup) -> Result<String, GroupError> {
        let id = self.idgen.next_id()?;
        let group_id = base36_upper(id);
        let group = Group {
            name: req.name.clone(),
            avatar: req.avatar.clone(),
            introduction: req.introduction.clone(),
            owner: req.owner.clone(),
            members: normalize_members(&req.owner, &req.members),
        };
        self.write()
            .groups
            .insert((app.to_string(), group_id.clone()), group);
        Ok(group_id)
    }

    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError> {
        let inner = self.read();
        let group = inner
            .groups
            .get(&(app.to_string(), group_id.to_string()))
            .ok_or(GroupError::NotFound)?;
        Ok(group.members.clone())
    }

    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        if account.is_empty() {
            return Err(GroupError::Backend("empty account".into()));
        }
        let mut inner = self.write();
        let group = inner
            .groups
            .get_mut(&(app.to_string(), group_id.to_string()))
            .ok_or(GroupError::NotFound)?;
        if !group.members.iter().any(|m| m == account) {
            group.members.push(account.to_string());
        }
        Ok(())
    }

    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        let mut inner = self.write();
        let group = inner
            .groups
            .get_mut(&(app.to_string(), group_id.to_string()))
            .ok_or(GroupError::NotFound)?;
        group.members.retain(|m| m != account);
        Ok(())
    }

    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError> {
        let inner = self.read();
        let group = inner
            .groups
            .get(&(app.to_string(), group_id.to_string()))
            .ok_or(GroupError::NotFound)?;
        Ok(GroupInfo {
            id: group_id.to_string(),
            name: group.name.clone(),
            avatar: group.avatar.clone(),
            introduction: group.introduction.clone(),
            owner: group.owner.clone(),
            members: group.members.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idgen::SequenceIdGen;

    #[tokio::test]
    async fn create_returns_nonempty_id_and_members_include_owner() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let dir = MemoryGroupDirectory::new(idgen);
        let id = dir
            .create(
                "kim",
                &CreateGroup {
                    name: "g".into(),
                    avatar: String::new(),
                    introduction: String::new(),
                    owner: "alice".into(),
                    members: vec!["bob".into()],
                },
            )
            .await
            .unwrap();
        assert!(!id.is_empty());
        let members = dir.members("kim", &id).await.unwrap();
        assert!(members.contains(&"alice".to_string()));
        assert!(members.contains(&"bob".to_string()));
        assert_eq!(members[0], "alice");
    }

    #[tokio::test]
    async fn unknown_group_members_is_not_found() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let dir = MemoryGroupDirectory::new(idgen);
        match dir.members("kim", "no-such").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn seed_is_readable_by_members() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let dir = MemoryGroupDirectory::new(idgen);
        dir.seed("kim", "g1", vec!["a".into(), "b".into()]);
        let members = dir.members("kim", "g1").await.unwrap();
        assert_eq!(members, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn join_quit_detail_roundtrip() {
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        let dir = MemoryGroupDirectory::new(idgen);
        let id = dir
            .create(
                "kim",
                &CreateGroup {
                    name: "g".into(),
                    avatar: "a".into(),
                    introduction: "i".into(),
                    owner: "alice".into(),
                    members: vec!["bob".into()],
                },
            )
            .await
            .unwrap();
        dir.join("kim", &id, "carol").await.unwrap();
        let d = dir.detail("kim", &id).await.unwrap();
        assert_eq!(d.name, "g");
        assert!(d.members.contains(&"carol".to_string()));
        dir.quit("kim", &id, "bob").await.unwrap();
        let members = dir.members("kim", &id).await.unwrap();
        assert!(!members.contains(&"bob".to_string()));
        match dir.join("kim", "nope", "x").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        match dir.detail("kim", "nope").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        match dir.quit("kim", "nope", "x").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
