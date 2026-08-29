import Long from "long";
import protobuf from "protobufjs";

import descriptor from "./proto/pkt.json";
import { asBigInt } from "./bytes";

protobuf.util.Long = Long;
protobuf.configure();

const root = protobuf.Root.fromJSON(descriptor);

const TO_OBJECT: protobuf.IConversionOptions = {
  longs: String,
  enums: Number,
  defaults: true,
};

function lookup(name: string): protobuf.Type {
  return root.lookupType(`kim.pkt.${name}`);
}

const HeaderType = lookup("Header");
const LoginReqType = lookup("LoginReq");
const LoginRespType = lookup("LoginResp");
const KickoutNotifyType = lookup("KickoutNotify");
const MessageReqType = lookup("MessageReq");
const MessageRespType = lookup("MessageResp");
const MessagePushType = lookup("MessagePush");
const MessageAckReqType = lookup("MessageAckReq");
const MessageIndexReqType = lookup("MessageIndexReq");
const MessageIndexRespType = lookup("MessageIndexResp");
const MessageContentReqType = lookup("MessageContentReq");
const MessageContentRespType = lookup("MessageContentResp");
const GroupCreateReqType = lookup("GroupCreateReq");
const GroupCreateRespType = lookup("GroupCreateResp");
const GroupCreateNotifyType = lookup("GroupCreateNotify");
const GroupJoinReqType = lookup("GroupJoinReq");
const GroupQuitReqType = lookup("GroupQuitReq");
const GroupDetailType = lookup("GroupDetail");
const GroupMembersRespType = lookup("GroupMembersResp");
const AuthReqType = lookup("AuthReq");
const AuthRespType = lookup("AuthResp");

function encode(type: protobuf.Type, obj: object): Uint8Array {
  return type.encode(type.create(obj)).finish();
}

function decode<T>(type: protobuf.Type, buf: Uint8Array): T {
  return type.toObject(type.decode(buf), TO_OBJECT) as T;
}

export interface HeaderFields {
  command?: string;
  channelId?: string;
  sequence?: number;
  flag?: number;
  status?: number;
  dest?: string;
  bodyLength?: number;
}

export function encodeHeader(h: HeaderFields): Uint8Array {
  return encode(HeaderType, h);
}

export function decodeHeader(buf: Uint8Array): Required<HeaderFields> {
  const o = decode<HeaderFields>(HeaderType, buf);
  return {
    command: o.command ?? "",
    channelId: o.channelId ?? "",
    sequence: o.sequence ?? 0,
    flag: o.flag ?? 0,
    status: o.status ?? 0,
    dest: o.dest ?? "",
    bodyLength: o.bodyLength ?? 0,
  };
}

export function encodeLoginReq(token: string): Uint8Array {
  return encode(LoginReqType, { token });
}

export function decodeLoginResp(buf: Uint8Array): { channelId: string } {
  const o = decode<{ channelId?: string }>(LoginRespType, buf);
  return { channelId: o.channelId ?? "" };
}

export function encodeLoginResp(channelId: string): Uint8Array {
  return encode(LoginRespType, { channelId });
}

export function decodeKickout(buf: Uint8Array): { channelId: string } {
  const o = decode<{ channelId?: string }>(KickoutNotifyType, buf);
  return { channelId: o.channelId ?? "" };
}

export function encodeKickout(channelId: string): Uint8Array {
  return encode(KickoutNotifyType, { channelId });
}

export interface ContentBody {
  type: number;
  body: string;
  extra: string;
}

export function encodeMessageReq(c: ContentBody): Uint8Array {
  return encode(MessageReqType, c);
}

export function decodeMessageResp(buf: Uint8Array): {
  messageId: bigint;
  sendTime: bigint;
} {
  const o = decode<{ messageId?: unknown; sendTime?: unknown }>(
    MessageRespType,
    buf,
  );
  return { messageId: asBigInt(o.messageId), sendTime: asBigInt(o.sendTime) };
}

export function encodeMessageResp(messageId: bigint, sendTime: bigint): Uint8Array {
  return encode(MessageRespType, {
    messageId: messageId.toString(),
    sendTime: sendTime.toString(),
  });
}

export function decodeMessagePush(buf: Uint8Array): {
  messageId: bigint;
  type: number;
  body: string;
  extra: string;
  sender: string;
  sendTime: bigint;
} {
  const o = decode<{
    messageId?: unknown;
    type?: number;
    body?: string;
    extra?: string;
    sender?: string;
    sendTime?: unknown;
  }>(MessagePushType, buf);
  return {
    messageId: asBigInt(o.messageId),
    type: o.type ?? 0,
    body: o.body ?? "",
    extra: o.extra ?? "",
    sender: o.sender ?? "",
    sendTime: asBigInt(o.sendTime),
  };
}

export function encodeMessagePush(p: {
  messageId: bigint;
  type: number;
  body: string;
  extra: string;
  sender: string;
  sendTime: bigint;
}): Uint8Array {
  return encode(MessagePushType, {
    messageId: p.messageId.toString(),
    type: p.type,
    body: p.body,
    extra: p.extra,
    sender: p.sender,
    sendTime: p.sendTime.toString(),
  });
}

