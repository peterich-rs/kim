use async_trait::async_trait;
use redis::aio::ConnectionManager;

use super::{AckIndex, StoreError, ACK_TTL};

pub(crate) struct RedisAckIndex {
    conn: ConnectionManager,
}

impl RedisAckIndex {
    pub(crate) async fn open(url: &str) -> Result<Self, StoreError> {
        let conn = kim_session::open_connection_manager(url)
            .await
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(Self { conn })
    }
}

fn key(account: &str) -> String {
    format!("chat:ack:{account}")
}

fn redis_err(e: redis::RedisError) -> StoreError {
    StoreError::Backend(e.to_string())
}

#[async_trait]
impl AckIndex for RedisAckIndex {
    async fn get(&self, account: &str) -> Result<i64, StoreError> {
        let mut conn = self.conn.clone();
        let val: Option<i64> = redis::cmd("GET")
            .arg(key(account))
            .query_async(&mut conn)
            .await
            .map_err(redis_err)?;
        Ok(val.unwrap_or(0))
    }

    async fn set(&self, account: &str, message_id: i64) -> Result<(), StoreError> {
        let mut conn = self.conn.clone();
        redis::cmd("SET")
            .arg(key(account))
            .arg(message_id)
            .arg("EX")
            .arg(ACK_TTL.as_secs())
            .query_async::<()>(&mut conn)
            .await
            .map_err(redis_err)?;
        Ok(())
    }
}
