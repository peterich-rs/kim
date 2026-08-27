use bytes::{BufMut, Bytes, BytesMut};

use crate::error::ProtocolError;
use crate::magic::MAGIC_BASIC_PKT;

pub const CODE_PING: u16 = 1;
pub const CODE_PONG: u16 = 2;
const MAX_BASIC_BODY: usize = 4096;

pub struct BasicPkt {
    pub code: u16,
    pub body: Bytes,
}

impl BasicPkt {
    pub fn ping() -> Self {
        Self {
            code: CODE_PING,
            body: Bytes::new(),
        }
    }

    pub fn pong() -> Self {
        Self {
            code: CODE_PONG,
            body: Bytes::new(),
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(8 + self.body.len());
        buf.extend_from_slice(&MAGIC_BASIC_PKT);
        buf.put_u16_le(self.code);
        buf.put_u16_le(self.body.len() as u16);
        buf.extend_from_slice(&self.body);
        buf.freeze()
    }

    pub fn decode(rest: &[u8]) -> Result<Self, ProtocolError> {
        if rest.len() < 4 {
            return Err(ProtocolError::Incomplete);
        }
        let code = u16::from_le_bytes([rest[0], rest[1]]);
        let length = u16::from_le_bytes([rest[2], rest[3]]) as usize;
        if length > MAX_BASIC_BODY {
            return Err(ProtocolError::BasicTooLarge(length));
        }
        if rest.len() < 4 + length {
            return Err(ProtocolError::Incomplete);
        }
        Ok(Self {
            code,
            body: Bytes::copy_from_slice(&rest[4..4 + length]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read, Packet};

    #[test]
    fn ping_roundtrip_empty_is_8_bytes() {
        let bytes = BasicPkt::ping().encode();
        assert_eq!(bytes.len(), 8);
        match read(&bytes).unwrap() {
            Packet::Basic(p) => {
                assert_eq!(p.code, CODE_PING);
                assert!(p.body.is_empty());
            }
            _ => panic!("expected basic"),
        }
    }
}
