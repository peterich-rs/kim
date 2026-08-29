import { concat, equal4, readU16LE, readU32BE, writeU16LE, writeU32BE } from "./bytes";
import { decodeHeader, encodeHeader, type HeaderFields } from "./proto";
import { Flag, Status } from "./status";

/** Same bytes as `kim_protocol::MAGIC_LOGIC_PKT`. */
export const MAGIC_LOGIC = new Uint8Array([0xc3, 0x11, 0xa3, 0x65]);
/** Same bytes as `kim_protocol::MAGIC_BASIC_PKT`. */
export const MAGIC_BASIC = new Uint8Array([0xc3, 0x15, 0xa7, 0x65]);

export const CODE_PING = 1;
export const CODE_PONG = 2;
const MAX_BASIC_BODY = 4096;

let seq = 1;

export function nextSequence(): number {
  const n = seq;
  seq = (seq + 1) >>> 0;
  if (seq === 0) {
    seq = 1;
  }
  return n;
}

/** Tests only. */
export function resetSequence(start = 1): void {
  seq = start;
}

export class BasicPkt {
  constructor(
    public readonly code: number,
    public readonly body: Uint8Array = new Uint8Array(),
  ) {}

  static ping(): BasicPkt {
    return new BasicPkt(CODE_PING);
  }

  static pong(): BasicPkt {
    return new BasicPkt(CODE_PONG);
  }

  encode(): Uint8Array {
    if (this.body.length > MAX_BASIC_BODY) {
      throw new Error("basic body too large");
    }
    return concat([
      MAGIC_BASIC,
      writeU16LE(this.code),
      writeU16LE(this.body.length),
      this.body,
    ]);
  }

  static decode(buf: Uint8Array): BasicPkt {
    let rest = buf;
    if (equal4(buf, MAGIC_BASIC)) {
      rest = buf.subarray(4);
    }
    if (rest.length < 4) {
      throw new Error("incomplete basic packet");
    }
    const code = readU16LE(rest, 0);
    const length = readU16LE(rest, 2);
    if (length > MAX_BASIC_BODY) {
      throw new Error("basic body too large");
    }
    if (rest.length < 4 + length) {
      throw new Error("incomplete basic packet");
    }
    return new BasicPkt(code, rest.subarray(4, 4 + length));
  }
}

/**
 * LogicPkt wire (this repo, not the booklet extra payload-length field):
 * `magic(4) | header_len u32 BE | Header protobuf | body`.
 * Body length is `Header.bodyLength`.
 */
export class LogicPkt {
  command = "";
  channelId = "";
  sequence = 0;
  flag: number = Flag.Request;
  status: number = Status.Success;
  dest = "";
  payload: Uint8Array = new Uint8Array();

  static build(
    command: string,
    dest: string,
    payload: Uint8Array = new Uint8Array(),
    sequence?: number,
  ): LogicPkt {
    const pkt = new LogicPkt();
    pkt.command = command;
    pkt.dest = dest;
    pkt.payload = payload;
    pkt.sequence = sequence ?? nextSequence();
    return pkt;
  }

  static from(buf: Uint8Array): LogicPkt {
    let offset = 0;
    if (equal4(buf, MAGIC_LOGIC)) {
      offset = 4;
    }
    if (buf.length < offset + 4) {
      throw new Error("incomplete logic packet");
    }
    const hlen = readU32BE(buf, offset);
    offset += 4;
    if (buf.length < offset + hlen) {
      throw new Error("incomplete logic header");
    }
    const header = decodeHeader(buf.subarray(offset, offset + hlen));
    offset += hlen;
    const need = header.bodyLength;
    if (buf.length < offset + need) {
      throw new Error("incomplete logic body");
    }
    const pkt = new LogicPkt();
    pkt.command = header.command;
    pkt.channelId = header.channelId;
    pkt.sequence = header.sequence;
    pkt.flag = header.flag;
    pkt.status = header.status;
    pkt.dest = header.dest;
    pkt.payload = buf.subarray(offset, offset + need);
    return pkt;
  }

  bytes(): Uint8Array {
    const header: HeaderFields = {
      command: this.command,
      channelId: this.channelId,
      sequence: this.sequence,
      flag: this.flag,
      status: this.status,
      dest: this.dest,
      bodyLength: this.payload.length,
    };
    const headerBytes = encodeHeader(header);
    return concat([
      MAGIC_LOGIC,
      writeU32BE(headerBytes.length),
      headerBytes,
      this.payload,
    ]);
  }
}

export type WirePacket =
  | { kind: "basic"; pkt: BasicPkt }
  | { kind: "logic"; pkt: LogicPkt };

export function readPacket(buf: Uint8Array): WirePacket {
  if (equal4(buf, MAGIC_BASIC)) {
    return { kind: "basic", pkt: BasicPkt.decode(buf) };
  }
  if (equal4(buf, MAGIC_LOGIC)) {
    return { kind: "logic", pkt: LogicPkt.from(buf) };
  }
  throw new Error("bad magic");
}
