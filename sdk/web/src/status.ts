/** Wire Status. Numbers match `kim.pkt.Status` / crates/kim-protocol. */
export const Status = {
  Success: 0,
  InvalidPacket: 1,
  CommandNotFound: 2,
  ServiceUnavailable: 3,
  SystemException: 99,
  InvalidPacketBody: 101,
  InvalidCommand: 103,
  Unauthorized: 105,
  ContentBlocked: 106,
  NotGroupMember: 107,
  UserNotFound: 108,
  NotFriends: 109,
  Blocked: 110,
  NoDestination: 300,
  SessionNotFound: 404,
} as const;

export type StatusCode = (typeof Status)[keyof typeof Status];

/** Client-only; never on the wire. */
export const KIMStatus = {
  RequestTimeout: 1001,
  SendFailed: 1002,
} as const;

export const Flag = {
  Request: 0,
  Response: 1,
  Push: 2,
} as const;

export type FlagCode = (typeof Flag)[keyof typeof Flag];

/** Server asked the SDK to retry this request. */
export function isRetryable(status: number): boolean {
  return status >= 300 && status < 400;
}

/** Session is gone; close and (if enabled) log in again. */
export function needsRelogin(status: number): boolean {
  return Number(status) >= 400;
}
