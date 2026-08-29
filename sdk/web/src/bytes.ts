export function readU32BE(buf: Uint8Array, offset: number): number {
  return new DataView(buf.buffer, buf.byteOffset + offset, 4).getUint32(0, false);
}

export function writeU32BE(n: number): Uint8Array {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, n >>> 0, false);
  return out;
}

export function readU16LE(buf: Uint8Array, offset: number): number {
  return buf[offset]! | (buf[offset + 1]! << 8);
}

export function writeU16LE(n: number): Uint8Array {
  const v = n & 0xffff;
  return new Uint8Array([v & 0xff, (v >> 8) & 0xff]);
}

export function concat(parts: Uint8Array[]): Uint8Array {
  let len = 0;
  for (const p of parts) len += p.length;
  const out = new Uint8Array(len);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

export function equal4(buf: Uint8Array, magic: Uint8Array): boolean {
  return (
    buf.length >= 4 &&
    buf[0] === magic[0] &&
    buf[1] === magic[1] &&
    buf[2] === magic[2] &&
    buf[3] === magic[3]
  );
}

export function toUint8(data: unknown): Uint8Array {
  if (data instanceof Uint8Array) {
    return data;
  }
  if (data instanceof ArrayBuffer) {
    return new Uint8Array(data);
  }
  if (ArrayBuffer.isView(data)) {
    const v = data as ArrayBufferView;
    return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  }
  throw new Error("expected binary websocket frame");
}

export function asBigInt(v: unknown): bigint {
  if (typeof v === "bigint") {
    return v;
  }
  if (typeof v === "number") {
    return BigInt(v);
  }
  if (typeof v === "string" && v.length > 0) {
    return BigInt(v);
  }
  if (v != null && typeof v === "object" && "toString" in v) {
    const s = String(v);
    if (s.length > 0 && s !== "[object Object]") {
      return BigInt(s);
    }
  }
  return 0n;
}
