use bytes::{BufMut, Bytes, BytesMut};

use crate::storage::SessionError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub channel_id: String,
    pub gate_id: String,
}

impl Location {
    /// Booklet `endian.WriteShortBytes`. Redis loc value / Lua must parse this:
    /// `| channel_len u16 LE | channel_id UTF-8 | gate_len u16 LE | gate_id UTF-8 |`
    pub fn encode(&self) -> Bytes {
        let ch = self.channel_id.as_bytes();
        let gate = self.gate_id.as_bytes();
        debug_assert!(
            ch.len() <= u16::MAX as usize && gate.len() <= u16::MAX as usize,
            "Location channel_id/gate_id must fit in u16 LE length prefix"
        );
        if ch.len() > u16::MAX as usize || gate.len() > u16::MAX as usize {
            tracing::error!(
                channel_len = ch.len(),
                gate_len = gate.len(),
                "location id truncated to u16::MAX"
            );
        }
        let ch_len = u16::try_from(ch.len()).unwrap_or(u16::MAX);
        let gate_len = u16::try_from(gate.len()).unwrap_or(u16::MAX);
        let ch_take = usize::from(ch_len);
        let gate_take = usize::from(gate_len);
        let mut buf = BytesMut::with_capacity(4 + ch_take + gate_take);
        buf.put_u16_le(ch_len);
        buf.extend_from_slice(&ch[..ch_take]);
        buf.put_u16_le(gate_len);
        buf.extend_from_slice(&gate[..gate_take]);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, SessionError> {
        let (channel_id, rest) = read_short_string(buf)?;
        let (gate_id, _) = read_short_string(rest)?;
        Ok(Self {
            channel_id,
            gate_id,
        })
    }
}

fn read_short_string(buf: &[u8]) -> Result<(String, &[u8]), SessionError> {
    if buf.len() < 2 {
        return Err(SessionError::Other("truncated location".into()));
    }
    let n = usize::from(u16::from_le_bytes([buf[0], buf[1]]));
    let rest = &buf[2..];
    if rest.len() < n {
        return Err(SessionError::Other("truncated location".into()));
    }
    let s = std::str::from_utf8(&rest[..n])
        .map_err(|_| SessionError::Other("invalid utf-8 in location".into()))?;
    Ok((s.to_string(), &rest[n..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let loc = Location {
            channel_id: "wg-1_alice_1".into(),
            gate_id: "wg-1".into(),
        };
        let bytes = loc.encode();
        assert_eq!(Location::decode(&bytes).unwrap(), loc);
    }

    #[test]
    fn encode_layout_u16le_lengths() {
        let loc = Location {
            channel_id: "ab".into(),
            gate_id: "g".into(),
        };
        let b = loc.encode();
        assert_eq!(&b[..2], &[2, 0]);
        assert_eq!(&b[2..4], b"ab");
        assert_eq!(&b[4..6], &[1, 0]);
        assert_eq!(&b[6..], b"g");
    }

    #[test]
    fn empty_and_unicode_roundtrip() {
        let loc = Location {
            channel_id: String::new(),
            gate_id: "网关".into(),
        };
        assert_eq!(Location::decode(&loc.encode()).unwrap(), loc);
    }

    #[test]
    fn decode_truncated() {
        assert!(matches!(
            Location::decode(&[1]),
            Err(SessionError::Other(_))
        ));
        assert!(matches!(
            Location::decode(&[5, 0, b'a']),
            Err(SessionError::Other(_))
        ));
    }

    #[test]
    fn decode_invalid_utf8() {
        // channel_len=1, byte 0xff, then a valid empty gate
        let buf = [1, 0, 0xff, 0, 0];
        assert!(matches!(
            Location::decode(&buf),
            Err(SessionError::Other(_))
        ));
    }
}
