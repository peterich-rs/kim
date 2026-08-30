import { describe, expect, it } from "vitest";

import { COPY } from "../app/copy.ts";
import { mapUserError, mapWireStatus } from "../app/lib/errors.ts";
import { validateAccount, validateConfirm, validatePassword } from "../app/lib/validation.ts";

describe("validation", () => {
  it("accepts accounts that match Royal rules", () => {
    expect(validateAccount("alice")).toBeUndefined();
    expect(validateAccount("a_b1")).toBeUndefined();
  });

  it("rejects short or illegal accounts", () => {
    expect(validateAccount("ab")).toBe(COPY.invalidAccount);
    expect(validateAccount("alice-1")).toBe(COPY.invalidAccount);
  });

  it("rejects short passwords", () => {
    expect(validatePassword("secret")).toBe(COPY.invalidPassword);
    expect(validatePassword("secret12")).toBeUndefined();
  });

  it("requires matching confirmation", () => {
    expect(validateConfirm("secret12", "secret12")).toBeUndefined();
    expect(validateConfirm("secret12", "secret13")).toBe(COPY.mismatch);
  });
});

describe("mapUserError", () => {
  it("maps known auth failures to product copy", () => {
    expect(mapUserError(Object.assign(new Error("账号或密码错误"), { status: 401 }))).toBe(
      COPY.badCredentials,
    );
    expect(mapUserError(Object.assign(new Error("账号已存在"), { status: 409 }))).toBe(
      COPY.accountExists,
    );
    expect(mapUserError(new Error("invalid account"))).toBe(COPY.invalidAccount);
    expect(mapUserError(new Error("login timeout (chat unreachable?)"))).toBe(COPY.timeout);
    expect(mapUserError(new Error("login status 105"))).toBe(COPY.authFailed);
    expect(mapUserError(new Error("connection closed before login"))).toBe(COPY.network);
    expect(mapUserError(new Error("websocket error"))).toBe(COPY.network);
    expect(mapUserError(new Error("login failed"))).toBe(COPY.wsFailed);
    expect(mapUserError(new Error("login status 3"))).toBe(COPY.wsFailed);
    expect(mapUserError(new Error("login status 99"))).toBe(COPY.wsFailed);
  });

  it("does not leak raw technical messages", () => {
    expect(mapUserError(new Error("http 502"))).toBe(COPY.unavailable);
    expect(mapUserError(new Error("ECONNREFUSED"))).toBe(COPY.unavailable);
  });
});

describe("mapWireStatus", () => {
  it("maps talk and social statuses to product copy", () => {
    expect(mapWireStatus(109)).toBe(COPY.notFriends);
    expect(mapWireStatus(110)).toBe(COPY.blocked);
    expect(mapWireStatus(108)).toBe(COPY.userNotFound);
    expect(mapWireStatus(101)).toBe(COPY.cannotAddSelf);
    expect(mapWireStatus(0)).toBeUndefined();
  });
});
