import { toUint8 } from "./bytes";
import { Command } from "./command";
import { doLogin } from "./login";
import { Content, Message, Response, type LoginBody, type TalkResult } from "./message";
import { OfflineMessages, type ContentLoader } from "./offline";
import { BasicPkt, LogicPkt, readPacket } from "./packet";
import {
  decodeAuthResp,
  decodeContentResp,
  decodeGroupCreateNotify,
  decodeGroupCreateResp,
  decodeGroupDetail,
  decodeGroupMembers,
  decodeIndexResp,
  decodeKickout,
  decodeMessagePush,
  decodeMessageResp,
  encodeAckReq,
  encodeContentReq,
  encodeGroupCreateReq,
  encodeGroupJoinReq,
  encodeGroupQuitReq,
  encodeIndexReq,
  encodeMessageReq,
  type WireIndex,
} from "./proto";
import { Flag, isRetryable, KIMStatus, needsRelogin, Status } from "./status";
import { MemoryStore, type MsgStore } from "./store";
import { defaultWebSocket, WS_OPEN, type WebSocketFactory, type WebSocketLike } from "./ws";

export const State = {
  INIT: 0,
  CONNECTING: 1,
  CONNECTED: 2,
  CLOSING: 3,
  CLOSED: 4,
} as const;

export const KIMEvent = {
  Closed: "closed",
  Reconnecting: "reconnecting",
  Reconnected: "reconnected",
  Kickout: "kickout",
} as const;

export type KIMEventName = (typeof KIMEvent)[keyof typeof KIMEvent];

export interface ClientOptions {
  heartbeatMs?: number;
  sendTimeoutMs?: number;
  loginTimeoutMs?: number;
  ackDelayMs?: number;
  ackPollMs?: number;
  ackForceAfterMs?: number;
  unackOverflow?: number;
  retrySleepMs?: number;
  reconnect?: boolean;
  store?: MsgStore;
  websocket?: WebSocketFactory;
  /** Optional HTTP lookup. Token stays on Authorization, never on the WS URL. */
  routerUrl?: string;
  fetch?: typeof fetch;
}

interface ResolvedOptions {
  heartbeatMs: number;
  sendTimeoutMs: number;
  loginTimeoutMs: number;
  ackDelayMs: number;
  ackPollMs: number;
  ackForceAfterMs: number;
  unackOverflow: number;
  retrySleepMs: number;
  reconnect: boolean;
  store: MsgStore;
  websocket: WebSocketFactory;
  routerUrl?: string;
  fetch: typeof fetch;
}

function resolveOptions(opts: ClientOptions | undefined): ResolvedOptions {
  return {
    heartbeatMs: opts?.heartbeatMs ?? 50_000,
    sendTimeoutMs: opts?.sendTimeoutMs ?? 10_000,
    loginTimeoutMs: opts?.loginTimeoutMs ?? 10_000,
    ackDelayMs: opts?.ackDelayMs ?? 500,
    ackPollMs: opts?.ackPollMs ?? 500,
    ackForceAfterMs: opts?.ackForceAfterMs ?? 3_000,
    unackOverflow: opts?.unackOverflow ?? 10,
    retrySleepMs: opts?.retrySleepMs ?? 2_000,
    reconnect: opts?.reconnect ?? true,
    store: opts?.store ?? new MemoryStore(),
    websocket: opts?.websocket ?? defaultWebSocket,
    routerUrl: opts?.routerUrl,
    fetch: opts?.fetch ?? fetch,
  };
}

