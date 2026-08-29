use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::idgen::IdGenerator;

use super::{base36_upper, normalize_members, CreateGroup, GroupDirectory, GroupError, GroupInfo};

pub struct PostgresGroupDirectory {
    pool: PgPool,
    idgen: Arc<dyn IdGenerator>,
}

impl PostgresGroupDirectory {
    pub fn from_pool(pool: PgPool, idgen: Arc<dyn IdGenerator>) -> Self {
        Self { pool, idgen }
    }
}

fn pg_err(e: sqlx::Error) -> GroupError {
    GroupError::Backend(e.to_string())
}

#[async_trait]
impl GroupDirectory for PostgresGroupDirectory {
    async fn create(&self, app: &str, req: &CreateGroup) -> Result<String, GroupError> {
        let id = self.idgen.next_id()?;
        let group_id = base36_upper(id);
        let members = normalize_members(&req.owner, &req.members);
        let mut tx = self.pool.begin().await.map_err(pg_err)?;
        sqlx::query(
            "INSERT INTO groups (app, id, name, avatar, introduction, owner)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(app)
        .bind(&group_id)
        .bind(&req.name)
        .bind(&req.avatar)
        .bind(&req.introduction)
        .bind(&req.owner)
        .execute(&mut *tx)
        .await
        .map_err(pg_err)?;
        for (pos, account) in members.iter().enumerate() {
            let pos = i32::try_from(pos).unwrap_or(i32::MAX);
            sqlx::query(
                "INSERT INTO group_members (app, group_id, account, pos)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(app)
            .bind(&group_id)
            .bind(account)
            .bind(pos)
            .execute(&mut *tx)
            .await
            .map_err(pg_err)?;
        }
        tx.commit().await.map_err(pg_err)?;
        Ok(group_id)
    }

    async fn members(&self, app: &str, group_id: &str) -> Result<Vec<String>, GroupError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT account FROM group_members
             WHERE app = $1 AND group_id = $2
             ORDER BY pos ASC",
        )
        .bind(app)
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn join(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        if account.is_empty() {
            return Err(GroupError::Backend("empty account".into()));
        }
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM groups WHERE app = $1 AND id = $2")
                .bind(app)
                .bind(group_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        if exists.is_none() {
            return Err(GroupError::Backend("unknown group".into()));
        }
        let next: (i32,) = sqlx::query_as(
            "SELECT COALESCE(MAX(pos), -1) + 1 FROM group_members WHERE app = $1 AND group_id = $2",
        )
        .bind(app)
        .bind(group_id)
        .fetch_one(&self.pool)
        .await
        .map_err(pg_err)?;
        sqlx::query(
            "INSERT INTO group_members (app, group_id, account, pos)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (app, group_id, account) DO NOTHING",
        )
        .bind(app)
        .bind(group_id)
        .bind(account)
        .bind(next.0)
        .execute(&self.pool)
        .await
        .map_err(pg_err)?;
        Ok(())
    }

    async fn quit(&self, app: &str, group_id: &str, account: &str) -> Result<(), GroupError> {
        sqlx::query("DELETE FROM group_members WHERE app = $1 AND group_id = $2 AND account = $3")
            .bind(app)
            .bind(group_id)
            .bind(account)
            .execute(&self.pool)
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn detail(&self, app: &str, group_id: &str) -> Result<GroupInfo, GroupError> {
        let row: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT name, avatar, introduction, owner FROM groups WHERE app = $1 AND id = $2",
        )
        .bind(app)
        .bind(group_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(pg_err)?;
        let (name, avatar, introduction, owner) =
            row.ok_or_else(|| GroupError::Backend("unknown group".into()))?;
        let members = self.members(app, group_id).await?;
        Ok(GroupInfo {
            id: group_id.to_string(),
            name,
            avatar,
            introduction,
            owner,
            members,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idgen::SequenceIdGen;
    use crate::store::connect_pool;
    use std::time::Duration;

    async fn dir() -> Option<PostgresGroupDirectory> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let pool = connect_pool(
            &url,
            crate::store::PoolOpts {
                max_connections: 2,
                acquire_timeout: Duration::from_secs(3),
                idle_timeout: Duration::from_secs(60),
            },
        )
        .await
        .ok()?;
        let idgen: Arc<dyn IdGenerator> = Arc::new(SequenceIdGen::default());
        Some(PostgresGroupDirectory::from_pool(pool, idgen))
    }

    #[tokio::test]
    async fn postgres_create_join_quit_detail() {
        let Some(dir) = dir().await else {
            return;
        };
        let app = format!("g_{}", crate::store::now_unix_nano());
        let id = dir
            .create(
                &app,
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
        let members = dir.members(&app, &id).await.unwrap();
        assert_eq!(members[0], "alice");
        assert!(members.contains(&"bob".to_string()));
        dir.join(&app, &id, "carol").await.unwrap();
        let d = dir.detail(&app, &id).await.unwrap();
        assert_eq!(d.name, "g");
        assert!(d.members.contains(&"carol".to_string()));
        dir.quit(&app, &id, "bob").await.unwrap();
        let after = dir.members(&app, &id).await.unwrap();
        assert!(!after.contains(&"bob".to_string()));
        assert!(dir.join(&app, "nope", "x").await.is_err());
        assert!(dir.detail(&app, "nope").await.is_err());
        dir.quit(&app, "nope", "x").await.unwrap();
    }
}
