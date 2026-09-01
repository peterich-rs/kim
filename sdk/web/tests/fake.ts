import { Command } from "../src/command.ts";
import { LogicPkt, readPacket } from "../src/packet.ts";
import {
  encodeIndexResp,
  encodeLoginResp,
  encodeMessageResp,
  decodeIndexReq,
  decodeAckReq,
  type WireIndex,
} from "../src/proto.ts";
import { Flag, Status } from "../src/status.ts";
import {
  WS_CLOSED,
  WS_CONNECTING,
  WS_OPEN,
  type WebSocketLike,
} from "../src/ws.ts";

export class FakeSocket implements WebSocketLike {
  binaryType = "arraybuffer";
  readyState = WS_CONNECTING;
  onopen: ((ev?: unknown) => void) | null = null;
  onclose: ((ev?: { reason?: string }) => void) | null = null;
  onerror: ((ev?: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  readonly sent: Uint8Array[] = [];

  constructor(private readonly gw?: LoopbackGw) {}

  send(data: Uint8Array): void {
    const copy = Uint8Array.from(data);
    this.sent.push(copy);
    if (this.gw) {
      queueMicrotask(() => this.gw!.reply(this, copy));
    }
  }

  close(): void {
    if (this.readyState === WS_CLOSED) {
      return;
    }
    this.readyState = WS_CLOSED;
    this.onclose?.({});
  }

  open(): void {
    this.readyState = WS_OPEN;
    this.onopen?.();
  }

  deliver(buf: Uint8Array): void {
    const copy = Uint8Array.from(buf);
    this.onmessage?.({ data: copy.buffer });
  }
}

export class LoopbackGw {
  channelId = "wg-1_alice_1";
  talkStatus = Status.Success;
  sockets: FakeSocket[] = [];
  lastTalkDest = "";
  lastTalkBody = "";
  dropOn = "";
  statusFor: Record<string, number> = {};
  pending: WireIndex[] = [];
  acked: bigint[] = [];

  factory = (_url: string): FakeSocket => {
    const s = new FakeSocket(this);
    this.sockets.push(s);
    queueMicrotask(() => s.open());
    return s;
  };

  lastSocket(): FakeSocket {
    const s = this.sockets[this.sockets.length - 1];
    if (!s) {
      throw new Error("no socket");
    }
    return s;
  }

  reply(sock: FakeSocket, data: Uint8Array): void {
    const wire = readPacket(data);
    if (wire.kind !== "logic") {
      return;
    }
    const pkt = wire.pkt;
    if (this.dropOn && pkt.command === this.dropOn) {
      sock.close();
      return;
    }
    if (pkt.command === Command.ChatUserTalk || pkt.command === Command.ChatGroupTalk) {
      this.lastTalkDest = pkt.dest;
      this.lastTalkBody = new TextDecoder().decode(pkt.payload);
    }
    const resp = new LogicPkt();
    resp.command = pkt.command;
    resp.sequence = pkt.sequence;
    resp.flag = Flag.Response;
    resp.status = this.statusFor[pkt.command] ?? Status.Success;
    if (pkt.command === Command.SignIn) {
      resp.payload = encodeLoginResp(this.channelId);
    } else if (pkt.command === Command.OfflineIndex) {
      const req = decodeIndexReq(pkt.payload);
      if (req.resume) {
        const rest = this.pending.slice(0, 200);
        const hasMore = this.pending.length > 200;
        resp.payload = encodeIndexResp(rest, hasMore);
      } else if (req.messageId === 0n) {
        resp.payload = encodeIndexResp(this.pending.slice(0, 200), false);
      } else {
        resp.payload = encodeIndexResp([], false);
      }
    } else if (pkt.command === Command.ChatTalkAck) {
      const ack = decodeAckReq(pkt.payload);
      const ids = ack.messageIds.length > 0 ? ack.messageIds : [ack.messageId];
      this.acked.push(...ids.filter((id) => id !== 0n));
      const acked = new Set(this.acked.map((id) => id.toString()));
      this.pending = this.pending.filter((idx) => !acked.has(idx.messageId.toString()));
    } else if (
      pkt.command === Command.ChatUserTalk ||
      pkt.command === Command.ChatGroupTalk
    ) {
      resp.status = this.talkStatus;
      if (resp.status === Status.Success) {
        resp.payload = encodeMessageResp(20001n, 2000n);
      }
    }
    sock.deliver(resp.bytes());
  }
}