export function encodeAckReq(messageId: bigint): Uint8Array {
  return encode(MessageAckReqType, { messageId: messageId.toString() });
}

export function encodeIndexReq(messageId: bigint): Uint8Array {
  return encode(MessageIndexReqType, { messageId: messageId.toString() });
}

export interface WireIndex {
  messageId: bigint;
  direction: number;
  sendTime: bigint;
  accountB: string;
  group: string;
}

export function decodeIndexResp(buf: Uint8Array): WireIndex[] {
  const o = decode<{
    indexes?: Array<{
      messageId?: unknown;
      direction?: number;
      sendTime?: unknown;
      accountB?: string;
      group?: string;
    }>;
  }>(MessageIndexRespType, buf);
  return (o.indexes ?? []).map((idx) => ({
    messageId: asBigInt(idx.messageId),
    direction: idx.direction ?? 0,
    sendTime: asBigInt(idx.sendTime),
    accountB: idx.accountB ?? "",
    group: idx.group ?? "",
  }));
}

export function encodeIndexResp(indexes: WireIndex[]): Uint8Array {
  return encode(MessageIndexRespType, {
    indexes: indexes.map((idx) => ({
      messageId: idx.messageId.toString(),
      direction: idx.direction,
      sendTime: idx.sendTime.toString(),
      accountB: idx.accountB,
      group: idx.group,
    })),
  });
}

export function encodeContentReq(ids: bigint[]): Uint8Array {
  return encode(MessageContentReqType, {
    messageIds: ids.map((id) => id.toString()),
  });
}

export interface WireContent {
  messageId: bigint;
  type: number;
  body: string;
  extra: string;
}

export function decodeContentResp(buf: Uint8Array): WireContent[] {
  const o = decode<{
    messages?: Array<{
      messageId?: unknown;
      type?: number;
      body?: string;
      extra?: string;
    }>;
  }>(MessageContentRespType, buf);
  return (o.messages ?? []).map((m) => ({
    messageId: asBigInt(m.messageId),
    type: m.type ?? 0,
    body: m.body ?? "",
    extra: m.extra ?? "",
  }));
}

export function encodeContentResp(messages: WireContent[]): Uint8Array {
  return encode(MessageContentRespType, {
    messages: messages.map((m) => ({
      messageId: m.messageId.toString(),
      type: m.type,
      body: m.body,
      extra: m.extra,
    })),
  });
}

export function encodeGroupCreateReq(req: {
  name: string;
  avatar: string;
  introduction: string;
  owner: string;
  members: string[];
}): Uint8Array {
  return encode(GroupCreateReqType, req);
}

export function decodeGroupCreateResp(buf: Uint8Array): { groupId: string } {
  const o = decode<{ groupId?: string }>(GroupCreateRespType, buf);
  return { groupId: o.groupId ?? "" };
}

export function encodeGroupCreateResp(groupId: string): Uint8Array {
  return encode(GroupCreateRespType, { groupId });
}

export function decodeGroupCreateNotify(buf: Uint8Array): {
  groupId: string;
  members: string[];
} {
  const o = decode<{ groupId?: string; members?: string[] }>(
    GroupCreateNotifyType,
    buf,
  );
  return { groupId: o.groupId ?? "", members: o.members ?? [] };
}

export function encodeGroupJoinReq(account: string, groupId: string): Uint8Array {
  return encode(GroupJoinReqType, { account, groupId });
}

export function encodeGroupQuitReq(account: string, groupId: string): Uint8Array {
  return encode(GroupQuitReqType, { account, groupId });
}

export function decodeGroupDetail(buf: Uint8Array): {
  groupId: string;
  name: string;
  avatar: string;
  introduction: string;
  owner: string;
  members: string[];
} {
  const o = decode<{
    groupId?: string;
    name?: string;
    avatar?: string;
    introduction?: string;
    owner?: string;
    members?: string[];
  }>(GroupDetailType, buf);
  return {
    groupId: o.groupId ?? "",
    name: o.name ?? "",
    avatar: o.avatar ?? "",
    introduction: o.introduction ?? "",
    owner: o.owner ?? "",
    members: o.members ?? [],
  };
}

export function decodeGroupMembers(buf: Uint8Array): string[] {
  const o = decode<{ members?: string[] }>(GroupMembersRespType, buf);
  return o.members ?? [];
}

export function encodeAuthReq(account: string, password: string): Uint8Array {
  return encode(AuthReqType, { account, password });
}

export function decodeAuthResp(buf: Uint8Array): {
  token: string;
  exp: number;
  account: string;
} {
  const o = decode<{ token?: string; exp?: unknown; account?: string }>(
    AuthRespType,
    buf,
  );
  const expRaw = o.exp;
  const exp =
    typeof expRaw === "number"
      ? expRaw
      : typeof expRaw === "string"
        ? Number(expRaw)
        : 0;
  return {
    token: o.token ?? "",
    exp: Number.isFinite(exp) ? exp : 0,
    account: o.account ?? "",
  };
}
