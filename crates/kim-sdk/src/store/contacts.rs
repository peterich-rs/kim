use std::collections::HashSet;

use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::error::{map_sqlx, SdkError};
use crate::timeline::PersonRef;

pub(crate) async fn replace_all(
    tx: &mut SqliteConnection,
    account: &str,
    rows: &[PersonRef],
    now: i64,
) -> Result<(), SdkError> {
    let outgoing = load_outgoing(tx, account).await?;
    sqlx::query("DELETE FROM contacts WHERE account = ?")
        .bind(account)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    let mut server_peers = HashSet::with_capacity(rows.len());
    for row in rows {
        insert_row(tx, account, row, now).await?;
        server_peers.insert(row.account.clone());
    }
    for row in outgoing {
        if !server_peers.contains(&row.account) {
            insert_row(tx, account, &row, now).await?;
        }
    }
    Ok(())
}

async fn insert_row(
    tx: &mut SqliteConnection,
    account: &str,
    row: &PersonRef,
    now: i64,
) -> Result<(), SdkError> {
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
    Ok(())
}

async fn load_outgoing(
    tx: &mut SqliteConnection,
    account: &str,
) -> Result<Vec<PersonRef>, SdkError> {
    let rows = sqlx::query(
        r"
        SELECT peer, relation, nickname, avatar, bio, kind
        FROM contacts WHERE account = ? AND relation = 'outgoing'
        ",
    )
    .bind(account)
    .fetch_all(&mut *tx)
    .await
    .map_err(map_sqlx)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(person_from_row(&row)?);
    }
    Ok(out)
}

fn person_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<PersonRef, SdkError> {
    Ok(PersonRef {
        account: row.try_get("peer").map_err(map_sqlx)?,
        nickname: row.try_get("nickname").map_err(map_sqlx)?,
        avatar: row.try_get("avatar").map_err(map_sqlx)?,
        bio: row.try_get("bio").map_err(map_sqlx)?,
        relation: row.try_get("relation").map_err(map_sqlx)?,
        kind: row.try_get("kind").map_err(map_sqlx)?,
    })
}

/// Inserts or updates a contact. A missing relation is the profile-push case:
/// only existing rows are patched, so an unrelated profile event cannot create
/// a contact with a fabricated relationship.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_one(
    tx: &mut SqliteConnection,
    account: &str,
    peer: &str,
    relation: Option<&str>,
    nickname: &str,
    avatar: &str,
    bio: Option<&str>,
    kind: Option<i32>,
    now: i64,
) -> Result<bool, SdkError> {
    if let Some(relation) = relation {
        let bio_present = bio.is_some();
        let kind_present = kind.is_some();
        let result = sqlx::query(
            r"
            INSERT INTO contacts (
              account, peer, relation, nickname, avatar, bio, kind, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(account, peer) DO UPDATE SET
              relation = excluded.relation,
              nickname = excluded.nickname,
              avatar = excluded.avatar,
              bio = CASE WHEN ? THEN excluded.bio ELSE contacts.bio END,
              kind = CASE WHEN ? THEN excluded.kind ELSE contacts.kind END,
              updated_at = excluded.updated_at
            ",
        )
        .bind(account)
        .bind(peer)
        .bind(relation)
        .bind(nickname)
        .bind(avatar)
        .bind(bio.unwrap_or_default())
        .bind(kind.unwrap_or(1))
        .bind(now)
        .bind(bio_present)
        .bind(kind_present)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() > 0)
    } else {
        let result = sqlx::query(
            r"
            UPDATE contacts
            SET nickname = ?, avatar = ?, updated_at = ?
            WHERE account = ? AND peer = ?
            ",
        )
        .bind(nickname)
        .bind(avatar)
        .bind(now)
        .bind(account)
        .bind(peer)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() > 0)
    }
}

pub(crate) async fn delete_peer(
    tx: &mut SqliteConnection,
    account: &str,
    peer: &str,
) -> Result<bool, SdkError> {
    let result = sqlx::query("DELETE FROM contacts WHERE account = ? AND peer = ?")
        .bind(account)
        .bind(peer)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(result.rows_affected() > 0)
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
        out.push(person_from_row(&row)?);
    }
    Ok(out)
}
