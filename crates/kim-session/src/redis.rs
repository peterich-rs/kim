use std::sync::LazyLock;

use ::redis::aio::ConnectionManager;
use ::redis::Client;
use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};
use prost::Message;

use crate::keys::{key_location, key_session};
use crate::SESSION_TTL;

/// HASH login:loc:{account} field=channel_id value=Location blob; always DEL sn.
const DELETE_LUA: &str = r#"
-- KEYS[1] = login:loc:{account}
-- KEYS[2] = login:sn:{channel_id}
-- ARGV[1] = channel_id
redis.call('DEL', KEYS[2])
redis.call('HDEL', KEYS[1], ARGV[1])
if redis.call('HLEN', KEYS[1]) == 0 then
  redis.call('DEL', KEYS[1])
end
return 1
"#;

static DELETE_SCRIPT: LazyLock<::redis::Script> =
    LazyLock::new(|| ::redis::Script::new(DELETE_LUA));

pub(crate) struct RedisSessionStore {
    conn: ConnectionManager,
}

impl RedisSessionStore {
    pub(crate) async fn open(url: &str) -> Result<Self, SessionError> {
        let client = Client::open(url).map_err(redis_err)?;
        let conn = ConnectionManager::new(client).await.map_err(redis_err)?;
        Ok(Self { conn })
    }

    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, SessionError> {
        let mut conn = self.conn.clone();
        ::redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(redis_err)
    }

    async fn hash_locs(&self, account: &str) -> Result<Vec<Location>, SessionError> {
        let mut conn = self.conn.clone();
        let values: Vec<Vec<u8>> = ::redis::cmd("HVALS")
            .arg(key_location(account, ""))
            .query_async(&mut conn)
            .await
            .map_err(redis_err)?;
        let mut out = Vec::with_capacity(values.len());
        for bytes in values {
            out.push(Location::decode(&bytes)?);
        }
        Ok(out)
    }
}

fn redis_err(e: ::redis::RedisError) -> SessionError {
    SessionError::Other(e.to_string())
}

#[async_trait]
impl SessionStorage for RedisSessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        let loc = Location {
            channel_id: session.channel_id.clone(),
            gate_id: session.gate_id.clone(),
            device: session.device.clone(),
        };
        let loc_key = key_location(&session.account, "");
        let sn_key = key_session(&session.channel_id);
        let loc_bytes = loc.encode();
        let sn_bytes = session.encode_to_vec();
        let ttl = SESSION_TTL.as_secs();
        let mut conn = self.conn.clone();
        ::redis::pipe()
            .atomic()
            .cmd("HSET")
            .arg(&loc_key)
            .arg(&session.channel_id)
            .arg(loc_bytes.as_ref())
            .ignore()
            .cmd("EXPIRE")
            .arg(&loc_key)
            .arg(ttl)
            .ignore()
            .cmd("SET")
            .arg(&sn_key)
            .arg(&sn_bytes)
            .arg("EX")
            .arg(ttl)
            .ignore()
            .query_async::<()>(&mut conn)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        let loc_key = key_location(account, "");
        let sn_key = key_session(channel_id);
        let mut conn = self.conn.clone();
        let _: i32 = DELETE_SCRIPT
            .key(loc_key)
            .key(sn_key)
            .arg(channel_id)
            .invoke_async(&mut conn)
            .await
            .map_err(redis_err)?;
        Ok(())
    }

    async fn get(&self, channel_id: &str) -> Result<Session, SessionError> {
        match self.get_bytes(&key_session(channel_id)).await? {
            Some(bytes) => {
                Session::decode(bytes.as_slice()).map_err(|e| SessionError::Other(e.to_string()))
            }
            None => Err(SessionError::NotFound),
        }
    }

    async fn get_locations(&self, accounts: &[String]) -> Result<Vec<Location>, SessionError> {
        if accounts.is_empty() {
            return Err(SessionError::NotFound);
        }
        let mut out = Vec::new();
        for account in accounts {
            out.extend(self.hash_locs(account).await?);
        }
        if out.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(out)
        }
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        let slots = self.hash_locs(account).await?;
        let loc = if device.is_empty() {
            slots.into_iter().next()
        } else {
            slots.into_iter().find(|l| l.device == device)
        };
        loc.ok_or(SessionError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_session_store;

    /// Hardcoded loopback URL. Live tests never read `REDIS_URL`.
    const LIVE_URL: &str = "redis://127.0.0.1:6379";

    fn session(channel_id: &str, account: &str, gate_id: &str) -> Session {
        Session {
            channel_id: channel_id.into(),
            gate_id: gate_id.into(),
            account: account.into(),
            ..Session::default()
        }
    }

    fn unique(prefix: &str) -> String {
        format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        )
    }

    #[tokio::test]
    async fn invalid_url_is_error_not_memory() {
        match open_session_store(Some("not-a-redis-url")).await {
            Err(SessionError::Other(_)) => {}
            Err(e) => panic!("expected Other, got {e}"),
            Ok(_) => panic!("invalid URL must not fall back to Memory"),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_two_sessions_then_delete_one() {
        let store = open_session_store(Some(LIVE_URL)).await.unwrap();
        let account = unique("alice");
        let id1 = unique("id1");
        let id2 = unique("id2");
        store.add(&session(&id1, &account, "wg-1")).await.unwrap();
        store.add(&session(&id2, &account, "wg-1")).await.unwrap();
        let locs = store.list_locations(&account).await.unwrap();
        assert_eq!(locs.len(), 2);
        store.delete(&account, &id1).await.unwrap();

        let loc = store.get_location(&account, "").await.unwrap();
        assert_eq!(loc.channel_id, id2);
        assert!(matches!(store.get(&id1).await, Err(SessionError::NotFound)));
        let s2 = store.get(&id2).await.unwrap();
        assert_eq!(s2.channel_id, id2);
    }
}
