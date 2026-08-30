const TYPES: Record<string, string> = {
  "image/jpeg": "jpg",
  "image/jpg": "jpg",
  "image/png": "png",
  "image/webp": "webp",
  "image/gif": "gif",
};

export function extensionFor(contentType: string): string | null {
  const ct = contentType.split(";")[0]?.trim().toLowerCase() ?? "";
  return TYPES[ct] ?? null;
}

export function objectKey(account: string, ext: string, now = new Date()): string {
  const y = now.getUTCFullYear().toString();
  const m = String(now.getUTCMonth() + 1).padStart(2, "0");
  const id = crypto.randomUUID();
  return `${account}/${y}/${m}/${id}.${ext}`;
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
