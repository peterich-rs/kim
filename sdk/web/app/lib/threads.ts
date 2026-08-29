export type Kind = "user" | "group";

export interface Thread {
  id: string;
  kind: Kind;
  title: string;
  lastBody: string;
  lastAt: number;
  unread: number;
}

function keyOf(account: string): string {
  return `kim.web.threads.${account}`;
}

function isKind(value: unknown): value is Kind {
  return value === "user" || value === "group";
}

export function loadThreads(account: string): Thread[] {
  const raw = localStorage.getItem(keyOf(account));
  if (!raw) {
    return [];
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    const out: Thread[] = [];
    for (const row of parsed) {
      if (typeof row !== "object" || row === null) {
        continue;
      }
      const rec = row as Record<string, unknown>;
      if (typeof rec.id !== "string" || !rec.id || !isKind(rec.kind)) {
        continue;
      }
      out.push({
        id: rec.id,
        kind: rec.kind,
        title: typeof rec.title === "string" && rec.title ? rec.title : rec.id,
        lastBody: typeof rec.lastBody === "string" ? rec.lastBody : "",
        lastAt: typeof rec.lastAt === "number" ? rec.lastAt : 0,
        unread: 0,
      });
    }
    return out.sort((a, b) => b.lastAt - a.lastAt);
  } catch {
    return [];
  }
}

export function saveThreads(account: string, threads: Thread[]): void {
  const slim = threads.map((t) => ({
    id: t.id,
    kind: t.kind,
    title: t.title,
    lastBody: t.lastBody,
    lastAt: t.lastAt,
  }));
  localStorage.setItem(keyOf(account), JSON.stringify(slim));
}

export function clearThreads(account: string): void {
  localStorage.removeItem(keyOf(account));
}