async function lookupWs(
  routerUrl: string,
  token: string,
  fetchFn: typeof fetch,
): Promise<string> {
  const url = `${routerUrl.replace(/\/$/, "")}/api/lookup`;
  const resp = await fetchFn(url, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok) {
    throw new Error(`lookup ${resp.status}`);
  }
  const body = (await resp.json()) as { ws?: string };
  if (!body.ws) {
    throw new Error("lookup missing ws");
  }
  return body.ws;
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

interface Pending {
  timer: ReturnType<typeof setTimeout>;
  callback: (resp: Response) => void;
}

export class KIMClient implements ContentLoader {
  state: number = State.INIT;
  channelId = "";
  account = "";

  private conn: WebSocketLike | undefined;
  private nextSeq = 2;
  private lastRead = 0;
  private readonly sendq = new Map<number, Pending>();
  private readonly listeners = new Map<string, (e: KIMEventName) => void>();
  private messageCallback: (m: Message) => void = () => {
    /* warn once in constructor */
  };
  private offmessageCallback: (m: OfflineMessages) => void = () => {
    /* warn once in constructor */
  };
  private groupCreateCallback: ((groupId: string, members: string[]) => void) | undefined;
  private tokenCallback: ((token: string, exp: number) => void) | undefined;
  private lastMessage: Message | undefined;
  private unack = 0;
  private heartbeatTimer: ReturnType<typeof setInterval> | undefined;
  private watchTimer: ReturnType<typeof setInterval> | undefined;
  private ackTimer: ReturnType<typeof setTimeout> | undefined;
  private closeCallback: (() => void) | undefined;
  private handlingError = false;
  private kicked = false;
  private readonly seenIds = new Set<string>();
  private readonly opts: ResolvedOptions;

  constructor(
    public readonly wsurl: string,
    private readonly req: LoginBody,
    opts?: ClientOptions,
  ) {
    this.opts = resolveOptions(opts);
    this.messageCallback = () => {
      console.warn(
        "throw a message. register onmessage before login",
      );
    };
    this.offmessageCallback = () => {
      console.warn(
        "throw OfflineMessages. register onofflinemessage before login",
      );
    };
  }

  register(events: KIMEventName[], callback: (e: KIMEventName) => void): void {
    for (const event of events) {
      this.listeners.set(event, callback);
    }
  }

  onmessage(cb: (m: Message) => void): void {
    this.messageCallback = cb;
  }

  onofflinemessage(cb: (m: OfflineMessages) => void): void {
    this.offmessageCallback = cb;
  }

  ongroupcreate(cb: (groupId: string, members: string[]) => void): void {
    this.groupCreateCallback = cb;
  }

  ontoken(cb: (token: string, exp: number) => void): void {
    this.tokenCallback = cb;
  }

  get token(): string {
    return this.req.token;
  }

  async login(): Promise<{ success: boolean; err?: Error }> {
    if (this.state === State.CONNECTED || this.state === State.CONNECTING) {
      return { success: false, err: new Error("client has already been connected") };
    }
    this.state = State.CONNECTING;
    this.kicked = false;
    let url = this.wsurl;
    if (this.opts.routerUrl) {
      try {
        url = await lookupWs(this.opts.routerUrl, this.req.token, this.opts.fetch);
      } catch (err) {
        this.state = State.INIT;
        return { success: false, err: err as Error };
      }
    }
    const result = await doLogin(url, this.req, {
      timeoutMs: this.opts.loginTimeoutMs,
      websocket: this.opts.websocket,
    });
    if (!result.success || !result.conn || !result.channelId || !result.account) {
      this.state = State.INIT;
      return { success: false, err: result.err };
    }
    this.conn = result.conn;
    this.channelId = result.channelId;
    this.account = result.account;
    this.nextSeq = 2;
    this.bindConn(result.conn);
    try {
      await this.loadOfflineMessage();
    } catch {
      /* still go connected; empty offline */
    }
    if (this.state !== State.CONNECTING) {
      return { success: false, err: new Error("closed during sync") };
    }
    this.state = State.CONNECTED;
    this.startHeartbeat();
    this.messageAckLoop();
    return { success: true };
  }

  logout(): Promise<void> {
    return new Promise((resolve) => {
      if (this.state === State.CLOSING || this.state === State.CLOSED) {
        resolve();
        return;
      }
      this.state = State.CLOSING;
      this.stopLoops();
      this.flushSendq(KIMStatus.SendFailed);
      if (!this.conn) {
        this.finishClose("logout");
        resolve();
        return;
      }
      const tr = setTimeout(() => {
        this.finishClose("logout");
        resolve();
      }, 2000);
      this.closeCallback = () => {
        clearTimeout(tr);
        resolve();
      };
      try {
        this.conn.close();
      } catch {
        clearTimeout(tr);
        this.finishClose("logout");
        resolve();
      }
    });
  }

  async talkToUser(dest: string, req: Content, retry = 3): Promise<TalkResult> {
    return this.talk(Command.ChatUserTalk, dest, req, retry);
  }

  async talkToGroup(dest: string, req: Content, retry = 3): Promise<TalkResult> {
    return this.talk(Command.ChatGroupTalk, dest, req, retry);
  }

  async createGroup(req: {
    name: string;
    members: string[];
    avatar?: string;
    introduction?: string;
  }): Promise<{ status: number; groupId?: string; err?: Error }> {
    const pkt = LogicPkt.build(
      Command.GroupCreate,
      "",
      encodeGroupCreateReq({
        name: req.name,
        avatar: req.avatar ?? "",
        introduction: req.introduction ?? "",
        owner: this.account,
        members: req.members,
      }),
      this.allocSeq(),
    );
    const resp = await this.request(pkt);
    if (resp.status !== Status.Success) {
      return { status: resp.status, err: new Error(`status ${resp.status}`) };
    }
    const body = decodeGroupCreateResp(resp.payload);
    return { status: resp.status, groupId: body.groupId };
  }

  async joinGroup(
    groupId: string,
    account?: string,
  ): Promise<{ status: number; err?: Error }> {
    return this.groupDest(
      Command.GroupJoin,
      groupId,
      encodeGroupJoinReq(account ?? this.account, groupId),
    );
  }

  async quitGroup(
    groupId: string,
    account?: string,
  ): Promise<{ status: number; err?: Error }> {
    return this.groupDest(
      Command.GroupQuit,
      groupId,
      encodeGroupQuitReq(account ?? this.account, groupId),
    );
  }

  async groupDetail(groupId: string): Promise<{
    status: number;
    detail?: ReturnType<typeof decodeGroupDetail>;
    err?: Error;
  }> {
    const pkt = LogicPkt.build(Command.GroupDetail, groupId, new Uint8Array(), this.allocSeq());
    const resp = await this.request(pkt);
    if (resp.status !== Status.Success) {
      return { status: resp.status, err: new Error(`status ${resp.status}`) };
    }
    return { status: resp.status, detail: decodeGroupDetail(resp.payload) };
  }

  async groupMembers(
    groupId: string,
  ): Promise<{ status: number; members?: string[]; err?: Error }> {
    const pkt = LogicPkt.build(Command.GroupMembers, groupId, new Uint8Array(), this.allocSeq());
    const resp = await this.request(pkt);
    if (resp.status !== Status.Success) {
      return { status: resp.status, err: new Error(`status ${resp.status}`) };
    }
    return { status: resp.status, members: decodeGroupMembers(resp.payload) };
  }

  async loadContent(ids: bigint[]): Promise<{ status: number; contents: Message[] }> {
    if (ids.length === 0) {
      return { status: Status.Success, contents: [] };
    }
    if (ids.length > 200) {
      return { status: Status.InvalidPacketBody, contents: [] };
    }
    const pkt = LogicPkt.build(
      Command.OfflineContent,
      "",
      encodeContentReq(ids),
      this.allocSeq(),
    );
    const resp = await this.request(pkt);
    if (resp.status !== Status.Success) {
      return { status: resp.status, contents: [] };
    }
    const wires = decodeContentResp(resp.payload);
    const contents = wires.map((w) => {
      const m = new Message(w.messageId, 0n);
      m.type = w.type;
      m.body = w.body;
      m.extra = w.extra;
      m.contentLoaded = true;
      return m;
    });
    return { status: resp.status, contents };
  }

  private async groupDest(
    command: string,
    groupId: string,
    payload: Uint8Array,
  ): Promise<{ status: number; err?: Error }> {
    const pkt = LogicPkt.build(command, groupId, payload, this.allocSeq());
    const resp = await this.request(pkt);
    if (resp.status !== Status.Success) {
      return { status: resp.status, err: new Error(`status ${resp.status}`) };
    }
    return { status: resp.status };
  }

  private async talk(
    command: string,
    dest: string,
    req: Content,
    retry: number,
  ): Promise<TalkResult> {
    const clientId =
      globalThis.crypto?.randomUUID?.() ??
      `${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`;
    const body = encodeMessageReq({
      type: req.type,
      body: req.body,
      extra: req.extra,
      clientId,
    });
    for (let i = 0; i < retry + 1; i++) {
      const pkt = LogicPkt.build(command, dest, body, this.allocSeq());
      const resp = await this.request(pkt);
      if (resp.status === Status.Success) {
        return { status: Status.Success, resp: decodeMessageResp(resp.payload) };
      }
      if (isRetryable(resp.status)) {
        await sleep(this.opts.retrySleepMs);
        continue;
      }
      return { status: resp.status, err: new Error(`status ${resp.status}`) };
    }
    return { status: KIMStatus.SendFailed, err: new Error("over max retry times") };
  }

  request(data: LogicPkt): Promise<Response> {
    return new Promise((resolve) => {
      if (!this.conn || this.conn.readyState !== WS_OPEN) {
        resolve(new Response(KIMStatus.SendFailed));
        return;
      }
      const seq = data.sequence;
      const tr = setTimeout(() => {
        this.sendq.delete(seq);
        resolve(new Response(KIMStatus.RequestTimeout));
      }, this.opts.sendTimeoutMs);
      this.sendq.set(seq, {
        timer: tr,
        callback: (resp) => resolve(resp),
      });
      if (!this.send(data.bytes())) {
        clearTimeout(tr);
        this.sendq.delete(seq);
        resolve(new Response(KIMStatus.SendFailed));
      }
    });
  }

  private send(data: Uint8Array): boolean {
    if (!this.conn || this.conn.readyState !== WS_OPEN) {
      return false;
    }
    try {
      this.conn.send(data);
      return true;
    } catch {
      return false;
    }
  }

  private allocSeq(): number {
    const n = this.nextSeq;
    this.nextSeq = (this.nextSeq + 1) >>> 0;
    if (this.nextSeq === 0) {
      this.nextSeq = 1;
    }
    return n;
  }

  private bindConn(conn: WebSocketLike): void {
    conn.onmessage = (evt) => {
      try {
        this.lastRead = Date.now();
        const buf = toUint8(evt.data);
        const wire = readPacket(buf);
        if (wire.kind === "basic") {
          return;
        }
        void this.packetHandler(wire.pkt);
      } catch (err) {
        console.error(err);
      }
    };
    conn.onerror = () => {
      void this.errorHandler(new Error("websocket error"));
    };
    conn.onclose = (e) => {
      if (this.state === State.CLOSING) {
        this.finishClose("logout");
        return;
      }
      if (this.handlingError) {
        return;
      }
      void this.errorHandler(new Error(e?.reason || "closed"));
    };
  }

  private async packetHandler(pkt: LogicPkt): Promise<void> {
    if (needsRelogin(pkt.status)) {
      try {
        this.conn?.close();
      } catch {
        /* ignore */
      }
      return;
    }
    if (pkt.flag === Flag.Response) {
      const req = this.sendq.get(pkt.sequence);
      if (req) {
        clearTimeout(req.timer);
        this.sendq.delete(pkt.sequence);
        req.callback(new Response(pkt.status, pkt.dest, pkt.payload));
      }
      return;
    }
    switch (pkt.command) {
      case Command.ChatUserTalk:
      case Command.ChatGroupTalk: {
        const push = decodeMessagePush(pkt.payload);
        const seenKey = push.messageId.toString();
        if (this.seenIds.has(seenKey)) {
          return;
        }
        this.seenIds.add(seenKey);
        if (await this.opts.store.exist(push.messageId)) {
          return;
        }
        const message = new Message(push.messageId, push.sendTime);
        message.type = push.type;
        message.body = push.body;
        message.extra = push.extra;
        message.sender = push.sender;
        message.receiver = this.account;
        message.contentLoaded = true;
        if (pkt.command === Command.ChatGroupTalk) {
          message.group = pkt.dest;
        }
        if (this.state === State.CONNECTED) {
          this.lastMessage = message;
          this.unack += 1;
          try {
            this.messageCallback(message);
          } catch (err) {
            console.error(err);
          }
        }
        await this.opts.store.insert(message);
        break;
      }
      case Command.SignIn: {
        const ko = decodeKickout(pkt.payload);
        if (ko.channelId === this.channelId) {
          this.kicked = true;
          this.fireEvent(KIMEvent.Kickout);
          await this.logout();
        }
        break;
      }
      case Command.Renew: {
        const body = decodeAuthResp(pkt.payload);
        if (body.token) {
          this.req.token = body.token;
          this.tokenCallback?.(body.token, body.exp);
        }
        break;
      }
      case Command.GroupCreate: {
        const n = decodeGroupCreateNotify(pkt.payload);
        this.groupCreateCallback?.(n.groupId, n.members);
        break;
      }
      default:
        break;
    }
  }

  private async loadOfflineMessage(): Promise<void> {
    const indexes: WireIndex[] = [];
    let messageId = await this.opts.store.lastId();
    for (;;) {
      const pkt = LogicPkt.build(
        Command.OfflineIndex,
        "",
        encodeIndexReq(messageId),
        this.allocSeq(),
      );
      const resp = await this.request(pkt);
      if (resp.status !== Status.Success) {
        break;
      }
      const page = decodeIndexResp(resp.payload);
      if (page.length === 0) {
        break;
      }
      messageId = page[page.length - 1]!.messageId;
      indexes.push(...page);
    }
    const om = new OfflineMessages(this, indexes);
    try {
      this.offmessageCallback(om);
    } catch (err) {
      console.error(err);
    }
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    const ms = this.opts.heartbeatMs;
    if (!ms) {
      return;
    }
    this.lastRead = Date.now();
    this.heartbeatTimer = setInterval(() => {
      this.send(BasicPkt.ping().encode());
    }, ms);
    this.watchTimer = setInterval(() => {
      if (this.state !== State.CONNECTED) {
        return;
      }
      if (Date.now() - this.lastRead > ms * 3) {
        void this.errorHandler(new Error("read timeout"));
      }
    }, ms);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer !== undefined) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = undefined;
    }
    if (this.watchTimer !== undefined) {
      clearInterval(this.watchTimer);
      this.watchTimer = undefined;
    }
  }

  private messageAckLoop(): void {
    let start = Date.now();
    const loop = (): void => {
      if (this.state !== State.CONNECTED) {
        return;
      }
      const msg = this.lastMessage;
      if (msg && Date.now() - start > this.opts.ackForceAfterMs) {
        const overflow = this.unack > this.opts.unackOverflow;
        this.unack = 0;
        this.lastMessage = undefined;
        const diff = Date.now() - msg.arrivalTime;
        const sendAck = (): void => {
          const pkt = LogicPkt.build(
            Command.ChatTalkAck,
            "",
            encodeAckReq(msg.messageId),
            this.allocSeq(),
          );
          start = Date.now();
          this.send(pkt.bytes());
          void this.opts.store.setAck(msg.messageId);
        };
        if (!overflow && diff < this.opts.ackDelayMs) {
          setTimeout(sendAck, this.opts.ackDelayMs - diff);
        } else {
          sendAck();
        }
      }
      this.ackTimer = setTimeout(loop, this.opts.ackPollMs);
    };
    this.ackTimer = setTimeout(loop, this.opts.ackPollMs);
  }

  private stopLoops(): void {
    this.stopHeartbeat();
    if (this.ackTimer !== undefined) {
      clearTimeout(this.ackTimer);
      this.ackTimer = undefined;
    }
  }

  private flushSendq(status: number): void {
    for (const [, req] of this.sendq) {
      clearTimeout(req.timer);
      req.callback(new Response(status));
    }
    this.sendq.clear();
  }

  private async errorHandler(err: Error): Promise<void> {
    if (this.handlingError) {
      return;
    }
    if (this.state === State.CLOSING || this.state === State.CLOSED) {
      return;
    }
    if (this.kicked || !this.opts.reconnect) {
      this.finishClose(err.message);
      return;
    }
    this.handlingError = true;
    this.stopLoops();
    this.flushSendq(KIMStatus.SendFailed);
    this.fireEvent(KIMEvent.Reconnecting);
    this.state = State.INIT;
    try {
      this.conn?.close();
    } catch {
      /* ignore */
    }
    this.conn = undefined;
    let delay = 1000;
    try {
      for (;;) {
        await sleep(delay);
        if (this.state === State.CLOSING || this.state === State.CLOSED || this.kicked) {
          return;
        }
        const { success } = await this.login();
        if (success) {
          this.fireEvent(KIMEvent.Reconnected);
          return;
        }
        delay = Math.min(delay * 2, 16_000);
      }
    } finally {
      this.handlingError = false;
    }
  }

  private finishClose(_reason: string): void {
    if (this.state === State.CLOSED) {
      return;
    }
    this.state = State.CLOSED;
    this.stopLoops();
    this.flushSendq(KIMStatus.SendFailed);
    this.conn = undefined;
    this.channelId = "";
    this.account = "";
    this.fireEvent(KIMEvent.Closed);
    const cb = this.closeCallback;
    this.closeCallback = undefined;
    cb?.();
  }

  private fireEvent(evt: KIMEventName): void {
    this.listeners.get(evt)?.(evt);
  }
}
