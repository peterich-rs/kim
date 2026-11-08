use kim_naming::DefaultRegistration;
use kim_protocol::pkt::Header;

pub trait Selector: Send + Sync {
    fn lookup(&self, header: &Header, srvs: &[DefaultRegistration]) -> Option<String>;
}

pub struct HashSelector;

impl Selector for HashSelector {
    fn lookup(&self, header: &Header, srvs: &[DefaultRegistration]) -> Option<String> {
        if srvs.is_empty() {
            return None;
        }
        let i = crc32fast::hash(header.channel_id.as_bytes()) as usize % srvs.len();
        Some(srvs[i].service_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn r(id: &str) -> DefaultRegistration {
        DefaultRegistration {
            service_id: id.into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: "127.0.0.1".into(),
            public_port: 1,
            tags: vec![],
            meta: HashMap::new(),
        }
    }

    #[test]
    fn stable_and_empty() {
        let s = HashSelector;
        let h = Header {
            channel_id: "alice".into(),
            ..Header::default()
        };
        assert!(s.lookup(&h, &[]).is_none());
        let srvs = vec![r("a"), r("b")];
        let x = s.lookup(&h, &srvs).unwrap();
        assert_eq!(s.lookup(&h, &srvs).unwrap(), x);
    }
}
