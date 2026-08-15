import { useEffect, useState } from "react";
import { useSession } from "../auth/useSession";
import { useSettings } from "../state/hooks";

export type ThemeMode = "light" | "dark" | "system";
export type AccentName = "blue" | "zinc" | "emerald" | "violet";

export const ACCENTS = [
  { id: "blue", label: "Blue (default)", color: 'oklch(0.6204 0.195 253.83)' },
  { id: "zinc", label: "Zinc", color: 'oklch(0.442 0.017 285.79)' },
  { id: "emerald", label: "Emerald", color: 'oklch(0.596 0.145 163.225)' },
  { id: "violet", label: "Violet", color: 'oklch(0.541 0.281 293.009)' },
] as const;

const THEME_KEY = "feedea.theme";
const ACCENT_KEY = "feedea.accent";
const DARK_QUERY = "(prefers-color-scheme: dark)";
const DARK_COLOR = "#09090b";
const LIGHT_COLOR = "#f4f4f5";

const VALID_MODES = new Set<string>(["light", "dark", "system"]);
const VALID_ACCENTS = new Set<string>(ACCENTS.map((a) => a.id));

function readStored(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

function writeStored(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {}
}

function normalizeMode(value: string): ThemeMode {
  return (VALID_MODES.has(value) ? value : "dark") as ThemeMode;
}

function normalizeAccent(value: string): AccentName {
  return (VALID_ACCENTS.has(value) ? value : "blue") as AccentName;
}

function systemPrefersDark(): boolean {
  return window.matchMedia(DARK_QUERY).matches;
}

function effectiveTheme(mode: ThemeMode, systemDark: boolean): "light" | "dark" {
  if (mode === "system") {
    return systemDark ? "dark" : "light";
  }
  return mode;
}

export function useTheme() {
  const { session } = useSession();
  const { data: settings } = useSettings(!!session?.authenticated);
  const [systemDark, setSystemDark] = useState(systemPrefersDark);

  useEffect(() => {
    const mq = window.matchMedia(DARK_QUERY);
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const mode = normalizeMode(settings?.theme ?? readStored(THEME_KEY, "dark"));
  const accent = normalizeAccent(settings?.accent ?? readStored(ACCENT_KEY, "blue"));
  const theme = effectiveTheme(mode, systemDark);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", theme === "dark");
    root.setAttribute("data-accent", accent);
    root.style.colorScheme = theme;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      meta.setAttribute("content", theme === "dark" ? DARK_COLOR : LIGHT_COLOR);
    }
  }, [theme, accent]);

  useEffect(() => {
    if (settings?.theme) {
      writeStored(THEME_KEY, settings.theme);
    }
    if (settings?.accent) {
      writeStored(ACCENT_KEY, settings.accent);
    }
  }, [settings?.theme, settings?.accent]);

  return { mode, accent, theme };
}
