use bytes::{BufMut, Bytes, BytesMut};
use prost::Message;

use crate::error::ProtocolError;
use crate::magic::MAGIC_LOGIC_PKT;
use crate::pkt::Header;
use crate::wire::service_name;

pub struct LogicPkt {
    pub header: Header,
    pub body: Bytes,
}

impl LogicPkt {
    pub fn new(command: impl Into<String>, sequence: u32, body: Bytes) -> Self {
        let mut header = Header::default();
        header.command = command.into();
        header.sequence = sequence;
        header.body_length = body.len() as u32;
        Self { header, body }
    }

    pub fn service_name(&self) -> &str {
        service_name(&self.header.command)
    }

    pub fn set_meta(&mut self, key: &str, value: &str) {
        self.header.meta.retain(|m| m.key != key);
        self.header.meta.push(crate::pkt::Meta {
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.header
            .meta
            .iter()
            .find(|m| m.key == key)
            .map(|m| m.value.as_str())
    }

    pub fn del_meta(&mut self, key: &str) {
        self.header.meta.retain(|m| m.key != key);
    }

    pub fn encode(&self) -> Bytes {
        let header = self.header.encode_to_vec();
        let mut buf = BytesMut::with_capacity(8 + header.len() + self.body.len());
        buf.extend_from_slice(&MAGIC_LOGIC_PKT);
        buf.put_u32(header.len() as u32);
        buf.extend_from_slice(&header);
        buf.extend_from_slice(&self.body);
        buf.freeze()
    }

    pub fn decode(rest: &[u8]) -> Result<Self, ProtocolError> {
        if rest.len() < 4 {
            return Err(ProtocolError::Incomplete);
        }
        let header_len = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        if rest.len() < 4 + header_len {
            return Err(ProtocolError::Incomplete);
        }
        let header = Header::decode(&rest[4..4 + header_len])?;
        let body_start = 4 + header_len;
        let need = header.body_length as usize;
        if rest.len() < body_start + need {
            return Err(ProtocolError::Incomplete);
        }
        if rest.len() > body_start + need {
            tracing_optional_extra(rest.len() - (body_start + need));
        }
        let body = Bytes::copy_from_slice(&rest[body_start..body_start + need]);
        Ok(Self { header, body })
    }
}

fn tracing_optional_extra(_n: usize) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, Packet};

    #[test]
    fn logic_roundtrip() {
        let pkt = LogicPkt::new("chat.demo.echo", 7, Bytes::from_static(b"hi"));
        let bytes = pkt.encode();
        match read(&bytes).unwrap() {
            Packet::Logic(got) => {
                assert_eq!(got.header.command, "chat.demo.echo");
                assert_eq!(got.header.sequence, 7);
                assert_eq!(got.header.body_length, 2);
                assert_eq!(&got.body[..], b"hi");
            }
            _ => panic!("expected logic"),
        }
    }

    #[test]
    fn incomplete_header() {
        assert!(matches!(
            LogicPkt::decode(&[0, 0, 0, 10]),
            Err(ProtocolError::Incomplete)
        ));
    }

    #[test]
    fn meta_replace() {
        let mut pkt = LogicPkt::new("chat.demo.echo", 1, Bytes::new());
        pkt.set_meta("dest.server", "a");
        pkt.set_meta("dest.server", "b");
        assert_eq!(pkt.get_meta("dest.server"), Some("b"));
        assert_eq!(
            pkt.header
                .meta
                .iter()
                .filter(|m| m.key == "dest.server")
                .count(),
            1
        );
        pkt.del_meta("dest.server");
        assert!(pkt.get_meta("dest.server").is_none());
    }
}
