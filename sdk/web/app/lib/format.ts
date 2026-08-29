import { COPY } from "../copy.ts";

const AVATAR_COLORS = [
  "#e17076",
  "#faa775",
  "#a695e7",
  "#7bc862",
  "#6ec9cb",
  "#65aadd",
  "#ee7aae",
  "#e5c85f",
  "#54a0ff",
  "#3ee0c5",
];

export function initial(name: string): string {
  return (name.trim().charAt(0) || "?").toUpperCase();
}

export function avatarColor(name: string): string {
  let hash = 0;
  for (const ch of name) {
    hash = (hash * 33 + ch.charCodeAt(0)) >>> 0;
  }
  const color = AVATAR_COLORS[hash % AVATAR_COLORS.length];
  return color ?? "#65aadd";
}

export function sendTimeMs(sendTime: bigint, fallback = Date.now()): number {
  if (sendTime <= 0n) {
    return fallback;
  }
  if (sendTime > 10_000_000_000_000_000n) {
    return Number(sendTime / 1_000_000n);
  }
  if (sendTime > 10_000_000_000_000n) {
    return Number(sendTime);
  }
  return Number(sendTime) * 1000;
}

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

export function formatListTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const today = startOfDay(now);
  const point = startOfDay(d);
  if (point === today) {
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  }
  if (point === today - 86_400_000) {
    return COPY.yesterday;
  }
  return d.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

export function formatClock(ts: number): string {
  return new Date(ts).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function truncate(text: string, max = 36): string {
  const t = text.trim().replace(/\s+/g, " ");
  if (t.length <= max) {
    return t;
  }
  return `${t.slice(0, max)}…`;
}
