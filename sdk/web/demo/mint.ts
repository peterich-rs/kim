/** Demo-only. Same value as `kim_protocol::DEMO_DEFAULT_SECRET`. */
export const DEMO_SECRET = "jwt-1sNzdiSgnNuxyq2g7xml2JvLArU";

function b64url(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) {
    bin += String.fromCharCode(b);
  }
  return btoa(bin).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}

function encodePart(obj: object): string {
  return b64url(new TextEncoder().encode(JSON.stringify(obj)));
}

/** HS256 JWT. Prefers Royal `POST /api/{app}/token` when not on the Vite demo port. */
export async function mintToken(account: string, secret = DEMO_SECRET): Promise<string> {
  const params = new URLSearchParams(location.search);
  const app = params.get("app")?.trim() || "kim";
  const remote =
    params.get("tokenUrl")?.trim() ||
    (location.port === "5173" || location.port === "5174" ? "" : location.origin);
  if (remote) {
    const r = await fetch(`${remote.replace(/\/$/u, "")}/api/${encodeURIComponent(app)}/token`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ account }),
    });
    if (!r.ok) {
      throw new Error(`token ${r.status}`);
    }
    const body = (await r.json()) as { token?: string };
    if (!body.token) {
      throw new Error("token missing");
    }
    return body.token;
  }
  const header = encodePart({ alg: "HS256", typ: "JWT" });
  const payload = encodePart({
    acc: account,
    app: "kim",
    exp: Math.floor(Date.now() / 1000) + 86400,
  });
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
  return `${header}.${payload}.${b64url(new Uint8Array(sig))}`;
}
