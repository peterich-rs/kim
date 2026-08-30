import { COPY } from "../copy.ts";

function statusOf(err: unknown): number | undefined {
  if (typeof err === "object" && err !== null && "status" in err) {
    const value = err.status;
    return typeof value === "number" ? value : undefined;
  }
  return undefined;
}

function messageOf(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

export function mapUserError(err: unknown): string {
  const status = statusOf(err);
  const msg = messageOf(err);

  if (status === 401 || msg.includes("账号或密码错误")) {
    return COPY.badCredentials;
  }
  if (status === 409 || msg.includes("账号已存在")) {
    return COPY.accountExists;
  }
  if (msg.includes("invalid account")) {
    return COPY.invalidAccount;
  }
  if (msg.includes("invalid password")) {
    return COPY.invalidPassword;
  }
  if (msg.includes("timeout") || msg.includes("chat unreachable")) {
    return COPY.timeout;
  }
  if (
    msg.includes("Failed to fetch") ||
    msg.includes("NetworkError") ||
    msg === "offline" ||
    msg.includes("network")
  ) {
    return COPY.network;
  }
  if (status !== undefined && status >= 500) {
    return COPY.unavailable;
  }
  if (msg.includes("登录网关") || msg.includes("login")) {
    return COPY.authFailed;
  }
  return COPY.unavailable;
}

/** Long-connection Status. Undefined = caller keeps a generic message. */
export function mapWireStatus(status: number): string | undefined {
  switch (status) {
    case 101:
      return COPY.cannotAddSelf;
    case 108:
      return COPY.userNotFound;
    case 109:
      return COPY.notFriends;
    case 110:
      return COPY.blocked;
    default:
      return undefined;
  }
}
