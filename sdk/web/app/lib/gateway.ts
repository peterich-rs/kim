export function gatewayUrl(): string {
  if (import.meta.env.PROD) {
    return `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/`;
  }
  const ws = import.meta.env.VITE_KIM_WS;
  if (typeof ws === "string" && ws.length > 0) {
    return ws.endsWith("/") ? ws : `${ws}/`;
  }
  return "ws://127.0.0.1:8001/";
}
