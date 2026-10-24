import { requireNativeViewManager } from "expo-modules-core";
import { withUniwind } from "uniwind";
import type { NativeTabBarProps } from "./NativeTabBar";

export const NativeTabBar = withUniwind(
  requireNativeViewManager<NativeTabBarProps>("OshwhubCore", "LiquidTabBar"),
);
