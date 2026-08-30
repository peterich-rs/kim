import {
  Content,
  InboxKind,
  KIMClient,
  KIMEvent,
  KeyValueStore,
  Message,
  State,
} from "../../src/index.ts";
import type { WireInboxItem, WireProfile } from "../../src/proto.ts";
import { COPY } from "../copy.ts";
import { mapWireStatus } from "./errors.ts";
import type { Kind } from "./threads.ts";

export interface ChatHandlers {
  onStatus: (status: "connecting" | "online" | "reconnecting" | "offline") => void;
  onMessage: (msg: Message, dest: string) => void;
  onKick: () => void;
  onGroup: (groupId: string, members: string[]) => void;
  onFriend?: (from: string, nickname: string) => void;
  onToken?: (token: string, exp: number) => void;
}

export function threadOf(msg: Message, me: string): string {
  if (msg.group) {
    return msg.group;
  }
  if (msg.sender === me) {
    return msg.receiver;
  }
  return msg.sender;
}

export class ChatSession {
  client: KIMClient | undefined;
  readonly account: string;
  private disposed = false;
  private readonly handlers: ChatHandlers;

  constructor(account: string, handlers: ChatHandlers) {
    this.account = account;
    this.handlers = handlers;
  }

  get alive(): boolean {
    return !this.disposed;
  }

  async connect(ws: string, token: string): Promise<void> {
    this.handlers.onStatus("connecting");
    const next = new KIMClient(
      ws,
      { token },
      {
        store: new KeyValueStore(localStorage, `kim_${this.account}`),
        reconnect: true,
      },
    );
    this.bind(next);
    if (this.handlers.onToken) {
      next.ontoken(this.handlers.onToken);
    }
    const { success, err } = await next.login();
    if (this.disposed) {
      await next.logout();
      return;
    }
    if (!success) {
      throw err ?? new Error("login failed");
    }
    this.client = next;
    this.handlers.onStatus("online");
  }

  private bind(cli: KIMClient): void {
    cli.register(
      [KIMEvent.Closed, KIMEvent.Kickout, KIMEvent.Reconnecting, KIMEvent.Reconnected],
      (evt) => {
        if (this.disposed) {
          return;
        }
        if (evt === KIMEvent.Kickout) {
          this.handlers.onStatus("offline");
          this.handlers.onKick();
          return;
        }
        if (evt === KIMEvent.Reconnecting) {
          this.handlers.onStatus("reconnecting");
        } else if (evt === KIMEvent.Reconnected) {
          this.handlers.onStatus("online");
        } else {
          this.handlers.onStatus("offline");
        }
      },
    );
    cli.onmessage((m) => {
      if (this.disposed) {
        return;
      }
      this.handlers.onMessage(m, threadOf(m, cli.account));
    });
    cli.onofflinemessage((om) => {
      void (async () => {
        for (const u of om.listUsers()) {
          const page = await om.loadUser(u, 1);
          if (this.disposed) {
            return;
          }
          for (const m of page) {
            m.sender = m.sender || u;
            this.handlers.onMessage(m, u);
          }
        }
        for (const g of om.listGroups()) {
          const page = await om.loadGroup(g, 1);
          if (this.disposed) {
            return;
          }
          for (const m of page) {
            m.group = g;
            this.handlers.onMessage(m, g);
          }
        }
      })();
    });
    cli.ongroupcreate((groupId, members) => {
      if (this.disposed) {
        return;
      }
      this.handlers.onGroup(groupId, members);
    });
    cli.onfriendrequest((from, nickname) => {
      if (this.disposed) {
        return;
      }
      this.handlers.onFriend?.(from, nickname);
    });
  }

  async send(dest: string, kind: Kind, text: string): Promise<Message> {
    const cli = this.client;
    if (!cli || cli.state !== State.CONNECTED) {
      throw new Error("not connected");
    }
    const { status, resp, err } =
      kind === "group"
        ? await cli.talkToGroup(dest, new Content(text))
        : await cli.talkToUser(dest, new Content(text));
    if (status !== 0) {
      throw new Error(mapWireStatus(status) ?? err?.message ?? `send ${status}`);
    }
    const msg = new Message(resp?.messageId ?? 0n, resp?.sendTime ?? 0n);
    msg.sender = cli.account;
    msg.receiver = dest;
    msg.group = kind === "group" ? dest : "";
    msg.type = 1;
    msg.body = text;
    msg.contentLoaded = true;
    return msg;
  }

