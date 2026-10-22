import { LoadSkiaWeb } from "@shopify/react-native-skia/lib/module/web/index.js";
import { createRoot } from "react-dom/client";
import "./global.css";

const host = document.getElementById("root");
if (!host) throw new Error("Missing app host");
const root = createRoot(host);
root.render(<output style={{ display: "block", padding: 32 }}>正在启动 OSHWHub…</output>);

LoadSkiaWeb({ locateFile: () => "/canvaskit.wasm" })
  .then(() => import("@oshwhub/ui"))
  .then(({ App }) => root.render(<App />))
  .catch((error: unknown) => {
    console.error("app.startup.failed", error);
    root.render(
      <div role="alert" style={{ padding: 32 }}>
        无法初始化图形引擎，请重新打开应用。
      </div>,
    );
  });
