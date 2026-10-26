export const themeColors = [
  // OSHWHub uses #3673fe; deepen it for readable small text and white button labels.
  {
    id: "oshwhub",
    label: "OSHWHub 蓝",
    primary: "#2c65e8",
    light: "#edf3ff",
    darkPrimary: "#8bb3ff",
    dark: "#1d2d4b",
  },
  {
    id: "purple",
    label: "紫色",
    primary: "#7654d1",
    light: "#f4efff",
    darkPrimary: "#bfa5ff",
    dark: "#302342",
  },
  {
    id: "orange",
    label: "橙色",
    primary: "#b45309",
    light: "#fff4e6",
    darkPrimary: "#ffbd7a",
    dark: "#3b281a",
  },
] as const;

export type ThemeColor = (typeof themeColors)[number];

export function resolveThemeColor(value: unknown): ThemeColor {
  return themeColors.find((theme) => theme.id === value) ?? themeColors[0];
}

export const themeModes = [
  { id: "system", label: "跟随系统" },
  { id: "light", label: "亮色模式" },
  { id: "dark", label: "暗色模式" },
] as const;

export type ThemeMode = (typeof themeModes)[number]["id"];

export function resolveThemeMode(value: unknown): ThemeMode {
  return themeModes.find((mode) => mode.id === value)?.id ?? "system";
}
