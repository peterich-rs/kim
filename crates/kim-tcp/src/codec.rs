use bytes::{Buf, BufMut, BytesMut};

use kim_core::{Error, Frame, OpCode};

/// TCP 帧：
///
/// ```text
/// | opcode 1B | length 4B little-endian | payload N B |
/// ```
///
/// TCP 是一条字节流，一次 read 可能拿到半个包或两个粘在一起的包。
/// 这里用缓冲区攒字节，凑齐一帧再交给上层。
pub const HEADER_LEN: usize = 5;
pub const MAX_PAYLOAD: usize = 1024 * 1024;

pub fn encode_frame(opcode: OpCode, payload: &[u8]) -> BytesMut {
    let mut buf = BytesMut::with_capacity(HEADER_LEN + payload.len());
    buf.put_u8(u8::from(opcode));
    buf.put_u32_le(payload.len() as u32);
    buf.extend_from_slice(payload);
    buf
}

/// 从缓冲里尽量切出一帧。
///
/// - `Ok(None)`：字节还不够，继续读。
/// - `Ok(Some)`：切出完整一帧，缓冲里可能还剩下一帧的开头。
pub fn decode_frame(buf: &mut BytesMut) -> Result<Option<Frame>, Error> {
    if buf.len() < HEADER_LEN {
        return Ok(None);
    }

    let len = peek_len(buf);
    if len > MAX_PAYLOAD {
        return Err(Error::FrameTooLarge {
            size: len,
            max: MAX_PAYLOAD,
        });
    }
    if buf.len() < HEADER_LEN + len {
        return Ok(None);
    }

    let opcode_raw = buf.get_u8();
    let opcode = OpCode::from_u8(opcode_raw)
        .ok_or_else(|| Error::other(format!("unknown opcode {opcode_raw}")))?;
    let _len = buf.get_u32_le();
    let payload = buf.split_to(len).freeze();
    Ok(Some(Frame { opcode, payload }))
}

fn peek_len(buf: &BytesMut) -> usize {
    u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn roundtrip() {
        let encoded = encode_frame(OpCode::Binary, b"hello");
        let mut buf = encoded;
        let frame = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(frame.opcode, OpCode::Binary);
        assert_eq!(frame.payload, Bytes::from_static(b"hello"));
        assert!(buf.is_empty());
    }

    #[test]
    fn half_packet_then_rest() {
        let encoded = encode_frame(OpCode::Binary, b"abcdef");
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&encoded[..3]);
        assert!(decode_frame(&mut buf).unwrap().is_none());
        buf.extend_from_slice(&encoded[3..]);
        let frame = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(&frame.payload[..], b"abcdef");
    }

    #[test]
    fn two_frames_stuck_together() {
        let mut buf = encode_frame(OpCode::Binary, b"one");
        buf.extend_from_slice(&encode_frame(OpCode::Binary, b"two"));
        let a = decode_frame(&mut buf).unwrap().unwrap();
        let b = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(&a.payload[..], b"one");
        assert_eq!(&b.payload[..], b"two");
        assert!(buf.is_empty());
    }

    #[test]
    fn rejects_oversized() {
        let mut buf = BytesMut::new();
        buf.put_u8(u8::from(OpCode::Binary));
        buf.put_u32_le((MAX_PAYLOAD as u32) + 1);
        match decode_frame(&mut buf) {
            Err(Error::FrameTooLarge { .. }) => {}
            other => panic!("expected FrameTooLarge, got {other:?}"),
        }
    }
}
