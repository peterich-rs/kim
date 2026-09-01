import { describe, expect, it } from "vitest";

import { KIMClient, KIMEvent, State } from "../src/client.ts";
import { Command } from "../src/command.ts";
import { Content } from "../src/message.ts";
import { LogicPkt, readPacket } from "../src/packet.ts";
import { encodeKickout, encodeMessagePush } from "../src/proto.ts";
import { Flag, KIMStatus, Status } from "../src/status.ts";
import { MemoryStore } from "../src/store.ts";
import { accountFromToken } from "../src/token.ts";
import { LoopbackGw } from "./fake.ts";
import { mintToken } from "./token.ts";

function client(gw: LoopbackGw, account = "alice"): KIMClient {
  const cli = new KIMClient(
    "ws://127.0.0.1:1/",
    { token: mintToken(account) },
    {
      websocket: gw.factory,
      reconnect: false,
      heartbeatMs: 0,
      sendTimeoutMs: 500,
      loginTimeoutMs: 500,
      retrySleepMs: 1,
      ackForceAfterMs: 20,
      ackDelayMs: 0,
      ackPollMs: 10,
      store: new MemoryStore(),
    },
  );
  cli.onofflinemessage(() => {
    /* unit tests ignore offline */
  });
  return cli;
}

describe("accountFromToken", () => {
  it("reads acc without verifying", () => {
    expect(accountFromToken(mintToken("bob"))).toBe("bob");
  });
});

