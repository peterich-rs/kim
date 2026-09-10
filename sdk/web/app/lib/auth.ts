import {
  decodeAuthResp,
  decodePasswordKeyResp,
  encodeAuthReq,
  encodePasswordChangeReq,
} from "../../src/proto.ts";
import {
  decodePublicKeyB64,
  PASSWORD_SEAL_ALG,
  sealPasswordUtf8,
} from "../../src/password_seal.ts";
import { assertPageAuthTransport } from "./secure_origin.ts";

export interface AuthSession {
  token: string;
  exp: number;
  account: string;
}

export interface AuthError extends Error {
  status: number;
}

function asError(status: number, message: string): AuthError {
  const err = new Error(message) as AuthError;
  err.status = status;
  return err;
}

interface SealMaterial {
  keyId: string;
  publicKey: Uint8Array;
}

let cachedSeal: { at: number; material: SealMaterial | null } | undefined;
const SEAL_TTL_MS = 5 * 60 * 1000;

async function fetchSealKey(): Promise<SealMaterial | null> {
  if (cachedSeal && Date.now() - cachedSeal.at < SEAL_TTL_MS) {
    return cachedSeal.material;
  }
  const resp = await fetch("/api/v1/auth/password-key", {
    headers: { Accept: "application/x-protobuf" },
  });
  if (resp.status === 404) {
    // Do not cache misses: a later deploy of the seal key must be picked up.
    cachedSeal = undefined;
    return null;
  }
  if (!resp.ok) {
    throw asError(resp.status, (await resp.text()) || `http ${resp.status}`);
  }
  const buf = new Uint8Array(await resp.arrayBuffer());
  const decoded = decodePasswordKeyResp(buf);
  if (decoded.alg !== PASSWORD_SEAL_ALG || !decoded.publicKeyB64) {
    throw asError(resp.status, "password seal unavailable");
  }
  const material = {
    keyId: decoded.keyId,
    publicKey: decodePublicKeyB64(decoded.publicKeyB64),
  };
  cachedSeal = { at: Date.now(), material };
  return material;
}

/** Test helper: clear seal cache between tests. */
export function clearPasswordSealCache(): void {
  cachedSeal = undefined;
}

async function postAuth(
  path: string,
  account: string,
  password: string,
  retried = false,
): Promise<AuthSession> {
  assertPageAuthTransport();
  const seal = await fetchSealKey();
  const body = seal
    ? encodeAuthReq(account, "", {
        passwordSealed: sealPasswordUtf8(password, seal.publicKey),
        keyId: seal.keyId,
      })
    : encodeAuthReq(account, password);
  const resp = await fetch(path, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-protobuf",
      Accept: "application/x-protobuf",
    },
    body: body.slice(),
  });
  if (!resp.ok) {
    const text = (await resp.text()) || `http ${resp.status}`;
    if (!retried && resp.status === 400 && text.includes("password key id mismatch")) {
      clearPasswordSealCache();
      return postAuth(path, account, password, true);
    }
    throw asError(resp.status, text);
  }
  const buf = new Uint8Array(await resp.arrayBuffer());
  const decoded = decodeAuthResp(buf);
  if (!decoded.token) {
    throw asError(resp.status, "token missing");
  }
  return {
    token: decoded.token,
    exp: decoded.exp,
    account: decoded.account || account,
  };
}

export async function register(account: string, password: string): Promise<AuthSession> {
  return postAuth("/api/v1/auth/register", account, password);
}

export async function login(account: string, password: string): Promise<AuthSession> {
  return postAuth("/api/v1/auth/login", account, password);
}

export async function logout(token: string): Promise<void> {
  const resp = await fetch("/api/v1/auth/logout", {
    method: "POST",
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!resp.ok && resp.status !== 401) {
    throw asError(resp.status, (await resp.text()) || `http ${resp.status}`);
  }
}

export async function changePassword(
  token: string,
  oldPassword: string,
  newPassword: string,
  retried = false,
): Promise<void> {
  assertPageAuthTransport();
  const seal = await fetchSealKey();
  const body = seal
    ? encodePasswordChangeReq("", "", {
        oldPasswordSealed: sealPasswordUtf8(oldPassword, seal.publicKey),
        newPasswordSealed: sealPasswordUtf8(newPassword, seal.publicKey),
        keyId: seal.keyId,
      })
    : encodePasswordChangeReq(oldPassword, newPassword);
  const resp = await fetch("/api/v1/auth/password", {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/x-protobuf",
    },
    body: body.slice(),
  });
  if (!resp.ok) {
    const text = (await resp.text()) || `http ${resp.status}`;
    if (!retried && resp.status === 400 && text.includes("password key id mismatch")) {
      clearPasswordSealCache();
      return changePassword(token, oldPassword, newPassword, true);
    }
    throw asError(resp.status, text);
  }
}
