import { requireNativeModule } from "expo-modules-core";

const preferences = requireNativeModule<{
  getThemeColor(): string | null;
  setThemeColor(value: string): void;
  getThemeMode(): string | null;
  setThemeMode(value: string): void;
}>("OshwhubCore");

export function readThemeColor(): string | null {
  return preferences.getThemeColor();
}

export function writeThemeColor(value: string): void {
  preferences.setThemeColor(value);
}

export function readThemeMode(): string | null {
  return preferences.getThemeMode();
}

export function writeThemeMode(value: string): void {
  preferences.setThemeMode(value);
}
