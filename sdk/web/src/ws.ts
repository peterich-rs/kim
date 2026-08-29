export const WS_CONNECTING = 0;
export const WS_OPEN = 1;
export const WS_CLOSING = 2;
export const WS_CLOSED = 3;

export interface WebSocketLike {
  binaryType: string;
  readonly readyState: number;
  onopen: ((ev?: unknown) => void) | null;
  onclose: ((ev?: { reason?: string }) => void) | null;
  onerror: ((ev?: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  send(data: Uint8Array): void;
  close(): void;
}

export type WebSocketFactory = (url: string) => WebSocketLike;

export function defaultWebSocket(url: string): WebSocketLike {
  if (typeof WebSocket === "undefined") {
    throw new Error("WebSocket is not available");
  }
  const ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  return ws as unknown as WebSocketLike;
}