describe("KIMClient", () => {
  it("lookup uses routerUrl and never puts token on the WS URL", async () => {
    const gw = new LoopbackGw();
    let seenFetch = "";
    let seenWs = "";
    const cli = new KIMClient(
      "ws://fallback/",
      { token: mintToken("alice") },
      {
        websocket: (url) => {
          seenWs = url;
          return gw.factory(url);
        },
        fetch: (async (input: RequestInfo | URL) => {
          seenFetch = String(input);
          return new Response(JSON.stringify({ ws: "ws://127.0.0.1:8001/", tcp: "127.0.0.1:8003" }), {
            status: 200,
            headers: { "content-type": "application/json" },
          });
        }) as typeof fetch,
        routerUrl: "http://127.0.0.1:8088",
        reconnect: false,
        heartbeatMs: 0,
        loginTimeoutMs: 500,
        store: new MemoryStore(),
      },
    );
    cli.onofflinemessage(() => {
      /* ignore */
    });
    const { success } = await cli.login();
    expect(success).toBe(true);
    expect(seenFetch).toBe("http://127.0.0.1:8088/api/lookup");
    expect(seenWs).toBe("ws://127.0.0.1:8001/");
    expect(seenWs).not.toContain("token");
    expect(cli.wsurl).toBe("ws://fallback/");
  });

  it("does not start a second handshake if the socket drops during offline sync", async () => {
    const gw = new LoopbackGw();
    gw.dropOn = Command.OfflineIndex;
    let opens = 0;
    const inner = gw.factory;
    gw.factory = (url) => {
      opens += 1;
      return inner(url);
    };
    const cli = new KIMClient(
      "ws://127.0.0.1:1/",
      { token: mintToken("alice") },
      {
        websocket: gw.factory,
        reconnect: true,
        heartbeatMs: 0,
        sendTimeoutMs: 200,
        loginTimeoutMs: 500,
        retrySleepMs: 1,
        store: new MemoryStore(),
      },
    );
    cli.onofflinemessage(() => {
      /* ignore */
    });
    const { success, err } = await cli.login();
    expect(success).toBe(false);
    expect(err?.message).toMatch(/closed during sync|closed before login/);
    expect(cli.state).not.toBe(State.CONNECTED);
    await new Promise((r) => setTimeout(r, 80));
    expect(opens).toBe(1);
  });

  it("offline index 404 does not fail login.signin", async () => {
    const gw = new LoopbackGw();
    gw.statusFor[Command.OfflineIndex] = Status.SessionNotFound;
    const cli = client(gw);
    const { success, err } = await cli.login();
    expect(err).toBeUndefined();
    expect(success).toBe(true);
    expect(cli.state).toBe(State.CONNECTED);
  });

  it("logs in and exposes channelId from LoginResp", async () => {
    const gw = new LoopbackGw();
    const cli = client(gw);
    const { success, err } = await cli.login();
    expect(err).toBeUndefined();
    expect(success).toBe(true);
    expect(cli.channelId).toBe("wg-1_alice_1");
    expect(cli.account).toBe("alice");
    expect(cli.state).toBe(State.CONNECTED);
  });

  it("talkToUser awaits the matching sequence", async () => {
    const gw = new LoopbackGw();
    const cli = client(gw);
    await cli.login();
    const { status, resp } = await cli.talkToUser("bob", new Content("hello"));
    expect(status).toBe(Status.Success);
    expect(resp?.messageId).toBe(20001n);
    expect(gw.lastTalkDest).toBe("bob");
  });

  it("does not retry talk on content blocked, not group member, or user not found", async () => {
    for (const blocked of [Status.ContentBlocked, Status.NotGroupMember, Status.UserNotFound]) {
      const gw = new LoopbackGw();
      let talks = 0;
      const orig = gw.reply.bind(gw);
      gw.reply = (sock, data) => {
        const wire = readPacket(data);
        if (wire.kind === "logic" && wire.pkt.command === Command.ChatUserTalk) {
          talks += 1;
          gw.talkStatus = blocked;
        }
        orig(sock, data);
      };
      const cli = client(gw);
      await cli.login();
      const { status } = await cli.talkToUser("bob", new Content("hello"), 3);
      expect(status).toBe(blocked);
      expect(talks).toBe(1);
    }
  });

  it("retries talk on status 300", async () => {
    const gw = new LoopbackGw();
    const orig = gw.reply.bind(gw);
    let talks = 0;
    const bodies: string[] = [];
    gw.reply = (sock, data) => {
      const wire = readPacket(data);
      if (wire.kind === "logic" && wire.pkt.command === Command.ChatUserTalk) {
        talks += 1;
        bodies.push(Buffer.from(wire.pkt.payload).toString("hex"));
        gw.talkStatus = talks === 1 ? Status.NoDestination : Status.Success;
      }
      orig(sock, data);
    };
    const cli = client(gw);
    await cli.login();
    const { status, resp } = await cli.talkToUser("bob", new Content("hello"), 1);
    expect(status).toBe(Status.Success);
    expect(resp?.messageId).toBe(20001n);
    expect(talks).toBe(2);
    expect(bodies[0]).toBe(bodies[1]);
  });

  it("request times out when no response", async () => {
    const gw = new LoopbackGw();
    const orig = gw.reply.bind(gw);
    gw.reply = (sock, data) => {
      const pkt = LogicPkt.from(data);
      if (pkt.command === Command.ChatUserTalk) {
        return;
      }
      orig(sock, data);
    };
    const cli = client(gw);
    await cli.login();
    const { status } = await cli.talkToUser("bob", new Content("x"), 0);
    expect(status).toBe(KIMStatus.RequestTimeout);
  });

  it("kickout matching channelId logs out and does not reconnect", async () => {
    const gw = new LoopbackGw();
    const cli = client(gw);
    const events: string[] = [];
    cli.register([KIMEvent.Kickout, KIMEvent.Closed, KIMEvent.Reconnecting], (e) => {
      events.push(e);
    });
    await cli.login();
    const sock = gw.lastSocket();
    const ko = LogicPkt.build(Command.SignIn, "", encodeKickout("wg-1_alice_1"), 9);
    ko.flag = Flag.Push;
    sock.deliver(ko.bytes());
    await new Promise((r) => setTimeout(r, 30));
    expect(events).toContain(KIMEvent.Kickout);
    expect(events).toContain(KIMEvent.Closed);
    expect(events).not.toContain(KIMEvent.Reconnecting);
    expect(cli.state).toBe(State.CLOSED);
  });

  it("ignores kickout for another channelId", async () => {
    const gw = new LoopbackGw();
    const cli = client(gw);
    await cli.login();
    const sock = gw.lastSocket();
    const ko = LogicPkt.build(Command.SignIn, "", encodeKickout("wg-1_alice_9"), 9);
    ko.flag = Flag.Push;
    sock.deliver(ko.bytes());
    await new Promise((r) => setTimeout(r, 20));
    expect(cli.state).toBe(State.CONNECTED);
    expect(cli.channelId).toBe("wg-1_alice_1");
  });

  it("delivers online push only after CONNECTED and dedups messageId", async () => {
    const gw = new LoopbackGw();
    const cli = client(gw);
    const seen: bigint[] = [];
    cli.onmessage((m) => seen.push(m.messageId));
    await cli.login();
    const push = LogicPkt.build(
      Command.ChatUserTalk,
      "",
      encodeMessagePush({
        messageId: 99n,
        type: 1,
        body: "hi",
        extra: "",
        sender: "bob",
        sendTime: 1n,
      }),
      8,
    );
    push.flag = Flag.Push;
    gw.lastSocket().deliver(push.bytes());
    gw.lastSocket().deliver(push.bytes());
    await new Promise((r) => setTimeout(r, 20));
    expect(seen).toEqual([99n]);
  });

  it("offline sync pages 201 pending and batch-acks 200 then 1", async () => {
    const gw = new LoopbackGw();
    gw.pending = Array.from({ length: 201 }, (_, i) => ({
      messageId: BigInt(i + 1),
      direction: 0,
      sendTime: BigInt(i + 1),
      accountB: "bob",
      group: "",
    }));
    const cli = client(gw);
    let groups = 0;
    cli.onofflinemessage((om) => {
      groups = om.getUserMessagesCount("bob");
    });
    await cli.login();
    expect(groups).toBe(201);
    expect(gw.acked).toHaveLength(201);
    expect(gw.pending).toHaveLength(0);
    await cli.logout();
  });
});
