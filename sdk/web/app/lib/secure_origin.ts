/** Reject non-localhost http:// Royal origins in production clients. */
export function insecureAuthOriginReason(origin: string): string | null {
  const base = origin.trim().replace(/\/$/, "");
  if (!base) return "auth origin is empty";
  // Relative paths (same-origin web app) are OK; page protocol is checked separately.
  if (base.startsWith("/")) return null;
  let url: URL;
  try {
    url = new URL(base);
  } catch {
    return "auth origin must be https (or http://127.0.0.1 / localhost for local dev)";
  }
  if (url.protocol === "https:") return null;
  if (url.protocol === "http:" && isLoopbackHost(url.hostname)) return null;
  return "auth origin must be https (or http://127.0.0.1 / localhost for local dev)";
}

export function assertSecureAuthOrigin(origin: string): void {
  const reason = insecureAuthOriginReason(origin);
  if (reason) throw Object.assign(new Error(reason), { status: 0 });
}

export function assertPageAuthTransport(): void {
  if (typeof location === "undefined") return;
  if (location.protocol === "https:") return;
  if (isLoopbackHost(location.hostname)) return;
  throw Object.assign(
    new Error("auth origin must be https (or http://127.0.0.1 / localhost for local dev)"),
    { status: 0 },
  );
}

function isLoopbackHost(host: string): boolean {
  const h = host.toLowerCase();
  return h === "127.0.0.1" || h === "localhost" || h === "::1" || h === "[::1]";
}
