use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};
use crate::timeline::PersonRef;

pub(crate) async fn replace_all(
    tx: &mut SqliteConnection,
    account: &str,
    rows: &[PersonRef],
    now: i64,
) -> Result<(), SdkError> {
    sqlx::query("DELETE FROM contacts WHERE account = ?")
        .bind(account)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    for row in rows {
        sqlx::query(
            r"
            INSERT INTO contacts (
              account, peer, relation, nickname, avatar, bio, kind, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(account)
        .bind(&row.account)
        .bind(&row.relation)
        .bind(&row.nickname)
        .bind(&row.avatar)
        .bind(&row.bio)
        .bind(row.kind)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    }
    Ok(())
}

pub(crate) async fn load_all(pool: &SqlitePool, account: &str) -> Result<Vec<PersonRef>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT peer, relation, nickname, avatar, bio, kind
        FROM contacts WHERE account = ?
        ORDER BY nickname, peer
        ",
    )
    .bind(account)
    .fetch_all(pool)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(PersonRef {
            account: row.try_get("peer").map_err(map_sqlx)?,
            nickname: row.try_get("nickname").map_err(map_sqlx)?,
            avatar: row.try_get("avatar").map_err(map_sqlx)?,
            bio: row.try_get("bio").map_err(map_sqlx)?,
            relation: row.try_get("relation").map_err(map_sqlx)?,
            kind: row.try_get("kind").map_err(map_sqlx)?,
        });
    }
    Ok(out)
}
