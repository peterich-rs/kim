import { bearerToken, verifyHs256 } from "./jwt";
import { extensionFor, objectKey, parseMaxBytes, publicUrl } from "./object";

const ALLOW_ORIGINS = new Set([
  "https://kim.ainexc.com",
  "http://localhost:5173",
  "http://127.0.0.1:5173",
]);

function corsHeaders(origin: string | null): HeadersInit {
  const allow = origin && ALLOW_ORIGINS.has(origin) ? origin : "https://kim.ainexc.com";
  return {
    "Access-Control-Allow-Origin": allow,
    "Access-Control-Allow-Headers": "Authorization, Content-Type",
    "Access-Control-Allow-Methods": "POST, OPTIONS",
    Vary: "Origin",
  };
}

function json(
  status: number,
  body: unknown,
  origin: string | null,
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      ...corsHeaders(origin),
    },
  });
}

async function accountOf(env: Env, authorization: string | null): Promise<string> {
  const token = bearerToken(authorization);
  if (!token) {
    throw new Error("unauthorized");
  }
  const origin = env.ROYAL_ORIGIN?.replace(/\/+$/, "") ?? "";
  if (origin) {
    const me = await fetch(`${origin}/api/v1/auth/me`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (me.status === 401) {
      throw new Error("unauthorized");
    }
    if (!me.ok) {
      throw new Error("royal unavailable");
    }
    const body = (await me.json()) as { account?: unknown };
    if (typeof body.account !== "string" || body.account.length === 0) {
      throw new Error("unauthorized");
    }
    return body.account;
  }
  const secret = env.JWT_SECRET ?? "";
  if (!secret) {
    throw new Error("media unconfigured");
  }
  const claims = await verifyHs256(token, secret, env.APP || "kim");
  return claims.acc;
}

export async function handleRequest(
  request: Request,
  env: Env,
): Promise<Response> {
  const origin = request.headers.get("Origin");
  const url = new URL(request.url);
  if (request.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders(origin) });
  }
  if (request.method === "GET" && url.pathname === "/health") {
    return json(200, { ok: true }, origin);
  }
  if (request.method !== "POST" || url.pathname !== "/v1/objects") {
    return json(404, { error: "not found" }, origin);
  }

  let account: string;
  try {
    account = await accountOf(env, request.headers.get("Authorization"));
  } catch (err) {
    const msg = err instanceof Error ? err.message : "unauthorized";
    if (msg === "media unconfigured" || msg === "royal unavailable") {
      return json(503, { error: msg }, origin);
    }
    return json(401, { error: "unauthorized" }, origin);
  }

  const contentType = request.headers.get("Content-Type") ?? "";
  const ext = extensionFor(contentType);
  if (!ext) {
    return json(415, { error: "unsupported media type" }, origin);
  }
  const max = parseMaxBytes(env.MAX_BYTES);
  const buf = await request.arrayBuffer();
  if (buf.byteLength === 0) {
    return json(400, { error: "empty body" }, origin);
  }
  if (buf.byteLength > max) {
    return json(413, { error: "too large" }, origin);
  }
  const key = objectKey(account, ext);
  const ct = contentType.split(";")[0]?.trim().toLowerCase() ?? "application/octet-stream";
  await env.BUCKET.put(key, buf, {
    httpMetadata: { contentType: ct },
    customMetadata: { acc: account },
  });
  return json(
    201,
    {
      key,
      url: publicUrl(env.PUBLIC_BASE, key),
      contentType: ct,
      bytes: buf.byteLength,
    },
    origin,
  );
}

export default {
  async fetch(request, env): Promise<Response> {
    try {
      return await handleRequest(request, env);
    } catch (err) {
      console.error("media worker failed", err);
      return json(500, { error: "internal" }, request.headers.get("Origin"));
    }
  },
} satisfies ExportedHandler<Env>;
