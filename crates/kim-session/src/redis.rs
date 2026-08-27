use ::redis::aio::ConnectionManager;
use ::redis::Client;
use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};
use prost::Message;

use crate::keys::{key_location, key_session};
use crate::SESSION_TTL;

/// Delete: always DEL sn; DEL loc only if it still names this channel_id.
/// KEYS/ARGV and layout match docs/link-layer-login.md §5.2 exactly.
const DELETE_LUA: &str = r#"
-- KEYS[1] = login:loc:{account}     （Location::encode 的 blob）
-- KEYS[2] = login:sn:{channel_id}
-- ARGV[1] = 正在 Delete 的 channel_id
-- 布局：u16le n | channel[n] | u16le m | gate[m]
local loc = redis.call('GET', KEYS[1])
redis.call('DEL', KEYS[2])          -- sn 总是删
if type(loc) ~= 'string' or #loc < 2 then
  return 0
end
local n = loc:byte(1) + loc:byte(2) * 256
if n < 0 or #loc < 2 + n then
  return 0
end
local ch = loc:sub(3, 2 + n)
if ch == ARGV[1] then
  redis.call('DEL', KEYS[1])
  return 1
end
return 0
"#;

pub(crate) struct RedisSessionStore {
    conn: ConnectionManager,
}

impl RedisSessionStore {
    pub(crate) async fn open(url: &str) -> Result<Self, SessionError> {
        let client = Client::open(url).map_err(redis_err)?;
        let conn = ConnectionManager::new(client).await.map_err(redis_err)?;
        Ok(Self { conn })
    }

    async fn set_ex(&self, key: &str, value: &[u8]) -> Result<(), SessionError> {
        let mut conn = self.conn.clone();
        ::redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(SESSION_TTL.as_secs())
            .query_async::<()>(&mut conn)
            .await
            .map_err(redis_err)
    }

    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, SessionError> {
        let mut conn = self.conn.clone();
        ::redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(redis_err)
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
        };
        let loc_key = key_location(&session.account, &session.device);
        let sn_key = key_session(&session.channel_id);
        let loc_bytes = loc.encode();
        self.set_ex(&loc_key, loc_bytes.as_ref()).await?;
        self.set_ex(&sn_key, &session.encode_to_vec()).await?;
        Ok(())
    }

    async fn delete(&self, account: &str, channel_id: &str) -> Result<(), SessionError> {
        let loc_key = key_location(account, "");
        let sn_key = key_session(channel_id);
        let mut conn = self.conn.clone();
        let deleted_loc: i32 = ::redis::Script::new(DELETE_LUA)
            .key(loc_key)
            .key(sn_key)
            .arg(channel_id)
            .invoke_async(&mut conn)
            .await
            .map_err(redis_err)?;
        if deleted_loc == 0 {
            tracing::debug!("keep location, newer channel");
        }
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
        let mut out = Vec::new();
        for account in accounts {
            match self.get_location(account, "").await {
                Ok(loc) => out.push(loc),
                Err(SessionError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        if out.is_empty() {
            Err(SessionError::NotFound)
        } else {
            Ok(out)
        }
    }

    async fn get_location(&self, account: &str, device: &str) -> Result<Location, SessionError> {
        match self.get_bytes(&key_location(account, device)).await? {
            Some(bytes) => Location::decode(&bytes),
            None => Err(SessionError::NotFound),
        }
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
    async fn live_delete_old_channel_keeps_newer_location() {
        let store = open_session_store(Some(LIVE_URL)).await.unwrap();
        let account = unique("alice");
        let id1 = unique("id1");
        let id2 = unique("id2");
        store.add(&session(&id1, &account, "wg-1")).await.unwrap();
        store.add(&session(&id2, &account, "wg-1")).await.unwrap();
        store.delete(&account, &id1).await.unwrap();

        let loc = store.get_location(&account, "").await.unwrap();
        assert_eq!(loc.channel_id, id2);
        assert!(matches!(store.get(&id1).await, Err(SessionError::NotFound)));
        let s2 = store.get(&id2).await.unwrap();
        assert_eq!(s2.channel_id, id2);
    }
}
