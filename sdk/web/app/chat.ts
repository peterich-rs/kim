import {
  Content,
  KIMClient,
  KIMEvent,
  KeyValueStore,
  Message,
  State,
} from "../src/index.ts";

export type Kind = "user" | "group";

export interface Thread {
  id: string;
  kind: Kind;
  title: string;
}

export interface ChatHandlers {
  onStatus: (text: string, cls: "ok" | "bad" | "") => void;
  onEvent: (evt: string) => void;
  onMessage: (msg: Message, dest: string) => void;
  onKick: () => void;
  onGroup: (groupId: string, members: string[]) => void;
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
  private readonly handlers: ChatHandlers;

  constructor(account: string, handlers: ChatHandlers) {
    this.account = account;
    this.handlers = handlers;
  }

  async connect(ws: string, token: string): Promise<void> {
    const next = new KIMClient(ws, { token }, {
      store: new KeyValueStore(localStorage, `kim_${this.account}`),
      reconnect: true,
    });
    this.bind(next);
    const { success, err } = await next.login();
    if (!success) {
      throw err ?? new Error("登录网关失败");
    }
    this.client = next;
    this.handlers.onStatus(`在线 · ${next.channelId}`, "ok");
  }

  private bind(cli: KIMClient): void {
    cli.register(
      [KIMEvent.Closed, KIMEvent.Kickout, KIMEvent.Reconnecting, KIMEvent.Reconnected],
      (evt) => {
        if (evt === KIMEvent.Kickout) {
          this.handlers.onStatus("已在其他设备登录", "bad");
          this.handlers.onKick();
          return;
        }
        if (evt === KIMEvent.Reconnecting) {
          this.handlers.onStatus("重连中…", "");
        } else if (evt === KIMEvent.Reconnected) {
          this.handlers.onStatus(`在线 · ${cli.channelId}`, "ok");
        } else {
          this.handlers.onStatus("已断开", "bad");
        }
        this.handlers.onEvent(evt);
      },
    );
    cli.onmessage((m) => {
      this.handlers.onMessage(m, threadOf(m, cli.account));
    });
    cli.onofflinemessage((om) => {
      void (async () => {
        for (const u of om.listUsers()) {
          const page = await om.loadUser(u, 1);
          for (const m of page) {
            m.sender = m.sender || u;
            this.handlers.onMessage(m, u);
          }
        }
        for (const g of om.listGroups()) {
          const page = await om.loadGroup(g, 1);
          for (const m of page) {
            m.group = g;
            this.handlers.onMessage(m, g);
          }
        }
      })();
    });
    cli.ongroupcreate((groupId, members) => {
      this.handlers.onGroup(groupId, members);
    });
  }

  async send(dest: string, kind: Kind, text: string): Promise<Message> {
    const cli = this.client;
    if (!cli || cli.state !== State.CONNECTED) {
      throw new Error("未连接");
    }
    const { status, resp, err } =
      kind === "group"
        ? await cli.talkToGroup(dest, new Content(text))
        : await cli.talkToUser(dest, new Content(text));
    if (status !== 0) {
      throw err ?? new Error(`发送失败 ${status}`);
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
      throw new Error("未连接");
    }
    const list = members.includes(cli.account) ? members : [cli.account, ...members];
    const { status, groupId, err } = await cli.createGroup({ name, members: list });
    if (status !== 0 || !groupId) {
      throw err ?? new Error(`建群失败 ${status}`);
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

  async disconnect(): Promise<void> {
    const cur = this.client;
    this.client = undefined;
    await cur?.logout();
  }
}
