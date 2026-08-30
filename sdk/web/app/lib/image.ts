import { MessageType } from "../../src/command.ts";
import { COPY } from "../copy.ts";
import { truncate } from "./format.ts";

export const MEDIA_HOST = "media.kim.ainexc.com";
export const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
export const ACCEPT_IMAGE = "image/jpeg,image/png,image/webp,image/gif";

const IMAGE_EXT = /\.(?:jpe?g|png|webp|gif)(?:\?|$)/i;

export type ImageSize = { w: number; h: number };

export function encodeImageExtra(w: number, h: number): string {
  const width = Math.max(0, Math.round(w));
  const height = Math.max(0, Math.round(h));
  if (width <= 0 || height <= 0) {
    return "";
  }
  return JSON.stringify({ w: width, h: height });
}

export function parseImageExtra(extra: string): ImageSize | undefined {
  const raw = extra.trim();
  if (!raw.startsWith("{")) {
    return undefined;
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return undefined;
    }
    const rec = parsed as Record<string, unknown>;
    const w = Number(rec.w);
    const h = Number(rec.h);
    if (!Number.isFinite(w) || !Number.isFinite(h) || w <= 0 || h <= 0) {
      return undefined;
    }
    return { w: Math.round(w), h: Math.round(h) };
  } catch {
    return undefined;
  }
}

export function isMediaUrl(body: string): boolean {
  const url = body.trim();
  if (!/^https?:\/\//i.test(url)) {
    return false;
  }
  try {
    const parsed = new URL(url);
    if (parsed.hostname === MEDIA_HOST) {
      return true;
    }
    return IMAGE_EXT.test(parsed.pathname);
  } catch {
    return IMAGE_EXT.test(url);
  }
}

export function isImageMessage(type: number, body: string, extra = ""): boolean {
  if (type === MessageType.Image) {
    return true;
  }
  if (type === MessageType.Text || type === 0) {
    return parseImageExtra(extra) !== undefined || isMediaUrl(body);
  }
  return false;
}

export function previewBody(type: number, body: string, extra = ""): string {
  if (type === MessageType.Video) {
    return COPY.videoMessage;
  }
  if (isImageMessage(type, body, extra)) {
    return COPY.imageMessage;
  }
  return truncate(body);
}

export function readImageSize(file: Blob): Promise<ImageSize> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve({ w: img.naturalWidth, h: img.naturalHeight });
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("image decode failed"));
    };
    img.src = url;
  });
}

export function bubbleSize(size: ImageSize | undefined): { width: number; height: number } {
  const w = size?.w && size.w > 0 ? size.w : 160;
  const h = size?.h && size.h > 0 ? size.h : 160;
  const maxW = 280;
  const maxH = 320;
  const scale = Math.min(maxW / w, maxH / h, 1);
  return {
    width: Math.max(72, Math.round(w * scale)),
    height: Math.max(72, Math.round(h * scale)),
  };
}
