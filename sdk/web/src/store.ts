import { Message } from "./message";

export interface MsgStore {
  exist(id: bigint): Promise<boolean>;
  insert(msg: Message): Promise<void>;
  setAck(id: bigint): Promise<void>;
  lastId(): Promise<bigint>;
  get?(id: bigint): Promise<Message | undefined>;
}

export class MemoryStore implements MsgStore {
  private readonly msgs = new Map<string, Message>();
  private ack = 0n;

  async exist(id: bigint): Promise<boolean> {
    return this.msgs.has(id.toString());
  }

  async insert(msg: Message): Promise<void> {
    this.msgs.set(msg.messageId.toString(), msg);
  }

  async get(id: bigint): Promise<Message | undefined> {
    return this.msgs.get(id.toString());
  }

  async setAck(id: bigint): Promise<void> {
    this.ack = id;
  }

  async lastId(): Promise<bigint> {
    return this.ack;
  }
}

interface Kv {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

type StoredMessage = {
  messageId: string;
  sendTime: string;
  sender: string;
  receiver: string;
  group: string;
  type: number;
  body: string;
  extra: string;
  contentLoaded: boolean;
};

/** Browser `localStorage`, or any key-value bag in tests. */
export class KeyValueStore implements MsgStore {
  constructor(
    private readonly kv: Kv,
    private readonly prefix = "kim",
  ) {}

  private keyMsg(id: bigint): string {
    return `${this.prefix}_msg_${id.toString()}`;
  }

  private keyLast(): string {
    return `${this.prefix}_last`;
  }

  async exist(id: bigint): Promise<boolean> {
    return this.kv.getItem(this.keyMsg(id)) != null;
  }

  async insert(msg: Message): Promise<void> {
    const row: StoredMessage = {
      messageId: msg.messageId.toString(),
      sendTime: msg.sendTime.toString(),
      sender: msg.sender,
      receiver: msg.receiver,
      group: msg.group,
      type: msg.type,
      body: msg.body,
      extra: msg.extra,
      contentLoaded: msg.contentLoaded,
    };
    this.kv.setItem(this.keyMsg(msg.messageId), JSON.stringify(row));
  }

  async get(id: bigint): Promise<Message | undefined> {
    const raw = this.kv.getItem(this.keyMsg(id));
    if (!raw) {
      return undefined;
    }
    const row = JSON.parse(raw) as StoredMessage;
    const msg = new Message(BigInt(row.messageId), BigInt(row.sendTime));
    msg.sender = row.sender;
    msg.receiver = row.receiver;
    msg.group = row.group;
    msg.type = row.type;
    msg.body = row.body;
    msg.extra = row.extra;
    msg.contentLoaded = row.contentLoaded;
    return msg;
  }

  async setAck(id: bigint): Promise<void> {
    this.kv.setItem(this.keyLast(), id.toString());
  }

  async lastId(): Promise<bigint> {
    const raw = this.kv.getItem(this.keyLast());
    if (!raw) {
      return 0n;
    }
    return BigInt(raw);
  }
}

export function browserStore(): MsgStore {
  if (typeof localStorage === "undefined") {
    return new MemoryStore();
  }
  return new KeyValueStore(localStorage);
}
