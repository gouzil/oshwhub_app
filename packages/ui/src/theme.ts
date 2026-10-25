export const themeColors = [
  // OSHWHub uses #3673fe; deepen it for readable small text and white button labels.
  { id: "oshwhub", label: "OSHWHub 蓝", primary: "#2c65e8", light: "#edf3ff" },
  { id: "purple", label: "紫色", primary: "#7654d1", light: "#f4efff" },
  { id: "orange", label: "橙色", primary: "#b45309", light: "#fff4e6" },
] as const;

export type ThemeColor = (typeof themeColors)[number];

export function resolveThemeColor(value: unknown): ThemeColor {
  return themeColors.find((theme) => theme.id === value) ?? themeColors[0];
}
