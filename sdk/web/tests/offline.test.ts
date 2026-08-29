import { describe, expect, it } from "vitest";

import { Message } from "../src/message.ts";
import { OfflineMessages, type ContentLoader } from "../src/offline.ts";
import { Status } from "../src/status.ts";
import { MemoryStore } from "../src/store.ts";

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
