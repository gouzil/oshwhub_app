import type { ComponentType } from "react";
import { type NativeSyntheticEvent, View, type ViewProps } from "react-native";

export interface NativeTabBarProps extends ViewProps {
  items: { label: string; symbol: string }[];
  selectedIndex: number;
  tintColor: string;
  onSelect: (event: NativeSyntheticEvent<{ index: number }>) => void;
}

// Only the iOS implementation is mounted; other platforms use the shared navigation.
export const NativeTabBar: ComponentType<NativeTabBarProps> = View;