  async createGroup(name: string, members: string[]): Promise<string> {
    const cli = this.client;
    if (!cli) {
      throw new Error("not connected");
    }
    const list = members.includes(cli.account) ? members : [cli.account, ...members];
    const { status, groupId, err } = await cli.createGroup({ name, members: list });
    if (status !== 0 || !groupId) {
      throw err ?? new Error(`create group ${status}`);
    }
    return groupId;
  }

  async members(groupId: string): Promise<string[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { members } = await cli.groupMembers(groupId);
    return members ?? [];
  }

  async groupTitle(groupId: string): Promise<string | undefined> {
    const cli = this.client;
    if (!cli) {
      return undefined;
    }
    const { detail } = await cli.groupDetail(groupId);
    const name = detail?.name?.trim();
    return name || undefined;
  }

  async inbox(): Promise<WireInboxItem[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { items } = await cli.inbox();
    return items;
  }

  async history(dest: string, kind: Kind, beforeId = 0n): Promise<Message[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { messages } = await cli.history(
      dest,
      kind === "group" ? InboxKind.Group : InboxKind.User,
      beforeId,
    );
    return messages.map((row) => {
      const m = new Message(row.messageId, row.sendTime);
      m.sender = row.sender;
      m.receiver = kind === "group" ? dest : row.sender === cli.account ? dest : cli.account;
      m.group = kind === "group" ? dest : "";
      m.type = row.type;
      m.body = row.body;
      m.extra = row.extra;
      m.contentLoaded = true;
      return m;
    });
  }

  async markRead(dest: string, kind: Kind, messageId: bigint): Promise<void> {
    const cli = this.client;
    if (!cli || messageId <= 0n) {
      return;
    }
    await cli.markRead(dest, kind === "group" ? InboxKind.Group : InboxKind.User, messageId);
  }

  async searchUsers(query: string): Promise<WireProfile[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { users } = await cli.searchUsers(query);
    return users;
  }

  async friends(): Promise<WireProfile[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { users } = await cli.friendList();
    return users ?? [];
  }

  async incoming(): Promise<WireProfile[]> {
    const cli = this.client;
    if (!cli) {
      return [];
    }
    const { users } = await cli.friendIncoming();
    return users ?? [];
  }

  async requestFriend(account: string): Promise<void> {
    const cli = this.client;
    if (!cli) {
      throw new Error("not connected");
    }
    const { status, err } = await cli.friendRequest(account);
    if (status !== 0) {
      throw new Error(mapWireStatus(status) ?? err?.message ?? `request ${status}`);
    }
  }

  async acceptFriend(account: string): Promise<void> {
    const cli = this.client;
    if (!cli) {
      throw new Error("not connected");
    }
    const { status, err } = await cli.friendAccept(account);
    if (status !== 0) {
      throw new Error(mapWireStatus(status) ?? err?.message ?? `accept ${status}`);
    }
  }

  async rejectFriend(account: string): Promise<void> {
    const cli = this.client;
    if (!cli) {
      throw new Error("not connected");
    }
    const { status, err } = await cli.friendReject(account);
    if (status !== 0) {
      throw new Error(mapWireStatus(status) ?? err?.message ?? `reject ${status}`);
    }
  }

  async updateProfile(nickname: string, bio: string): Promise<WireProfile | undefined> {
    const cli = this.client;
    if (!cli) {
      throw new Error("not connected");
    }
    const { status, profile, err } = await cli.updateProfile({ nickname, bio });
    if (status !== 0) {
      throw err ?? new Error(`profile ${status}`);
    }
    return profile;
  }

  async me(): Promise<WireProfile | undefined> {
    const cli = this.client;
    if (!cli) {
      return undefined;
    }
    const { profile } = await cli.profile();
    return profile;
  }

  async disconnect(): Promise<void> {
    const cur = this.client;
    this.client = undefined;
    await cur?.logout();
  }

  dispose(): void {
    this.disposed = true;
    void this.disconnect();
  }
}
