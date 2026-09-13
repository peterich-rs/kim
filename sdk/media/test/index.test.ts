import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  vi.unstubAllGlobals();
});
import { handleRequest } from "../src/index";
import { signHs256 } from "../src/jwt";
import { objectKey, sha256Hex } from "../src/object";

type BucketHooks = {
  put?: (key: string) => void;
  head?: (key: string) => R2Object | null;
};

function env(overrides: Partial<Env> & BucketHooks = {}): Env {
  const put = overrides.put;
  const head = overrides.head;
  const keys = new Set<string>();
  return {
    APP: "kim",
    PUBLIC_BASE: "https://media.kim.ainexc.com",
    ROYAL_ORIGIN: "",
    MAX_BYTES: "1024",
    JWT_SECRET: "test-secret",
    BUCKET: {
      head: async (key: string) => {
        if (head) {
          return head(key);
        }
        return keys.has(key) ? ({ key } as R2Object) : null;
      },
      put: async (key: string) => {
        keys.add(key);
        put?.(key);
        return { key } as R2Object;
      },
    } as unknown as R2Bucket,
    ...overrides,
  };
}

describe("handleRequest", () => {
  it("uploads a content-addressed object after JWT verification", async () => {
    const token = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) + 60 },
      "test-secret",
    );
    const bodyBytes = new Uint8Array([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ]);
    const hash = await sha256Hex(bodyBytes.buffer);
    let stored = "";
    const res = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "image/png",
        },
        body: bodyBytes,
      }),
      env({ put: (k) => (stored = k) }),
    );
    expect(res.status).toBe(201);
    const body = (await res.json()) as {
      url: string;
      key: string;
      bytes: number;
      sha256: string;
      deduped: boolean;
      contentType: string;
    };
    expect(body.bytes).toBe(8);
    expect(body.sha256).toBe(hash);
    expect(body.deduped).toBe(false);
    expect(body.contentType).toBe("image/png");
    expect(body.key).toBe(objectKey(hash, "png"));
    expect(body.url).toBe(`https://media.kim.ainexc.com/${body.key}`);
    expect(body.url).not.toContain("/alice/");
    expect(stored).toBe(body.key);
  });

  it("skips put when the same bytes already exist", async () => {
    const token = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) + 60 },
      "test-secret",
    );
    // Full PNG signature so sniff wins over a lying second Content-Type.
    const bodyBytes = new Uint8Array([
      0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    ]);
    const hash = await sha256Hex(bodyBytes.buffer);
    const key = objectKey(hash, "png");
    let puts = 0;
    const shared = env({
      put: () => {
        puts += 1;
      },
    });

    const first = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "image/png",
        },
        body: bodyBytes,
      }),
      shared,
    );
    const second = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "image/jpeg",
        },
        body: bodyBytes,
      }),
      shared,
    );

    expect(first.status).toBe(201);
    expect(second.status).toBe(201);
    const a = (await first.json()) as { key: string; deduped: boolean };
    const b = (await second.json()) as { key: string; deduped: boolean; sha256: string };
    expect(a.key).toBe(key);
    expect(b.key).toBe(key);
    expect(a.deduped).toBe(false);
    expect(b.deduped).toBe(true);
    expect(b.sha256).toBe(hash);
    expect(puts).toBe(1);
  });

  it("asks Royal /me when ROYAL_ORIGIN is set", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ account: "bob", app: "kim" }))),
    );
    const bodyBytes = new Uint8Array([
      0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50,
    ]);
    const hash = await sha256Hex(bodyBytes.buffer);
    const res = await handleRequest(
      new Request("https://upload.kim.ainexc.com/v1/objects", {
        method: "POST",
        headers: {
          Authorization: "Bearer anything",
          "Content-Type": "image/webp",
        },
        body: bodyBytes,
      }),
      env({ ROYAL_ORIGIN: "https://kim.ainexc.com", JWT_SECRET: "" }),
    );
    expect(res.status).toBe(201);
    const body = (await res.json()) as { key: string; deduped: boolean };
    expect(body.key).toBe(objectKey(hash, "webp"));
    expect(body.key.includes("bob")).toBe(false);
    expect(body.deduped).toBe(false);
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
