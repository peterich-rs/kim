import { afterEach, describe, expect, it, vi } from "vitest";
import { clearPasswordSealCache, login, logout, register } from "../app/lib/auth.ts";
import { insecureAuthOriginReason } from "../app/lib/secure_origin.ts";

afterEach(() => {
  clearPasswordSealCache();
  vi.unstubAllGlobals();
});

function mockFetchSequence(
  handlers: Array<(url: string, init?: RequestInit) => Promise<unknown> | unknown>,
) {
  let i = 0;
  const fetchFn = vi.fn(async (url: string, init?: RequestInit) => {
    const handler = handlers[i] ?? handlers[handlers.length - 1];
    i += 1;
    return handler(String(url), init);
  });
  vi.stubGlobal("fetch", fetchFn);
  return fetchFn;
}

describe("auth http", () => {
  it("login maps 401 text", async () => {
    mockFetchSequence([
      () => ({ ok: false, status: 404, text: async () => "" }),
      () => ({
        ok: false,
        status: 401,
        text: async () => "账号或密码错误",
      }),
    ]);
    await expect(login("alice", "nope")).rejects.toMatchObject({
      status: 401,
      message: "账号或密码错误",
    });
  });

  it("register maps 409", async () => {
    mockFetchSequence([
      () => ({ ok: false, status: 404, text: async () => "" }),
      () => ({
        ok: false,
        status: 409,
        text: async () => "账号已存在",
      }),
    ]);
    await expect(register("alice", "secret123")).rejects.toMatchObject({
      status: 409,
    });
  });

  it("logout sends bearer and ignores 401", async () => {
    const fetchFn = vi.fn().mockResolvedValue({ ok: false, status: 401, text: async () => "" });
    vi.stubGlobal("fetch", fetchFn);
    await logout("tok");
    expect(fetchFn.mock.calls[0]?.[0]).toBe("/api/v1/auth/logout");
    expect(fetchFn.mock.calls[0]?.[1]).toMatchObject({
      method: "POST",
      headers: { Authorization: "Bearer tok" },
    });
  });

  it("surfaces network errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    await expect(login("alice", "secret123")).rejects.toThrow("offline");
  });

  it("register request path and content-type", async () => {
    const fetchFn = mockFetchSequence([
      () => ({ ok: false, status: 404, text: async () => "" }),
      () => ({
        ok: false,
        status: 400,
        text: async () => "invalid account",
      }),
    ]);
    await expect(register("ab", "secret123")).rejects.toMatchObject({ status: 400 });
    expect(fetchFn.mock.calls[0]?.[0]).toBe("/api/v1/auth/password-key");
    expect(fetchFn.mock.calls[1]?.[0]).toBe("/api/v1/auth/register");
    expect(fetchFn.mock.calls[1]?.[1]?.headers).toMatchObject({
      "Content-Type": "application/x-protobuf",
    });
    expect(fetchFn.mock.calls[1]?.[1]?.body).toBeInstanceOf(Uint8Array);
  });
});

describe("secure origin", () => {
  it("rejects non-localhost http", () => {
    expect(insecureAuthOriginReason("http://evil.example")).toBeTruthy();
    expect(insecureAuthOriginReason("https://kim.ainexc.com")).toBeNull();
    expect(insecureAuthOriginReason("http://127.0.0.1:8080")).toBeNull();
    expect(insecureAuthOriginReason("http://localhost:8080")).toBeNull();
  });
});
