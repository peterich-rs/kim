import { Message } from "./message";
import type { WireIndex } from "./proto";

export interface ContentLoader {
  readonly account: string;
  loadContent(ids: bigint[]): Promise<{ status: number; contents: Message[] }>;
}

const PAGE_COUNT = 20;

export class OfflineMessages {
  private readonly groupmessages = new Map<string, Message[]>();
  private readonly usermessages = new Map<string, Message[]>();

  constructor(
    private readonly cli: ContentLoader,
    indexes: WireIndex[],
  ) {
    for (let i = indexes.length - 1; i >= 0; i--) {
      const idx = indexes[i]!;
      const message = new Message(idx.messageId, idx.sendTime);
      if (idx.direction === 1) {
        message.sender = cli.account;
        message.receiver = idx.accountB;
      } else {
        message.sender = idx.accountB;
        message.receiver = cli.account;
      }
      if (idx.group) {
        let list = this.groupmessages.get(idx.group);
        if (!list) {
          list = [];
          this.groupmessages.set(idx.group, list);
        }
        message.group = idx.group;
        list.push(message);
      } else {
        let list = this.usermessages.get(idx.accountB);
        if (!list) {
          list = [];
          this.usermessages.set(idx.accountB, list);
        }
        list.push(message);
      }
    }
  }

  listUsers(): string[] {
    return [...this.usermessages.keys()];
  }

  listGroups(): string[] {
    return [...this.groupmessages.keys()];
  }

  getUserMessagesCount(account: string): number {
    return this.usermessages.get(account)?.length ?? 0;
  }

  getGroupMessagesCount(group: string): number {
    return this.groupmessages.get(group)?.length ?? 0;
  }

  loadUser(account: string, page: number): Promise<Message[]> {
    return this.lazyLoad(this.usermessages.get(account) ?? [], page);
  }

  loadGroup(group: string, page: number): Promise<Message[]> {
    return this.lazyLoad(this.groupmessages.get(group) ?? [], page);
  }

  private async lazyLoad(messages: Message[], page: number): Promise<Message[]> {
    const i = (page - 1) * PAGE_COUNT;
    if (i < 0 || i >= messages.length) {
      return [];
    }
    const msgs = messages.slice(i, i + PAGE_COUNT);
    if (msgs.length === 0) {
      return [];
    }
    if (msgs[0]?.contentLoaded) {
      return msgs;
    }
    const { status, contents } = await this.cli.loadContent(
      msgs.map((m) => m.messageId),
    );
    if (status !== 0) {
      return msgs;
    }
    const byId = new Map(
      contents
        .filter((c): c is Message => c != null)
        .map((c) => [c.messageId.toString(), c]),
    );
    for (const msg of msgs) {
      const content = byId.get(msg.messageId.toString());
      if (!content) {
        continue;
      }
      msg.type = content.type;
      msg.body = content.body;
      msg.extra = content.extra;
      msg.contentLoaded = true;
    }
    return msgs;
  }
}
