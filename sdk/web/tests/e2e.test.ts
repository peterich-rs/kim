import { spawn, type ChildProcess } from "node:child_process";
import { mkdtemp, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { KIMClient, KIMEvent } from "../src/client.ts";
import { Content } from "../src/message.ts";
import { MemoryStore } from "../src/store.ts";
import { mintToken } from "./token.ts";

const enabled = process.env.KIM_SDK_E2E === "1";
const repoRoot = fileURLToPath(new URL("../../..", import.meta.url));

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const addr = s.address();
      if (typeof addr === "object" && addr) {
        const port = addr.port;
        s.close(() => resolve(port));
        return;
      }
      reject(new Error("no port"));
    });
    s.on("error", reject);
  });
}

function waitFor(proc: ChildProcess, needle: string, ms: number): Promise<void> {
  return new Promise((resolve, reject) => {
    let buf = "";
    const onData = (chunk: Buffer) => {
      buf += chunk.toString();
      if (buf.includes(needle)) {
        cleanup();
        resolve();
      }
    };
    const t = setTimeout(() => {
      cleanup();
      reject(new Error(`timeout waiting for ${needle}: ${buf.slice(-500)}`));
    }, ms);
    const cleanup = () => {
      clearTimeout(t);
      proc.stdout?.off("data", onData);
      proc.stderr?.off("data", onData);
    };
    proc.stdout?.on("data", onData);
    proc.stderr?.on("data", onData);
  });
}

async function startBin(
  bin: string,
  args: string[],
  needle: string,
): Promise<ChildProcess> {
  const proc = spawn(bin, args, {
    cwd: repoRoot,
    env: { ...process.env, RUST_LOG: "info" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  proc.stdout?.setEncoding("utf8");
  proc.stderr?.setEncoding("utf8");
  try {
    await waitFor(proc, needle, 30_000);
    return proc;
  } catch (err) {
    proc.kill("SIGKILL");
    throw err;
  }
}

function stop(proc: ChildProcess | undefined): void {
  if (!proc || proc.exitCode !== null) {
    return;
  }
  proc.kill("SIGTERM");
}

function sdk(url: string, account: string): KIMClient {
  const cli = new KIMClient(
    url,
    { token: mintToken(account) },
    {
      reconnect: false,
      heartbeatMs: 0,
      sendTimeoutMs: 5_000,
      loginTimeoutMs: 5_000,
      ackForceAfterMs: 50,
      ackDelayMs: 0,
      ackPollMs: 20,
      store: new MemoryStore(),
    },
  );
  cli.onofflinemessage(() => {
    /* default: tests that care overwrite this */
  });
  return cli;
}

describe.skipIf(!enabled)("sdk e2e against gateway", () => {
  let chat: ChildProcess;
  let gw: ChildProcess;
  let url = "";
  const procs: ChildProcess[] = [];

  beforeAll(async () => {
    const chatPort = await freePort();
    const gwPort = await freePort();
    const dir = await mkdtemp(path.join(tmpdir(), "kim-sdk-"));
    const chatCfg = path.join(dir, "chat.toml");
    const gwCfg = path.join(dir, "gw.toml");
    await writeFile(
      chatCfg,
      `[self]
service_id = "chat-1"
service_name = "chat"
listen = "127.0.0.1:${chatPort}"
protocol = "tcp"
snowflake_node = 1
`,
    );
    await writeFile(
      gwCfg,
      `[self]
service_id = "wg-1"
service_name = "wgateway"
listen = "127.0.0.1:${gwPort}"
protocol = "ws"
jwt_secret = ""

[[services]]
service_id = "chat-1"
service_name = "chat"
protocol = "tcp"
public_address = "127.0.0.1"
public_port = ${chatPort}
`,
    );
    const chatBin = path.join(repoRoot, "target/debug/chat");
    const gwBin = path.join(repoRoot, "target/debug/gateway");
    chat = await startBin(chatBin, [chatCfg], "tcp server listening");
    procs.push(chat);
    gw = await startBin(gwBin, [gwCfg], "ws server listening");
    procs.push(gw);
    url = `ws://127.0.0.1:${gwPort}/`;
    await new Promise((r) => setTimeout(r, 200));
  }, 60_000);

  afterAll(() => {
    for (const p of procs) {
      stop(p);
    }
    stop(gw);
    stop(chat);
  });

  it("logs in with JWT and returns wg-1_{account}_N", async () => {
    const cli = sdk(url, "alice");
    const { success, err } = await cli.login();
    expect(err).toBeUndefined();
    expect(success).toBe(true);
    expect(cli.channelId).toMatch(/^wg-1_alice_\d+$/);
    await cli.logout();
  });

  it("delivers 1:1 talk as an online push", async () => {
    const bob = sdk(url, "bob");
    const alice = sdk(url, "alice");
    const got: string[] = [];
    bob.onmessage((m) => {
      got.push(`${m.sender}:${m.body}`);
    });
    expect((await bob.login()).success).toBe(true);
    expect((await alice.login()).success).toBe(true);
    const { status, resp } = await alice.talkToUser("bob", new Content("hello sdk"));
    expect(status).toBe(0);
    expect(resp).toBeDefined();
    expect(resp!.messageId > 10_000n).toBe(true);
    await expect.poll(() => got, { timeout: 3_000 }).toEqual(["alice:hello sdk"]);
    await alice.logout();
    await bob.logout();
  });

  it("syncs offline index after the receiver logs in", async () => {
    const alice = sdk(url, "alice");
    expect((await alice.login()).success).toBe(true);
    const { status } = await alice.talkToUser("dave", new Content("offline-hi"));
    expect(status).toBe(0);
    await alice.logout();

    const dave = sdk(url, "dave");
    let users: string[] = [];
    dave.onofflinemessage((om) => {
      users = om.listUsers();
    });
    expect((await dave.login()).success).toBe(true);
    expect(users).toContain("alice");
    await dave.logout();
  });

  it("kicks the previous connection of the same account", async () => {
    const first = sdk(url, "erin");
    const events: string[] = [];
    first.register([KIMEvent.Kickout], (e) => events.push(e));
    expect((await first.login()).success).toBe(true);
    const second = sdk(url, "erin");
    expect((await second.login()).success).toBe(true);
    await expect.poll(() => events, { timeout: 3_000 }).toContain(KIMEvent.Kickout);
    await second.logout();
  });

  it("creates a group and talks in it", async () => {
    const alice = sdk(url, "alice");
    const bob = sdk(url, "bob");
    const bodies: string[] = [];
    bob.onmessage((m) => {
      if (m.group) {
        bodies.push(m.body);
      }
    });
    expect((await bob.login()).success).toBe(true);
    expect((await alice.login()).success).toBe(true);
    const created = await alice.createGroup({
      name: "sdk",
      members: ["alice", "bob"],
    });
    expect(created.status).toBe(0);
    expect(created.groupId).toBeTruthy();
    const { status } = await alice.talkToGroup(
      created.groupId!,
      new Content("group-hi"),
    );
    expect(status).toBe(0);
    await expect.poll(() => bodies, { timeout: 3_000 }).toContain("group-hi");
    const detail = await alice.groupDetail(created.groupId!);
    expect(detail.detail?.name).toBe("sdk");
    await alice.logout();
    await bob.logout();
  });
});
