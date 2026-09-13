import { describe, expect, it } from "vitest";
import {
  contentTypeFor,
  extensionFor,
  objectKey,
  parseMaxBytes,
  publicUrl,
  resolveExtension,
  sha256Hex,
  sniffExtension,
} from "../src/object";

describe("object", () => {
  it("maps image content types", () => {
    expect(extensionFor("image/png")).toBe("png");
    expect(extensionFor("image/jpeg; charset=binary")).toBe("jpg");
    expect(extensionFor("application/pdf")).toBeNull();
    expect(contentTypeFor("png")).toBe("image/png");
    expect(contentTypeFor("jpg")).toBe("image/jpeg");
  });

  it("builds a content-addressed key", () => {
    const hash = "a".repeat(64);
    expect(objectKey(hash, "webp")).toBe(`${hash}.webp`);
  });

  it("sniffs magic bytes over a lying Content-Type", () => {
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    expect(sniffExtension(png.buffer)).toBe("png");
    expect(resolveExtension("image/jpeg", png.buffer)).toBe("png");
    expect(resolveExtension("image/webp", new Uint8Array([1, 2, 3]).buffer)).toBe(
      "webp",
    );
  });

  it("hashes bytes with sha256", async () => {
    const hex = await sha256Hex(new Uint8Array([1, 2, 3]).buffer);
    expect(hex).toMatch(/^[0-9a-f]{64}$/);
    expect(hex).toBe(
      "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81",
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
