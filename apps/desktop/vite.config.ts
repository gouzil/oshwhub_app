import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const require = createRequire(import.meta.url);
const wasm = resolve(
  dirname(require.resolve("canvaskit-wasm/package.json")),
  "bin/full/canvaskit.wasm",
);
const webExtensions = [
  ".web.tsx",
  ".web.ts",
  ".web.jsx",
  ".web.js",
  ".tsx",
  ".ts",
  ".jsx",
  ".js",
  ".json",
];

export default defineConfig({
  plugins: [
    react(),
    {
      name: "local-canvaskit",
      configureServer(server) {
        server.middlewares.use("/canvaskit.wasm", (_req, res) => {
          res.setHeader("Content-Type", "application/wasm");
          res.end(readFileSync(wasm));
        });
      },
      generateBundle() {
        this.emitFile({ type: "asset", fileName: "canvaskit.wasm", source: readFileSync(wasm) });
      },
    },
  ],
  resolve: {
    alias: [
      { find: /^react-native$/, replacement: "react-native-web" },
      {
        find: /^@oshwhub\/bridge$/,
        replacement: resolve(import.meta.dirname, "../../packages/bridge/src/index.web.ts"),
      },
      {
        find: /^@oshwhub\/ui$/,
        replacement: resolve(import.meta.dirname, "../../packages/ui/src/index.ts"),
      },
      {
        find: /^@oshwhub\/pcb-viewer$/,
        replacement: resolve(import.meta.dirname, "../../packages/pcb-viewer/src/index.ts"),
      },
    ],
    extensions: webExtensions,
    dedupe: ["react", "react-dom", "react-native-web"],
  },
  define: {
    global: "globalThis",
    __DEV__: JSON.stringify(process.env.NODE_ENV !== "production"),
    "process.env.NODE_ENV": JSON.stringify(process.env.NODE_ENV || "development"),
  },
  optimizeDeps: {
    include: ["react-native-web", "react-native-reanimated"],
    // Vite's dependency optimizer has a separate resolver from the app build.
    rolldownOptions: {
      resolve: { extensions: webExtensions },
      transform: { define: { global: "globalThis" } },
    },
  },
  build: { target: "es2022", sourcemap: true },
});
