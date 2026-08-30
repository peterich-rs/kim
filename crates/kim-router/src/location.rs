use bytes::{BufMut, Bytes, BytesMut};

use crate::storage::SessionError;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Location {
    pub channel_id: String,
    pub gate_id: String,
    pub device: String,
}

impl Location {
    /// `| channel_len u16 LE | channel_id | gate_len u16 LE | gate_id | device_len u16 LE | device |`
    /// Device is optional on decode: truncated buffers without the third field still work.
    pub fn encode(&self) -> Bytes {
        let ch = self.channel_id.as_bytes();
        let gate = self.gate_id.as_bytes();
        let device = self.device.as_bytes();
        debug_assert!(
            ch.len() <= u16::MAX as usize
                && gate.len() <= u16::MAX as usize
                && device.len() <= u16::MAX as usize,
            "Location fields must fit in u16 LE length prefix"
        );
        if ch.len() > u16::MAX as usize
            || gate.len() > u16::MAX as usize
            || device.len() > u16::MAX as usize
        {
            tracing::error!(
                channel_len = ch.len(),
                gate_len = gate.len(),
                device_len = device.len(),
                "location id truncated to u16::MAX"
            );
        }
        let ch_len = u16::try_from(ch.len()).unwrap_or(u16::MAX);
        let gate_len = u16::try_from(gate.len()).unwrap_or(u16::MAX);
        let device_len = u16::try_from(device.len()).unwrap_or(u16::MAX);
        let ch_take = usize::from(ch_len);
        let gate_take = usize::from(gate_len);
        let device_take = usize::from(device_len);
        let mut buf = BytesMut::with_capacity(6 + ch_take + gate_take + device_take);
        buf.put_u16_le(ch_len);
        buf.extend_from_slice(&ch[..ch_take]);
        buf.put_u16_le(gate_len);
        buf.extend_from_slice(&gate[..gate_take]);
        buf.put_u16_le(device_len);
        buf.extend_from_slice(&device[..device_take]);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Result<Self, SessionError> {
        let (channel_id, rest) = read_short_string(buf)?;
        let (gate_id, rest) = read_short_string(rest)?;
        let device = if rest.is_empty() {
            String::new()
        } else {
            read_short_string(rest)?.0
        };
        Ok(Self {
            channel_id,
            gate_id,
            device,
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
    use bytes::{BufMut, BytesMut};

    #[test]
    fn encode_decode_roundtrip() {
        let loc = Location {
            channel_id: "wg-1_alice_1".into(),
            gate_id: "wg-1".into(),
            device: "web".into(),
        };
        let bytes = loc.encode();
        assert_eq!(Location::decode(&bytes).unwrap(), loc);
    }

    #[test]
    fn encode_layout_u16le_lengths() {
        let loc = Location {
            channel_id: "ab".into(),
            gate_id: "g".into(),
            device: String::new(),
        };
        let b = loc.encode();
        assert_eq!(&b[..2], &[2, 0]);
        assert_eq!(&b[2..4], b"ab");
        assert_eq!(&b[4..6], &[1, 0]);
        assert_eq!(&b[6..7], b"g");
        assert_eq!(&b[7..9], &[0, 0]);
    }

    #[test]
    fn empty_and_unicode_roundtrip() {
        let loc = Location {
            channel_id: String::new(),
            gate_id: "网关".into(),
            device: String::new(),
        };
        assert_eq!(Location::decode(&loc.encode()).unwrap(), loc);
    }

    #[test]
    fn decode_two_field_legacy_has_empty_device() {
        let mut buf = BytesMut::new();
        buf.put_u16_le(2);
        buf.extend_from_slice(b"ab");
        buf.put_u16_le(1);
        buf.extend_from_slice(b"g");
        let loc = Location::decode(&buf).unwrap();
        assert_eq!(loc.channel_id, "ab");
        assert_eq!(loc.gate_id, "g");
        assert!(loc.device.is_empty());
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
