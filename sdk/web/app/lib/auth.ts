import { decodeAuthResp, encodeAuthReq } from "../../src/proto.ts";

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

async function postAuth(path: string, account: string, password: string): Promise<AuthSession> {
  const resp = await fetch(path, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-protobuf",
      Accept: "application/x-protobuf",
    },
    body: encodeAuthReq(account, password).slice(),
  });
  if (!resp.ok) {
    throw asError(resp.status, (await resp.text()) || `http ${resp.status}`);
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
