import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { COPY } from "../app/copy.ts";

const appDir = fileURLToPath(new URL("../app", import.meta.url));
const forbidden = ["给朋友", "两个窗口", "长连接", "短接口", "开两个", "给朋友用"];

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) {
      out.push(...walk(p));
    } else if (/\.(tsx|ts|css|html)$/.test(name)) {
      out.push(p);
    }
  }
  return out;
}

describe("product copy", () => {
  it("does not contain conversational leftover phrases", () => {
    const blob = JSON.stringify(COPY);
    for (const phrase of forbidden) {
      expect(blob).not.toContain(phrase);
    }
    for (const file of walk(appDir)) {
      const text = readFileSync(file, "utf8");
      for (const phrase of forbidden) {
        expect(text, file).not.toContain(phrase);
      }
    }
  });
});
