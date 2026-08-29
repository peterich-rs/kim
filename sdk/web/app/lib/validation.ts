import { COPY } from "../copy.ts";

const ACCOUNT = /^[A-Za-z0-9_]{3,32}$/;

export function validateAccount(raw: string): string | undefined {
  if (!ACCOUNT.test(raw.trim())) {
    return COPY.invalidAccount;
  }
  return undefined;
}

export function validatePassword(raw: string): string | undefined {
  if (raw.length < 8 || raw.length > 128) {
    return COPY.invalidPassword;
  }
  return undefined;
}

export function validateConfirm(password: string, confirm: string): string | undefined {
  if (password !== confirm) {
    return COPY.mismatch;
  }
  return undefined;
}
