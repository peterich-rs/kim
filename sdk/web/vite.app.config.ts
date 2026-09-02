import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

const PROD_API = "https://kim.ainexc.com";
const LOCAL_API = "http://127.0.0.1:8080";
const LOCAL_WS = "ws://127.0.0.1:8001/";

function toWsUrl(httpOrigin: string): string {
  const u = new URL(httpOrigin);
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  u.pathname = "/";
  u.search = "";
  u.hash = "";
  return u.toString();
}

export default defineConfig(({ mode }) => {
  const envDir = fileURLToPath(new URL(".", import.meta.url));
  const env = loadEnv(mode, envDir, "");
  const loopback = mode === "loopback";
  const apiOrigin = (
    env.KIM_ORIGIN ||
    env.VITE_KIM_ORIGIN ||
    (loopback ? LOCAL_API : PROD_API)
  ).replace(/\/$/, "");
  const wsUrl =
    env.KIM_WS || env.VITE_KIM_WS || (loopback ? LOCAL_WS : toWsUrl(apiOrigin));

  if (mode !== "production") {
    console.info(`[kim] /api → ${apiOrigin}`);
    console.info(`[kim] ws   → ${wsUrl}`);
  }

  return {
    root: "app",
    envDir,
    plugins: [react()],
    define: {
      "import.meta.env.VITE_KIM_ORIGIN": JSON.stringify(apiOrigin),
      "import.meta.env.VITE_KIM_WS": JSON.stringify(wsUrl),
    },
    resolve: {
      alias: {
        "@kim/web-sdk": fileURLToPath(new URL("./src/index.ts", import.meta.url)),
      },
    },
    server: {
      host: true,
      port: 5173,
      proxy: {
        "/api": {
          target: apiOrigin,
          changeOrigin: true,
        },
      },
    },
    build: {
      outDir: "../dist-app",
      emptyOutDir: true,
    },
    optimizeDeps: {
      include: ["protobufjs", "long", "@mui/material", "@emotion/react", "@emotion/styled"],
    },
  };
});
