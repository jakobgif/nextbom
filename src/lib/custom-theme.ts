const STORAGE_KEY = "custom-theme-css";
const STYLE_ID = "custom-theme";

function extractThemeBlocks(css: string): string {
  const root = css.match(/:root\s*\{[^}]*\}/s)?.[0] ?? "";
  const dark = css.match(/\.dark\s*\{[^}]*\}/s)?.[0] ?? "";
  return [root, dark].filter(Boolean).join("\n");
}

export function applyCustomTheme(css: string): boolean {
  const blocks = extractThemeBlocks(css);
  if (!blocks) return false;
  let el = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!el) {
    el = document.createElement("style");
    el.id = STYLE_ID;
    document.head.appendChild(el);
  }
  el.textContent = blocks;
  localStorage.setItem(STORAGE_KEY, css);
  return true;
}

export function resetCustomTheme() {
  document.getElementById(STYLE_ID)?.remove();
  localStorage.removeItem(STORAGE_KEY);
}

export function loadSavedTheme() {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) applyCustomTheme(saved);
}

export function getSavedThemeCss(): string | null {
  return localStorage.getItem(STORAGE_KEY);
}
