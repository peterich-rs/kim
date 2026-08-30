import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllGlobals();
});
import { handleRequest } from "../src/index";
import { signHs256 } from "../src/jwt";

function env(overrides: Partial<Env> & { put?: (key: string) => void } = {}): Env {
  const put = overrides.put;
  return {
    APP: "kim",
    PUBLIC_BASE: "https://media.kim.ainexc.com",
    ROYAL_ORIGIN: "",
    MAX_BYTES: "1024",
    JWT_SECRET: "test-secret",
    BUCKET: {
      put: async (key: string) => {
        put?.(key);
        return { key } as R2Object;
      },
    } as unknown as R2Bucket,
    ...overrides,
  };
}

describe("handleRequest", () => {
  it("uploads after JWT verification when Royal is unset", async () => {
    const token = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) + 60 },
      "test-secret",
    );
    let stored = "";
    const res = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "image/png",
        },
        body: new Uint8Array([137, 80, 78, 71]),
      }),
      env({ put: (k) => (stored = k) }),
    );
    expect(res.status).toBe(201);
    const body = (await res.json()) as { url: string; key: string; bytes: number };
    expect(body.bytes).toBe(4);
    expect(body.url).toContain("https://media.kim.ainexc.com/alice/");
    expect(stored).toBe(body.key);
  });

  it("asks Royal /me when ROYAL_ORIGIN is set", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ account: "bob", app: "kim" }))),
    );
    const res = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: "Bearer anything",
          "Content-Type": "image/webp",
        },
        body: new Uint8Array([1, 2, 3]),
      }),
      env({ ROYAL_ORIGIN: "https://kim.ainexc.com", JWT_SECRET: "" }),
    );
    expect(res.status).toBe(201);
    const body = (await res.json()) as { key: string };
    expect(body.key.startsWith("bob/")).toBe(true);
  });

  it("rejects missing auth and bad types", async () => {
    const noAuth = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: { "Content-Type": "image/png" },
        body: new Uint8Array([1]),
      }),
      env(),
    );
    expect(noAuth.status).toBe(401);
    const token = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) + 60 },
      "test-secret",
    );
    const badType = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/pdf",
        },
        body: new Uint8Array([1]),
      }),
      env(),
    );
    expect(badType.status).toBe(415);
  });
});
