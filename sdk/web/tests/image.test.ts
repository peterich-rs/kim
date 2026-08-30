import { describe, expect, it } from "vitest";

import { MessageType } from "../src/command.ts";
import { COPY } from "../app/copy.ts";
import {
  bubbleSize,
  encodeImageExtra,
  isImageMessage,
  isMediaUrl,
  parseImageExtra,
  previewBody,
} from "../app/lib/image.ts";

describe("image extra", () => {
  it("round-trips width and height", () => {
    expect(parseImageExtra(encodeImageExtra(1200, 800))).toEqual({ w: 1200, h: 800 });
    expect(parseImageExtra("")).toBeUndefined();
    expect(parseImageExtra("{bad")).toBeUndefined();
    expect(parseImageExtra(JSON.stringify({ w: 0, h: 10 }))).toBeUndefined();
  });

  it("treats media host and image extensions as images", () => {
    expect(isMediaUrl("https://media.kim.ainexc.com/alice/2026/08/a.png")).toBe(true);
    expect(isMediaUrl("https://cdn.example/x.jpg")).toBe(true);
    expect(isMediaUrl("https://example.com/readme")).toBe(false);
    expect(isMediaUrl("hello")).toBe(false);
  });

  it("classifies type=2 and media URLs as image messages", () => {
    expect(isImageMessage(MessageType.Image, "https://media.kim.ainexc.com/a.png")).toBe(true);
    expect(isImageMessage(1, "hi")).toBe(false);
    expect(isImageMessage(0, "https://media.kim.ainexc.com/a.png")).toBe(true);
    expect(isImageMessage(1, "x", encodeImageExtra(10, 10))).toBe(true);
  });

  it("previews image threads as [图片]", () => {
    expect(previewBody(2, "https://media.kim.ainexc.com/a.png")).toBe(COPY.imageMessage);
    expect(previewBody(4, "clip")).toBe(COPY.videoMessage);
    expect(previewBody(1, "hello world")).toBe("hello world");
  });

  it("scales bubble size to the chat cap", () => {
    expect(bubbleSize({ w: 1200, h: 800 })).toEqual({ width: 280, height: 187 });
    expect(bubbleSize(undefined).width).toBeGreaterThanOrEqual(72);
  });
});
