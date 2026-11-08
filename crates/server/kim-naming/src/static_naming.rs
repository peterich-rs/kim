use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::naming::{Error, Naming};
use crate::registration::DefaultRegistration;

type Callback = Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>;

#[derive(Default)]
pub struct StaticNaming {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    by_id: HashMap<String, DefaultRegistration>,
    subs: HashMap<String, Vec<Callback>>,
}

impl StaticNaming {
    pub fn from_slice(regs: Vec<DefaultRegistration>) -> Self {
        let mut by_id = HashMap::new();
        for r in regs {
            by_id.insert(r.service_id.clone(), r);
        }
        Self {
            inner: Mutex::new(Inner {
                by_id,
                subs: HashMap::new(),
            }),
        }
    }

    fn snapshot(inner: &Inner, service_name: &str) -> Vec<DefaultRegistration> {
        inner
            .by_id
            .values()
            .filter(|r| r.service_name == service_name)
            .cloned()
            .collect()
    }

    fn tag_match(reg: &DefaultRegistration, tags: &[&str]) -> bool {
        tags.iter().all(|t| reg.tags.iter().any(|x| x == t))
    }

    pub async fn insert(&self, reg: DefaultRegistration) {
        let mut inner = self.inner.lock().await;
        let name = reg.service_name.clone();
        inner.by_id.insert(reg.service_id.clone(), reg);
        let list = Self::snapshot(&inner, &name);
        let cbs = inner.subs.get(&name).cloned().unwrap_or_default();
        drop(inner);
        for cb in cbs {
            cb(list.clone());
        }
    }
}

#[async_trait]
impl Naming for StaticNaming {
    async fn find(
        &self,
        service_name: &str,
        tags: &[&str],
    ) -> Result<Vec<DefaultRegistration>, Error> {
        let inner = self.inner.lock().await;
        let mut list = Self::snapshot(&inner, service_name);
        if !tags.is_empty() {
            list.retain(|r| Self::tag_match(r, tags));
        }
        Ok(list)
    }

    async fn subscribe(
        &self,
        service_name: &str,
        callback: Arc<dyn Fn(Vec<DefaultRegistration>) + Send + Sync>,
    ) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        inner
            .subs
            .entry(service_name.to_string())
            .or_default()
            .push(callback);
        Ok(())
    }

    async fn unsubscribe(&self, service_name: &str) -> Result<(), Error> {
        self.inner.lock().await.subs.remove(service_name);
        Ok(())
    }

    async fn register(&self, service: DefaultRegistration) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        let name = service.service_name.clone();
        inner.by_id.insert(service.service_id.clone(), service);
        let list = Self::snapshot(&inner, &name);
        let cbs = inner.subs.get(&name).cloned().unwrap_or_default();
        drop(inner);
        for cb in cbs {
            cb(list.clone());
        }
        Ok(())
    }

    async fn deregister(&self, service_id: &str) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        let Some(old) = inner.by_id.remove(service_id) else {
            return Ok(());
        };
        let name = old.service_name;
        let list = Self::snapshot(&inner, &name);
        let cbs = inner.subs.get(&name).cloned().unwrap_or_default();
        drop(inner);
        for cb in cbs {
            cb(list.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(id: &str, name: &str) -> DefaultRegistration {
        DefaultRegistration {
            service_id: id.into(),
            service_name: name.into(),
            protocol: "tcp".into(),
            public_address: "127.0.0.1".into(),
            public_port: 1,
            tags: vec![],
            meta: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn find_snapshot_subscribe_later() {
        let n = StaticNaming::from_slice(vec![reg("a", "chat")]);
        assert_eq!(n.find("chat", &[]).await.unwrap().len(), 1);
        let seen = Arc::new(std::sync::Mutex::new(0usize));
        let seen2 = seen.clone();
        n.subscribe(
            "chat",
            Arc::new(move |list| {
                *seen2.lock().unwrap() = list.len();
            }),
        )
        .await
        .unwrap();
        assert_eq!(*seen.lock().unwrap(), 0);
        n.insert(reg("b", "chat")).await;
        assert_eq!(*seen.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn find_honors_tags_and() {
        let mut a = reg("a", "wgateway");
        a.tags = vec!["IDC:local".into()];
        let mut b = reg("b", "wgateway");
        b.tags = vec!["IDC:hk".into()];
        let n = StaticNaming::from_slice(vec![a, b]);
        assert_eq!(n.find("wgateway", &[]).await.unwrap().len(), 2);
        assert_eq!(n.find("wgateway", &["IDC:local"]).await.unwrap().len(), 1);
        assert!(n
            .find("wgateway", &["IDC:missing"])
            .await
            .unwrap()
            .is_empty());
    }
}
