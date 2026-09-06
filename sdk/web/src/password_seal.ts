/**
 * Libsodium-compatible X25519 sealed box (crypto_box_seal).
 * Matches Rust `crypto_box` / kim-protocol PasswordSealKey.
 */
import { blake2b } from "@noble/hashes/blake2b";
import nacl from "tweetnacl";

export const PASSWORD_SEAL_ALG = "x25519-seal";

export interface PasswordKeyInfo {
  keyId: string;
  alg: string;
  publicKeyB64: string;
}

export function decodePublicKeyB64(b64: string): Uint8Array {
  const bin = atob(b64.trim());
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  if (out.length !== 32) {
    throw new Error("invalid password seal public key");
  }
  return out;
}

/** Seal plaintext to a 32-byte X25519 public key. */
export function sealPassword(plaintext: Uint8Array, recipientPk: Uint8Array): Uint8Array {
  if (recipientPk.length !== 32) {
    throw new Error("bad recipient public key");
  }
  const eph = nacl.box.keyPair();
  const nonce = blake2b(concat(eph.publicKey, recipientPk), { dkLen: 24 });
  const shared = nacl.box.before(recipientPk, eph.secretKey);
  const boxed = nacl.secretbox(plaintext, nonce, shared);
  return concat(eph.publicKey, boxed);
}

export function sealPasswordUtf8(password: string, recipientPk: Uint8Array): Uint8Array {
  return sealPassword(new TextEncoder().encode(password), recipientPk);
}

function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}
