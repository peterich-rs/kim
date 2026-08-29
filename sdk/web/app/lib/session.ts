import type { AuthSession } from "./auth.ts";

const KEY = "kim.web.session";

export interface StoredSession extends AuthSession {
  ws: string;
}

export function loadSession(): StoredSession | undefined {
  const raw = localStorage.getItem(KEY);
  if (!raw) {
    return undefined;
  }
  try {
    const body = JSON.parse(raw) as Partial<StoredSession>;
    if (
      typeof body.token !== "string" ||
      typeof body.account !== "string" ||
      typeof body.exp !== "number" ||
      typeof body.ws !== "string"
    ) {
      clearSession();
      return undefined;
    }
    if (body.exp <= Math.floor(Date.now() / 1000) + 30) {
      clearSession();
      return undefined;
    }
    return {
      token: body.token,
      account: body.account,
      exp: body.exp,
      ws: body.ws,
    };
  } catch {
    clearSession();
    return undefined;
  }
}

export function saveSession(session: StoredSession): void {
  localStorage.setItem(KEY, JSON.stringify(session));
}

export function clearSession(): void {
  localStorage.removeItem(KEY);
}
