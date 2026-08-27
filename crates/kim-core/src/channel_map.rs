use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::Channel;

/// 当前进程里所有活着的连接。
///
/// 取连接时先 clone 出 Channel（里面是 Sender），再 await 写网络，
/// 不要把整张表的锁拿到网络等待里去。
#[derive(Clone, Default)]
pub struct ChannelMap {
    inner: Arc<RwLock<HashMap<String, Channel>>>,
}

impl ChannelMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn add(&self, channel: Channel) {
        let id = channel.id().to_string();
        self.inner.write().await.insert(id, channel);
    }

    pub async fn remove(&self, id: &str) -> Option<Channel> {
        self.inner.write().await.remove(id)
    }

    pub async fn get(&self, id: &str) -> Option<Channel> {
        self.inner.read().await.get(id).cloned()
    }

    pub async fn contains(&self, id: &str) -> bool {
        self.inner.read().await.contains_key(id)
    }

    pub async fn all(&self) -> Vec<Channel> {
        self.inner.read().await.values().cloned().collect()
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
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
        map.add(dummy_channel("a")).await;
        map.add(dummy_channel("b")).await;
        assert!(map.contains("a").await);
        assert_eq!(map.len().await, 2);
        assert_eq!(map.get("a").await.unwrap().id(), "a");
        map.remove("a").await;
        assert!(!map.contains("a").await);
        assert_eq!(map.len().await, 1);
    }
}
