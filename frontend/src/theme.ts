export type Theme = "light" | "dark";

export const themeStorageKey = "pdf-tools-theme";

export function initialTheme(): Theme {
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

export function readStoredTheme(): Theme | null {
  const value = localStorage.getItem(themeStorageKey);
  return value === "light" || value === "dark" ? value : null;
}

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
}

export function nextTheme(theme: Theme): Theme {
  return theme === "dark" ? "light" : "dark";
}
