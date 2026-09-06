import { describe, expect, it } from "vitest";
import nacl from "tweetnacl";
import { blake2b } from "@noble/hashes/blake2b";
import { sealPasswordUtf8 } from "../src/password_seal.ts";

describe("password seal", () => {
  it("roundtrips with tweetnacl secretbox open path", () => {
    const recipient = nacl.box.keyPair();
    const sealed = sealPasswordUtf8("secret123", recipient.publicKey);
    expect(sealed.length).toBeGreaterThan(32);
    const ephPk = sealed.subarray(0, 32);
    const boxed = sealed.subarray(32);
    const nonce = blake2b(
      (() => {
        const out = new Uint8Array(64);
        out.set(ephPk, 0);
        out.set(recipient.publicKey, 32);
        return out;
      })(),
      { dkLen: 24 },
    );
    const shared = nacl.box.before(ephPk, recipient.secretKey);
    const opened = nacl.secretbox.open(boxed, nonce, shared);
    expect(opened).not.toBeNull();
    expect(new TextDecoder().decode(opened!)).toBe("secret123");
  });
});
