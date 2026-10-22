import { BridgeError, decode } from "./protocol";
import type { Command } from "./types";

export { BridgeError } from "./protocol";
export * from "./types";

function desktop(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function call<T>(command: Command): Promise<T> {
  const input = JSON.stringify(command);
  if (desktop()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return decode<T>(await invoke<string>("dispatch", { input }));
  }
  try {
    const response = await fetch("http://127.0.0.1:4318/bridge", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: input,
      signal: AbortSignal.timeout(240_000),
    });
    if (!response.ok)
      throw new BridgeError("bridge_http", `本地核心返回 ${response.status}`, "bridge");
    return decode<T>(await response.json());
  } catch (error) {
    if (error instanceof BridgeError) throw error;
    throw new BridgeError("core_unavailable", "无法连接本地 Rust 核心，请运行 pnpm dev", "bridge");
  }
}

export async function openExternal(url: string): Promise<void> {
  if (!/^https?:\/\//i.test(url))
    throw new BridgeError("invalid_url", "链接必须使用 HTTP 或 HTTPS", "bridge");
  if (desktop()) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
