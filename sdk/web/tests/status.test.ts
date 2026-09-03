import { describe, expect, it } from "vitest";

import { isRetryable, KIMStatus, needsRelogin, Status } from "../src/status.ts";

describe("isRetryable", () => {
  it("retries ServiceUnavailable and the 3xx class", () => {
    expect(isRetryable(Status.ServiceUnavailable)).toBe(true);
    expect(isRetryable(Status.NoDestination)).toBe(true);
    expect(isRetryable(301)).toBe(true);
  });

  it("does not retry persist-first 99, 1xx, session loss, or client-only codes", () => {
    expect(isRetryable(Status.Success)).toBe(false);
    expect(isRetryable(Status.InvalidPacket)).toBe(false);
    expect(isRetryable(Status.CommandNotFound)).toBe(false);
    expect(isRetryable(Status.SystemException)).toBe(false);
    expect(isRetryable(Status.InvalidPacketBody)).toBe(false);
    expect(isRetryable(Status.Unauthorized)).toBe(false);
    expect(isRetryable(Status.ContentBlocked)).toBe(false);
    expect(isRetryable(Status.NotGroupMember)).toBe(false);
    expect(isRetryable(Status.UserNotFound)).toBe(false);
    expect(isRetryable(Status.NotFriends)).toBe(false);
    expect(isRetryable(Status.Blocked)).toBe(false);
    expect(isRetryable(Status.IdempotencyConflict)).toBe(false);
    expect(isRetryable(Status.SessionNotFound)).toBe(false);
    expect(isRetryable(KIMStatus.RequestTimeout)).toBe(false);
    expect(isRetryable(KIMStatus.SendFailed)).toBe(false);
  });
});

describe("needsRelogin", () => {
  it("is 4xx session loss, not ServiceUnavailable", () => {
    expect(needsRelogin(Status.SessionNotFound)).toBe(true);
    expect(needsRelogin(Status.ServiceUnavailable)).toBe(false);
    expect(needsRelogin(Status.SystemException)).toBe(false);
  });
});
