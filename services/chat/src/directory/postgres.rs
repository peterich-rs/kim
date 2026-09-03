use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use crate::idgen::IdGenerator;

use super::{
    base36_upper, normalize_members, CreateGroup, GroupDirectory, GroupError, GroupInfo,
    GroupSummary,
};

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
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM groups WHERE app = $1 AND id = $2")
                .bind(app)
                .bind(group_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        if exists.is_none() {
            return Err(GroupError::NotFound);
        }
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
            return Err(GroupError::NotFound);
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
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM groups WHERE app = $1 AND id = $2")
                .bind(app)
                .bind(group_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(pg_err)?;
        if exists.is_none() {
            return Err(GroupError::NotFound);
        }
        sqlx::query("DELETE FROM group_members WHERE app = $1 AND group_id = $2 AND account = $3")
            .bind(app)
            .bind(group_id)
            .bind(account)
            .execute(&self.pool)
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn summaries(
        &self,
        app: &str,
        group_ids: &[String],
    ) -> Result<Vec<GroupSummary>, GroupError> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, name, avatar FROM groups WHERE app = $1 AND id = ANY($2)")
                .bind(app)
                .bind(group_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(pg_err)?;
        let mut by_id = std::collections::HashMap::with_capacity(rows.len());
        for (id, name, avatar) in rows {
            by_id.insert(id, (name, avatar));
        }
        let mut out = Vec::with_capacity(group_ids.len());
        for id in group_ids {
            if let Some((name, avatar)) = by_id.get(id) {
                out.push(GroupSummary {
                    id: id.clone(),
                    name: name.clone(),
                    avatar: avatar.clone(),
                });
            }
        }
        Ok(out)
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
        let (name, avatar, introduction, owner) = row.ok_or(GroupError::NotFound)?;
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

    async fn dir() -> Option<PostgresGroupDirectory> {
        let url = std::env::var("DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())?;
        let pool = connect_pool(
            &url,
            crate::store::PoolOpts {
                max_connections: 2,
                ..crate::store::PoolOpts::default()
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
        let sums = dir
            .summaries(&app, &[id.clone(), "missing".into()])
            .await
            .unwrap();
        assert_eq!(sums.len(), 1);
        assert_eq!(sums[0].id, id);
        assert_eq!(sums[0].name, "g");
        assert_eq!(sums[0].avatar, "a");
        dir.quit(&app, &id, "bob").await.unwrap();
        let after = dir.members(&app, &id).await.unwrap();
        assert!(!after.contains(&"bob".to_string()));
        match dir.join(&app, "nope", "x").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        match dir.detail(&app, "nope").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
        match dir.quit(&app, "nope", "x").await {
            Err(GroupError::NotFound) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
