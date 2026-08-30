import { describe, expect, it } from "vitest";
import { bearerToken, signHs256, verifyHs256 } from "../src/jwt";

const secret = "test-secret";

describe("jwt", () => {
  it("round-trips HS256 claims", async () => {
    const token = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) + 60 },
      secret,
    );
    const claims = await verifyHs256(token, secret, "kim");
    expect(claims.acc).toBe("alice");
    expect(claims.app).toBe("kim");
  });

  it("rejects the wrong app and expired tokens", async () => {
    const token = await signHs256(
      { acc: "alice", app: "other", exp: Math.floor(Date.now() / 1000) + 60 },
      secret,
    );
    await expect(verifyHs256(token, secret, "kim")).rejects.toThrow(/app/);
    const old = await signHs256(
      { acc: "alice", app: "kim", exp: Math.floor(Date.now() / 1000) - 10 },
      secret,
    );
    await expect(verifyHs256(old, secret, "kim")).rejects.toThrow(/expired/);
  });

  it("parses Bearer", () => {
    expect(bearerToken("Bearer abc")).toBe("abc");
    expect(bearerToken("bearer abc")).toBe("abc");
    expect(bearerToken(null)).toBe("");
  });
});
