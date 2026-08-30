import { describe, expect, it } from "vitest";
import { extensionFor, objectKey, parseMaxBytes, publicUrl } from "../src/object";

describe("object", () => {
  it("maps image content types", () => {
    expect(extensionFor("image/png")).toBe("png");
    expect(extensionFor("image/jpeg; charset=binary")).toBe("jpg");
    expect(extensionFor("application/pdf")).toBeNull();
  });

  it("builds an unguessable key under the account", () => {
    const key = objectKey("alice", "webp", new Date("2026-08-30T00:00:00Z"));
    expect(key).toMatch(
      /^alice\/2026\/08\/[0-9a-f-]{36}\.webp$/,
    );
  });

  it("joins the public base", () => {
    expect(publicUrl("https://media.kim.ainexc.com/", "a/b.jpg")).toBe(
      "https://media.kim.ainexc.com/a/b.jpg",
    );
  });

  it("caps max bytes", () => {
    expect(parseMaxBytes("100")).toBe(100);
    expect(parseMaxBytes("nope")).toBe(5 * 1024 * 1024);
  });
});
