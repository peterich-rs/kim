const TYPES: Record<string, string> = {
  "image/jpeg": "jpg",
  "image/jpg": "jpg",
  "image/png": "png",
  "image/webp": "webp",
  "image/gif": "gif",
};

const CONTENT_TYPES: Record<string, string> = {
  jpg: "image/jpeg",
  png: "image/png",
  webp: "image/webp",
  gif: "image/gif",
};

export function extensionFor(contentType: string): string | null {
  const ct = contentType.split(";")[0]?.trim().toLowerCase() ?? "";
  return TYPES[ct] ?? null;
}

export function contentTypeFor(ext: string): string | null {
  return CONTENT_TYPES[ext] ?? null;
}

/** Magic-byte sniff so the object key follows bytes, not a lying Content-Type. */
export function sniffExtension(bytes: ArrayBuffer): string | null {
  const u = new Uint8Array(bytes);
  if (
    u.length >= 8 &&
    u[0] === 0x89 &&
    u[1] === 0x50 &&
    u[2] === 0x4e &&
    u[3] === 0x47
  ) {
    return "png";
  }
  if (u.length >= 3 && u[0] === 0xff && u[1] === 0xd8 && u[2] === 0xff) {
    return "jpg";
  }
  if (
    u.length >= 6 &&
    u[0] === 0x47 &&
    u[1] === 0x49 &&
    u[2] === 0x46 &&
    u[3] === 0x38
  ) {
    return "gif";
  }
  if (
    u.length >= 12 &&
    u[0] === 0x52 &&
    u[1] === 0x49 &&
    u[2] === 0x46 &&
    u[3] === 0x46 &&
    u[8] === 0x57 &&
    u[9] === 0x45 &&
    u[10] === 0x42 &&
    u[11] === 0x50
  ) {
    return "webp";
  }
  return null;
}

/** Resolve ext: sniff wins; fall back to Content-Type header. */
export function resolveExtension(
  contentType: string,
  bytes: ArrayBuffer,
): string | null {
  return sniffExtension(bytes) ?? extensionFor(contentType);
}

export function objectKey(sha256: string, ext: string): string {
  return `${sha256}.${ext}`;
}

export async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const u = new Uint8Array(digest);
  let out = "";
  for (let i = 0; i < u.length; i++) {
    out += u[i]!.toString(16).padStart(2, "0");
  }
  return out;
}

export function publicUrl(base: string, key: string): string {
  return `${base.replace(/\/+$/, "")}/${key}`;
}

export function parseMaxBytes(raw: string | undefined): number {
  const n = Number(raw);
  if (!Number.isFinite(n) || n <= 0) {
    return 5 * 1024 * 1024;
  }
  return Math.min(n, 25 * 1024 * 1024);
}
