import { requireNativeModule } from "expo-modules-core";
import { Linking } from "react-native";
import { BridgeError, decode } from "./protocol";
import type { Command } from "./types";

export { BridgeError } from "./protocol";
export * from "./types";

export async function call<T>(command: Command): Promise<T> {
  let native: { dispatch(input: string): Promise<string> };
  try {
    native = requireNativeModule("OshwhubCore");
  } catch {
    throw new BridgeError(
      "native_unavailable",
      "请使用包含 Rust 核心的开发构建或正式安装包",
      "bridge",
    );
  }
  return decode<T>(await native.dispatch(JSON.stringify(command)));
}

export async function openExternal(url: string): Promise<void> {
  if (!/^https?:\/\//i.test(url))
    throw new BridgeError("invalid_url", "链接必须使用 HTTP 或 HTTPS", "bridge");
  await Linking.openURL(url);
}
