use std::sync::LazyLock;

use ::redis::aio::ConnectionManager;
use ::redis::Client;
use async_trait::async_trait;
use kim_protocol::pkt::Session;
use kim_router::{Location, SessionError, SessionStorage};
use prost::Message;
use tracing::warn;

use crate::keys::{key_location, key_session};
use crate::SESSION_TTL;

/// HASH login:loc:v2:{account} field=channel_id value=Location blob; always DEL sn.
/// Pre-hash leftovers were a STRING Location blob; drop them instead of WRONGTYPE.
const DELETE_LUA: &str = r#"
-- KEYS[1] = login:loc:v2:{account}
-- KEYS[2] = login:sn:v2:{channel_id}
-- ARGV[1] = channel_id
redis.call('DEL', KEYS[2])
local t = redis.call('TYPE', KEYS[1])
if type(t) == 'table' then t = t['ok'] end
if t == 'hash' then
  redis.call('HDEL', KEYS[1], ARGV[1])
  if redis.call('HLEN', KEYS[1]) == 0 then
    redis.call('DEL', KEYS[1])
  end
elseif t ~= 'none' then
  redis.call('DEL', KEYS[1])
end
return 1
"#;

static DELETE_SCRIPT: LazyLock<::redis::Script> =
    LazyLock::new(|| ::redis::Script::new(DELETE_LUA));

#[derive(Clone)]
pub struct RedisSessionStore {
    conn: ConnectionManager,
}

impl RedisSessionStore {
    pub async fn open(url: &str) -> Result<Self, SessionError> {
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
        let key = key_location(account, "");
        let mut conn = self.conn.clone();
        let values: Result<Vec<Vec<u8>>, ::redis::RedisError> =
            ::redis::cmd("HVALS").arg(&key).query_async(&mut conn).await;
        let values = match values {
            Ok(v) => v,
            Err(err) if is_wrong_type(&err) => {
                warn!(%key, "dropping incompatible location key");
                let _: () = ::redis::cmd("DEL")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .map_err(redis_err)?;
                return Ok(Vec::new());
            }
            Err(err) => return Err(redis_err(err)),
        };
        let mut out = Vec::with_capacity(values.len());
        for bytes in values {
            out.push(Location::decode(&bytes)?);
        }
        Ok(out)
    }

    async fn write_session(
        &self,
        loc_key: &str,
        sn_key: &str,
        channel_id: &str,
        loc_bytes: &[u8],
        sn_bytes: &[u8],
        ttl: u64,
    ) -> Result<(), ::redis::RedisError> {
        let mut conn = self.conn.clone();
        ::redis::pipe()
            .atomic()
            .cmd("HSET")
            .arg(loc_key)
            .arg(channel_id)
            .arg(loc_bytes)
            .ignore()
            .cmd("EXPIRE")
            .arg(loc_key)
            .arg(ttl)
            .ignore()
            .cmd("SET")
            .arg(sn_key)
            .arg(sn_bytes)
            .arg("EX")
            .arg(ttl)
            .ignore()
            .query_async::<()>(&mut conn)
            .await
    }

    pub async fn count_empty_jti_locations(&self) -> Result<u64, SessionError> {
        let mut conn = self.conn.clone();
        let mut cursor: u64 = 0;
        let mut empty = 0u64;
        loop {
            let (next, keys): (u64, Vec<String>) = ::redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("login:loc:v2:*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(redis_err)?;
            for key in keys {
                let values: Result<Vec<Vec<u8>>, ::redis::RedisError> =
                    ::redis::cmd("HVALS").arg(&key).query_async(&mut conn).await;
                let values = match values {
                    Ok(v) => v,
                    Err(err) if is_wrong_type(&err) => continue,
                    Err(err) => return Err(redis_err(err)),
                };
                for bytes in values {
                    match Location::decode(&bytes) {
                        Ok(loc) if loc.jti.is_empty() => empty += 1,
                        _ => {}
                    }
                }
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(empty)
    }
}

fn loc_of(session: &Session) -> Location {
    Location {
        channel_id: session.channel_id.clone(),
        gate_id: session.gate_id.clone(),
        device: session.device.clone(),
        jti: session.jti.clone(),
    }
}

fn is_wrong_type(err: &::redis::RedisError) -> bool {
    err.kind() == ::redis::ErrorKind::TypeError || err.code() == Some("WRONGTYPE")
}

fn redis_err(e: ::redis::RedisError) -> SessionError {
    SessionError::Other(e.to_string())
}

#[async_trait]
impl SessionStorage for RedisSessionStore {
    async fn add(&self, session: &Session) -> Result<(), SessionError> {
        let loc = loc_of(session);
        let loc_key = key_location(&session.account, "");
        let sn_key = key_session(&session.channel_id);
        let loc_bytes = loc.encode();
        let sn_bytes = session.encode_to_vec();
        let ttl = SESSION_TTL.as_secs();
        match self
            .write_session(
                &loc_key,
                &sn_key,
                &session.channel_id,
                loc_bytes.as_ref(),
                &sn_bytes,
                ttl,
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(err) if is_wrong_type(&err) => {
                warn!(key = %loc_key, "replacing incompatible location key");
                let mut conn = self.conn.clone();
                let _: () = ::redis::cmd("DEL")
                    .arg(&loc_key)
                    .query_async(&mut conn)
                    .await
                    .map_err(redis_err)?;
                self.write_session(
                    &loc_key,
                    &sn_key,
                    &session.channel_id,
                    loc_bytes.as_ref(),
                    &sn_bytes,
                    ttl,
                )
                .await
                .map_err(redis_err)
            }
            Err(err) => Err(redis_err(err)),
        }
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
        if accounts.len() == 1 {
            let out = self.hash_locs(&accounts[0]).await?;
            return if out.is_empty() {
                Err(SessionError::NotFound)
            } else {
                Ok(out)
            };
        }
        let mut pipe = ::redis::pipe();
        for account in accounts {
            pipe.cmd("HVALS").arg(key_location(account, ""));
        }
        let mut conn = self.conn.clone();
        let nested: Result<Vec<Vec<Vec<u8>>>, ::redis::RedisError> =
            pipe.query_async(&mut conn).await;
        let nested = match nested {
            Ok(v) => v,
            Err(_) => {
                let mut out = Vec::new();
                for account in accounts {
                    out.extend(self.hash_locs(account).await?);
                }
                return if out.is_empty() {
                    Err(SessionError::NotFound)
                } else {
                    Ok(out)
                };
            }
        };
        let mut out = Vec::new();
        for values in nested {
            for bytes in values {
                out.push(Location::decode(&bytes)?);
            }
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

    #[test]
    fn type_error_counts_as_wrong_type() {
        let err = ::redis::RedisError::from((::redis::ErrorKind::TypeError, "WRONGTYPE"));
        assert!(is_wrong_type(&err));
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
