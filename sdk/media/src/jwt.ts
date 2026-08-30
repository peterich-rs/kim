export type Claims = {
  acc: string;
  app: string;
  exp: number;
  jti?: string;
};

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

function bytesToB64url(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) {
    bin += String.fromCharCode(b);
  }
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

export async function signHs256(claims: Claims, secret: string): Promise<string> {
  const header = bytesToB64url(
    new TextEncoder().encode(JSON.stringify({ alg: "HS256", typ: "JWT" })),
  );
  const payload = bytesToB64url(new TextEncoder().encode(JSON.stringify(claims)));
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(`${header}.${payload}`),
  );
  return `${header}.${payload}.${bytesToB64url(new Uint8Array(sig))}`;
}

export async function verifyHs256(
  token: string,
  secret: string,
  app: string,
  nowSecs = Math.floor(Date.now() / 1000),
): Promise<Claims> {
  const parts = token.split(".");
  if (parts.length !== 3 || !parts[0] || !parts[1] || !parts[2]) {
    throw new Error("invalid token");
  }
  const [header, payload, sig] = parts;
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  const ok = await crypto.subtle.verify(
    "HMAC",
    key,
    b64urlToBytes(sig) as BufferSource,
    new TextEncoder().encode(`${header}.${payload}`),
  );
  if (!ok) {
    throw new Error("bad signature");
  }
  const claims = JSON.parse(
    new TextDecoder().decode(b64urlToBytes(payload)),
  ) as Claims;
  if (typeof claims.acc !== "string" || claims.acc.length === 0) {
    throw new Error("token missing acc");
  }
  if (claims.app !== app) {
    throw new Error("token app mismatch");
  }
  if (typeof claims.exp !== "number" || claims.exp <= nowSecs) {
    throw new Error("expired");
  }
  return claims;
}

export function bearerToken(header: string | null): string {
  if (!header) {
    return "";
  }
  const v = header.trim();
  if (v.toLowerCase().startsWith("bearer ")) {
    return v.slice(7).trim();
  }
  return "";
}
