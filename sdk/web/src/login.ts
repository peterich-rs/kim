import type { LoginBody } from "./message";
import { LogicPkt } from "./packet";
import { encodeLoginReq, decodeLoginResp } from "./proto";
import { Flag, Status } from "./status";
import { Command } from "./command";
import { accountFromToken } from "./token";
import { toUint8 } from "./bytes";
import { readPacket } from "./packet";
import { WS_OPEN, type WebSocketFactory, type WebSocketLike } from "./ws";

export interface DoLoginOpts {
  timeoutMs: number;
  websocket: WebSocketFactory;
}

export interface DoLoginResult {
  success: boolean;
  err?: Error;
  channelId?: string;
  account?: string;
  conn?: WebSocketLike;
}

export function doLogin(
  url: string,
  req: LoginBody,
  opts: DoLoginOpts,
): Promise<DoLoginResult> {
  return new Promise((resolve) => {
    let settled = false;
    const conn = opts.websocket(url);
    conn.binaryType = "arraybuffer";

    const finish = (result: DoLoginResult) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(tr);
      resolve(result);
    };

    const tr = setTimeout(() => {
      try {
        conn.close();
      } catch {
        /* ignore */
      }
      finish({
        success: false,
        err: new Error("login timeout (chat unreachable?)"),
        conn,
      });
    }, opts.timeoutMs);

    conn.onerror = () => {
      finish({ success: false, err: new Error("websocket error"), conn });
    };

    conn.onclose = () => {
      finish({
        success: false,
        err: new Error("connection closed before login"),
        conn,
      });
    };

    conn.onopen = () => {
      if (conn.readyState !== WS_OPEN) {
        return;
      }
      const pkt = LogicPkt.build(
        Command.SignIn,
        "",
        encodeLoginReq(req.token, req.device ?? "web"),
        1,
      );
      conn.send(pkt.bytes());
    };

    conn.onmessage = (evt) => {
      void (async () => {
        try {
          const raw = evt.data;
          const buf =
            typeof Blob !== "undefined" && raw instanceof Blob
              ? new Uint8Array(await raw.arrayBuffer())
              : toUint8(raw);
          const wire = readPacket(buf);
          if (wire.kind === "basic") {
            return;
          }
          const pkt = wire.pkt;
          if (pkt.flag !== Flag.Response && pkt.flag !== 1) {
            return;
          }
          if (pkt.status !== Status.Success) {
            finish({
              success: false,
              err: new Error(`login status ${pkt.status}`),
              conn,
            });
            return;
          }
          const resp = decodeLoginResp(pkt.payload);
          const channelId = resp.channelId || pkt.channelId;
          if (!channelId) {
            finish({
              success: false,
              err: new Error("login missing channelId"),
              conn,
            });
            return;
          }
          let account: string;
          try {
            account = accountFromToken(req.token);
          } catch (err) {
            finish({
              success: false,
              err: err instanceof Error ? err : new Error(String(err)),
              conn,
            });
            return;
          }
          finish({
            success: true,
            channelId,
            account,
            conn,
          });
        } catch (err) {
          finish({
            success: false,
            err: err instanceof Error ? err : new Error(String(err)),
            conn,
          });
        }
      })();
    };
  });
}
