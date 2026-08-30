import { afterEach, describe, expect, it, vi } from "vitest";
import { uploadImage } from "../src/media";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("uploadImage", () => {
  it("posts raw bytes with the JWT", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo, init?: RequestInit) => {
        expect(String(input)).toBe("https://upload.kim.ainexc.com/v1/objects");
        expect(init?.method).toBe("POST");
        expect((init?.headers as Record<string, string>).Authorization).toBe(
          "Bearer tok",
        );
        return new Response(
          JSON.stringify({
            key: "alice/a.png",
            url: "https://media.kim.ainexc.com/alice/a.png",
            contentType: "image/png",
            bytes: 3,
          }),
          { status: 201 },
        );
      }),
    );
    const got = await uploadImage("tok", new Uint8Array([1, 2, 3]), {
      contentType: "image/png",
    });
    expect(got.url).toContain("media.kim.ainexc.com");
    expect(got.bytes).toBe(3);
  });
});
