import { createHmac } from "node:crypto";

/** Same value as `kim_protocol::DEMO_DEFAULT_SECRET`. Tests / e2e only. */
export const DEMO_SECRET = "jwt-1sNzdiSgnNuxyq2g7xml2JvLArU";

export function mintToken(
  acc: string,
  exp = Math.floor(Date.now() / 1000) + 86400,
): string {
  const enc = (obj: object) =>
    Buffer.from(JSON.stringify(obj)).toString("base64url");
  const header = enc({ alg: "HS256", typ: "JWT" });
  const payload = enc({ acc, app: "kim", exp });
  const sig = createHmac("sha256", DEMO_SECRET)
    .update(`${header}.${payload}`)
    .digest("base64url");
  return `${header}.${payload}.${sig}`;
}
