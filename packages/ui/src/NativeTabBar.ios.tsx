import { requireNativeViewManager } from "expo-modules-core";
import type { NativeTabBarProps } from "./NativeTabBar";

export const NativeTabBar = requireNativeViewManager<NativeTabBarProps>(
  "OshwhubCore",
  "LiquidTabBar",
);
