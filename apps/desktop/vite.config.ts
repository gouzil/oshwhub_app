import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { uniwind } from "uniwind/vite";
import { defineConfig, transformWithOxc } from "vite";

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
    {
      name: "react-native-primitives-jsx",
      enforce: "pre",
      transform(code, id) {
        // RN Primitives ships JSX in .mjs files for Metro.
        const filename = id.split("?")[0];
        if (/[/\\]@rn-primitives[/\\].*\.mjs$/.test(filename)) {
          return transformWithOxc(code, filename, { lang: "jsx" });
        }
      },
    },
    react(),
    tailwindcss(),
    uniwind({
      cssEntryFile: "./src/global.css",
      dtsFile: "../../packages/ui/src/uniwind-types.d.ts",
    }),
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
    dedupe: ["react", "react-dom", "react-native-web", "uniwind"],
  },
  define: {
    global: "globalThis",
    __DEV__: JSON.stringify(process.env.NODE_ENV !== "production"),
    "process.env.NODE_ENV": JSON.stringify(process.env.NODE_ENV || "development"),
  },
  optimizeDeps: {
    include: ["react-native-web", "react-native-reanimated"],
    exclude: ["@rn-primitives/slot"],
    // Vite's dependency optimizer has a separate resolver from the app build.
    rolldownOptions: {
      resolve: { extensions: webExtensions },
      transform: { define: { global: "globalThis" } },
    },
  },
  build: { target: "es2022", sourcemap: true },
});
