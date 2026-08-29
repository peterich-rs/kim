/**
 * Serve the H5 SPA from Workers Static Assets.
 * Auth protobuf and the gateway WebSocket stay on the VPS origin
 * (same hostname, orange-clouded DNS). fetch(request) must not recurse
 * into this Worker — that only holds for Routes, not Custom Domains.
 */
export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    const upgrade = request.headers.get("Upgrade")?.toLowerCase();
    if (upgrade === "websocket" || url.pathname.startsWith("/api/")) {
      try {
        return await fetch(request);
      } catch (err) {
        console.error("origin fetch failed", err);
        return new Response("Bad Gateway", { status: 502 });
      }
    }
    return env.ASSETS.fetch(request);
  },
} satisfies ExportedHandler<Env>;
