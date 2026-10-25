import { requireNativeModule } from "expo-modules-core";

const preferences = requireNativeModule<{
  getThemeColor(): string | null;
  setThemeColor(value: string): void;
}>("OshwhubCore");

export function readThemeColor(): string | null {
  return preferences.getThemeColor();
}

export function writeThemeColor(value: string): void {
  preferences.setThemeColor(value);
}
