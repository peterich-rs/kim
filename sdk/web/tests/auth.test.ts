import { describe, expect, it, vi } from "vitest";
import { login, logout, register } from "../app/lib/auth.ts";

describe("auth http", () => {
  it("login maps 401 text", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        text: async () => "账号或密码错误",
      }),
    );
    await expect(login("alice", "nope")).rejects.toMatchObject({
      status: 401,
      message: "账号或密码错误",
    });
    vi.unstubAllGlobals();
  });

  it("register maps 409", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 409,
        text: async () => "账号已存在",
      }),
    );
    await expect(register("alice", "secret123")).rejects.toMatchObject({
      status: 409,
    });
    vi.unstubAllGlobals();
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
    vi.unstubAllGlobals();
  });

  it("surfaces network errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    await expect(login("alice", "secret123")).rejects.toThrow("offline");
    vi.unstubAllGlobals();
  });

  it("register request path and content-type", async () => {
    const fetchFn = vi.fn().mockResolvedValue({
      ok: false,
      status: 400,
      text: async () => "invalid account",
    });
    vi.stubGlobal("fetch", fetchFn);
    await expect(register("ab", "secret123")).rejects.toMatchObject({ status: 400 });
    expect(fetchFn.mock.calls[0]?.[0]).toBe("/api/v1/auth/register");
    expect(fetchFn.mock.calls[0]?.[1]?.headers).toMatchObject({
      "Content-Type": "application/x-protobuf",
    });
    expect(fetchFn.mock.calls[0]?.[1]?.body).toBeInstanceOf(Uint8Array);
    vi.unstubAllGlobals();
  });
});
