use std::sync::Arc;

use dashmap::mapref::entry::Entry;
use dashmap::DashMap;

use crate::{Channel, Error};

/// 当前进程里所有活着的连接。
///
/// 取连接时先 clone 出 Channel（里面是 Sender），再 await 写网络。
/// 不要把 dashmap `Ref` 拿过 await——持守卫再碰同分片会死锁。
/// API 全部同步、全部 clone-out。
///
/// [`all`] 是弱一致快照：迭代期间其他分片的增删可见，不阻塞写入。
/// 只给 shutdown drain 用，不要拿它做热路径广播。
#[derive(Clone, Default)]
pub struct ChannelMap {
    inner: Arc<DashMap<Arc<str>, Channel>>,
}

impl ChannelMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Occupies `channel.id()`. Returns [`Error::ChannelExists`] if that id is
    /// already in the table; the caller still owns `channel` (via a prior clone)
    /// and must close it.
    pub fn add(&self, channel: Channel) -> Result<(), Error> {
        let id = channel.id_arc();
        match self.inner.entry(id.clone()) {
            Entry::Occupied(_) => Err(Error::ChannelExists(id.to_string())),
            Entry::Vacant(v) => {
                v.insert(channel);
                Ok(())
            }
        }
    }

    pub fn remove(&self, id: &str) -> Option<Channel> {
        self.inner.remove(id).map(|(_, v)| v)
    }

    pub fn get(&self, id: &str) -> Option<Channel> {
        self.inner.get(id).map(|r| r.value().clone())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains_key(id)
    }

    /// Weakly consistent snapshot. Other shards may mutate while this iterates.
    pub fn all(&self) -> Vec<Channel> {
        self.inner.iter().map(|r| r.value().clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChannelOpts, Conn, Error, Frame, OpCode};
    use async_trait::async_trait;
    use bytes::Bytes;

    struct NullConn;

    #[async_trait]
    impl Conn for NullConn {
        async fn read_frame(&mut self) -> Result<Frame, Error> {
            std::future::pending().await
        }
        async fn write_frame(&mut self, _opcode: OpCode, _payload: Bytes) -> Result<(), Error> {
            Ok(())
        }
        async fn flush(&mut self) -> Result<(), Error> {
            Ok(())
        }
        async fn shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }

    fn dummy_channel(id: &str) -> Channel {
        let (ch, _read_loop) = Channel::pair(id, NullConn, NullConn, ChannelOpts::default());
        ch
    }

    #[tokio::test]
    async fn add_get_remove() {
        let map = ChannelMap::new();
        assert!(map.is_empty());
        map.add(dummy_channel("a")).unwrap();
        map.add(dummy_channel("b")).unwrap();
        assert!(map.contains("a"));
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert_eq!(map.get("a").unwrap().id(), "a");
        map.remove("a");
        assert!(!map.contains("a"));
        assert_eq!(map.len(), 1);
    }

    #[tokio::test]
    async fn add_same_id_fails_without_replacing() {
        let map = ChannelMap::new();
        map.add(dummy_channel("a")).unwrap();
        let dup = dummy_channel("a");
        let err = map.add(dup.clone()).unwrap_err();
        assert!(matches!(err, Error::ChannelExists(id) if id == "a"));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("a").unwrap().id(), "a");
        dup.close().await;
    }
}
