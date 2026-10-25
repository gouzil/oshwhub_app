export function readThemeColor(): string | null {
  return localStorage.getItem("oshwhub.themeColor");
}

export function writeThemeColor(value: string): void {
  localStorage.setItem("oshwhub.themeColor", value);
}

export function readThemeMode(): string | null {
  return localStorage.getItem("oshwhub.themeMode");
}

export function writeThemeMode(value: string): void {
  localStorage.setItem("oshwhub.themeMode", value);
}
