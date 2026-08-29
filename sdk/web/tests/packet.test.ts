import { describe, expect, it } from "vitest";

import { Command } from "../src/command.ts";
import {
  BasicPkt,
  CODE_PING,
  CODE_PONG,
  LogicPkt,
  MAGIC_LOGIC,
  readPacket,
  resetSequence,
} from "../src/packet.ts";
import { encodeLoginReq, decodeLoginResp, encodeLoginResp } from "../src/proto.ts";
import { Flag, Status } from "../src/status.ts";
import { readU32BE } from "../src/bytes.ts";

describe("BasicPkt", () => {
  it("ping is 8 bytes matching kim-protocol LE code=1", () => {
    const bytes = BasicPkt.ping().encode();
    expect([...bytes]).toEqual([0xc3, 0x15, 0xa7, 0x65, 1, 0, 0, 0]);
    const got = BasicPkt.decode(bytes);
    expect(got.code).toBe(CODE_PING);
    expect(got.body.length).toBe(0);
  });

  it("pong roundtrips", () => {
    const bytes = BasicPkt.pong().encode();
    expect(bytes[4]).toBe(CODE_PONG);
    expect(readPacket(bytes).kind).toBe("basic");
  });
});

describe("LogicPkt", () => {
  it("uses BE header_len and Header.bodyLength, no extra payload length", () => {
    resetSequence(7);
    const body = encodeLoginReq("tok");
    const pkt = LogicPkt.build(Command.SignIn, "", body, 7);
    const buf = pkt.bytes();
    expect([...buf.subarray(0, 4)]).toEqual([...MAGIC_LOGIC]);
    const hlen = readU32BE(buf, 4);
    expect(buf.length).toBe(8 + hlen + body.length);
    const again = LogicPkt.from(buf);
    expect(again.command).toBe(Command.SignIn);
    expect(again.sequence).toBe(7);
    expect(again.payload.length).toBe(body.length);
    const stripped = LogicPkt.from(buf.subarray(4));
    expect(stripped.command).toBe(Command.SignIn);
    expect(stripped.sequence).toBe(7);
  });

  it("roundtrips LoginResp channelId", () => {
    const pkt = LogicPkt.build(Command.SignIn, "", encodeLoginResp("wg-1_alice_1"), 1);
    pkt.flag = Flag.Response;
    pkt.status = Status.Success;
    const got = LogicPkt.from(pkt.bytes());
    expect(decodeLoginResp(got.payload).channelId).toBe("wg-1_alice_1");
  });
});
