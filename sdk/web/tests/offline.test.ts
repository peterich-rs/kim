import { describe, expect, it } from "vitest";

import { Message } from "../src/message.ts";
import {
  ackPrefixIds,
  mergeIndexContent,
  OfflineMessages,
  type ContentLoader,
} from "../src/offline.ts";
import {
  decodeIndexReq,
  decodeIndexResp,
  encodeAckReq,
  encodeIndexReq,
  encodeIndexResp,
  type WireIndex,
} from "../src/proto.ts";
import { Status } from "../src/status.ts";
import { MemoryStore } from "../src/store.ts";

describe("mergeIndexContent", () => {
  it("keeps index routing and content body", () => {
    const indexes: WireIndex[] = [
      {
        messageId: 1n,
        direction: 0,
        sendTime: 10n,
        accountB: "bob",
        group: "",
      },
      {
        messageId: 2n,
        direction: 1,
        sendTime: 11n,
        accountB: "bob",
        group: "",
      },
      {
        messageId: 3n,
        direction: 0,
        sendTime: 12n,
        accountB: "g",
        group: "G1",
      },
    ];
    const contents = [
      Object.assign(new Message(1n, 0n), { type: 1, body: "hi", extra: "" }),
      Object.assign(new Message(3n, 0n), { type: 1, body: "g", extra: "x" }),
    ];
    const merged = mergeIndexContent(indexes, contents, "alice");
    expect(merged).toHaveLength(2);
    expect(merged[0]?.sender).toBe("bob");
    expect(merged[0]?.receiver).toBe("alice");
    expect(merged[0]?.body).toBe("hi");
    expect(merged[1]?.group).toBe("G1");
    expect(merged[1]?.body).toBe("g");
    const loaded = new Set(merged.map((m) => m.messageId.toString()));
    expect(ackPrefixIds(indexes, loaded)).toEqual([1n]);
  });
});

describe("MemoryStore", () => {
  it("tracks ack and existence", async () => {
    const s = new MemoryStore();
    expect(await s.lastId()).toBe(0n);
    const m = new Message(5n, 1n);
    await s.insert(m);
    expect(await s.exist(5n)).toBe(true);
    await s.setAck(5n);
    expect(await s.lastId()).toBe(5n);
  });
});

describe("OfflineMessages", () => {
  it("groups by user and group, newest first, lazy-loads content", async () => {
    const contents = new Map<string, Message>([
      [
        "10",
        Object.assign(new Message(10n, 1n), {
          type: 1,
          body: "g",
          contentLoaded: true,
        }),
      ],
      [
        "11",
        Object.assign(new Message(11n, 2n), {
          type: 1,
          body: "u",
          contentLoaded: true,
        }),
      ],
    ]);
    const loader: ContentLoader = {
      account: "alice",
      async loadContent(ids) {
        const found: Message[] = [];
        for (const id of ids) {
          const m = contents.get(id.toString());
          if (m) {
            found.push(m);
          }
        }
        return { status: Status.Success, contents: found };
      },
    };
    const om = new OfflineMessages(loader, [
      {
        messageId: 10n,
        direction: 0,
        sendTime: 1n,
        accountB: "bob",
        group: "G1",
      },
      {
        messageId: 11n,
        direction: 0,
        sendTime: 2n,
        accountB: "bob",
        group: "",
      },
      {
        messageId: 12n,
        direction: 1,
        sendTime: 3n,
        accountB: "carol",
        group: "",
      },
    ]);
    expect(om.listGroups()).toEqual(["G1"]);
    expect(om.listUsers().sort()).toEqual(["bob", "carol"]);
    expect(om.getUserMessagesCount("bob")).toBe(1);
    const page = await om.loadUser("bob", 1);
    expect(page[0]?.body).toBe("u");
    expect(page[0]?.contentLoaded).toBe(true);
    const g = await om.loadGroup("G1", 1);
    expect(g[0]?.body).toBe("g");
    const mine = await om.loadUser("carol", 1);
    expect(mine[0]?.sender).toBe("alice");
  });
});

function idx(id: number): WireIndex {
  return {
    messageId: BigInt(id),
    direction: 0,
    sendTime: BigInt(id),
    accountB: "bob",
    group: "",
  };
}

describe("pending index protocol", () => {
  it("encodes resume=true and decodes hasMore", () => {
    const req = decodeIndexReq(encodeIndexReq({ resume: true }));
    expect(req.resume).toBe(true);
    expect(req.messageId).toBe(0n);
    const page = decodeIndexResp(encodeIndexResp([idx(1)], true));
    expect(page.indexes).toHaveLength(1);
    expect(page.hasMore).toBe(true);
  });

  it("legacy encodeIndexReq(lastId) terminates against the leftover circuit", () => {
    const pending = Array.from({ length: 5 }, (_, i) => idx(i + 1));
    let messageId = 0n;
    let rounds = 0;
    for (;;) {
      rounds += 1;
      const req = decodeIndexReq(encodeIndexReq(messageId));
      expect(req.resume).toBe(false);
      const page =
        req.messageId === 0n
          ? { indexes: pending.slice(0, 200), hasMore: false }
          : { indexes: [] as WireIndex[], hasMore: false };
      if (page.indexes.length === 0) {
        break;
      }
      messageId = page.indexes[page.indexes.length - 1]!.messageId;
      if (rounds > 8) {
        throw new Error("hot loop");
      }
    }
    expect(rounds).toBe(2);
  });

  it("resume paginates 201 as 200 then 1", () => {
    const pending = Array.from({ length: 201 }, (_, i) => idx(i + 1));
    const first = pending.slice(0, 200);
    const second = pending.slice(200);
    expect(first).toHaveLength(200);
    expect(second).toHaveLength(1);
    const page1 = decodeIndexResp(encodeIndexResp(first, true));
    expect(page1.indexes).toHaveLength(200);
    expect(page1.hasMore).toBe(true);
    const ids = page1.indexes.map((i) => i.messageId);
    expect(() => encodeAckReq(ids)).not.toThrow();
    const tooMany = Array.from({ length: 201 }, (_, i) => BigInt(i + 1));
    expect(encodeAckReq(tooMany).length).toBeGreaterThan(0);
    const page2 = decodeIndexResp(encodeIndexResp(second, false));
    expect(page2.indexes).toHaveLength(1);
    expect(page2.hasMore).toBe(false);
  });
});
