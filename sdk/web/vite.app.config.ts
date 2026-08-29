import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  root: "app",
  resolve: {
    alias: {
      "@kim/web-sdk": fileURLToPath(new URL("./src/index.ts", import.meta.url)),
    },
  },
  server: {
    host: true,
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8080",
    },
  },
  build: {
    outDir: "../dist-app",
    emptyOutDir: true,
  },
  optimizeDeps: {
    include: ["protobufjs", "long"],
  },
});
