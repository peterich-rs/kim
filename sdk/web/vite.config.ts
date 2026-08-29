import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

export default defineConfig({
  root: "demo",
  resolve: {
    alias: {
      "@kim/web-sdk": fileURLToPath(new URL("./src/index.ts", import.meta.url)),
    },
  },
  server: {
    host: true,
    port: 5173,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  optimizeDeps: {
    include: ["protobufjs", "long"],
  },
});
