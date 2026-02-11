export const THEME_STORAGE_KEY = 'oatty-theme-mode';

export type ThemeMode = 'system' | 'light' | 'dark' | 'high-contrast';

const SUPPORTED_THEMES: ThemeMode[] = ['system', 'light', 'dark', 'high-contrast'];

export function normalizeTheme(value: string | null): ThemeMode {
  if (!value) {
    return 'system';
  }

  return SUPPORTED_THEMES.includes(value as ThemeMode) ? (value as ThemeMode) : 'system';
}

export function getStoredThemeMode(): ThemeMode {
  return normalizeTheme(localStorage.getItem(THEME_STORAGE_KEY));
}

export function applyThemeMode(mode: ThemeMode): void {
  document.documentElement.dataset.theme = mode;
  localStorage.setItem(THEME_STORAGE_KEY, mode);
}
