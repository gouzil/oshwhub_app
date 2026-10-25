export function readThemeColor(): string | null {
  return localStorage.getItem("oshwhub.themeColor");
}

export function writeThemeColor(value: string): void {
  localStorage.setItem("oshwhub.themeColor", value);
}
