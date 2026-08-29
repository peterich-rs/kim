function b64urlToBytes(s: string): Uint8Array {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/");
  const pad = b64.length % 4 === 0 ? "" : "=".repeat(4 - (b64.length % 4));
  const bin = atob(b64 + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) {
    out[i] = bin.charCodeAt(i);
  }
  return out;
}

/**
 * Read `acc` from a JWT payload. Does not verify the signature; the gateway
 * does. The token is already in the client's hands.
 */
export function accountFromToken(token: string): string {
  const parts = token.split(".");
  const payload = parts[1];
  if (parts.length < 2 || !payload) {
    throw new Error("invalid token");
  }
  const json = new TextDecoder().decode(b64urlToBytes(payload));
  const body = JSON.parse(json) as { acc?: unknown };
  if (typeof body.acc !== "string" || body.acc.length === 0) {
    throw new Error("token missing acc");
  }
  return body.acc;
}
