import { readonly, ref } from "vue";

export type ResolvedThemeMode = "dark" | "light";
export type ThemeMode = ResolvedThemeMode | "system";

const STORAGE_KEY = "purewall-theme";
const BROADCAST_CHANNEL = "purewall-theme";
const theme = ref<ThemeMode>("dark");
const resolvedTheme = ref<ResolvedThemeMode>("dark");
let systemThemeQuery: MediaQueryList | null = null;
let themeBroadcast: BroadcastChannel | null = null;
let initialized = false;

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "system" || value === "dark";
}

function resolveSystemTheme(): ResolvedThemeMode {
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

function resolveTheme(mode: ThemeMode): ResolvedThemeMode {
  return mode === "system" ? resolveSystemTheme() : mode;
}

function applyTheme(mode: ThemeMode) {
  theme.value = mode;
  const nextResolvedTheme = resolveTheme(mode);
  resolvedTheme.value = nextResolvedTheme;
  document.documentElement.dataset.theme = nextResolvedTheme;
  document.documentElement.dataset.themePreference = mode;
  document.documentElement.style.colorScheme = nextResolvedTheme;
}

function onSystemThemeChange() {
  if (theme.value === "system") {
    applyTheme("system");
    publishTheme("system");
  }
}

function applyExternalTheme(mode: ThemeMode) {
  applyTheme(mode);
}

function onThemeStorageChange(event: StorageEvent) {
  if (event.key !== STORAGE_KEY || !isThemeMode(event.newValue)) return;
  applyExternalTheme(event.newValue);
}

function publishTheme(mode: ThemeMode) {
  try {
    themeBroadcast?.postMessage(mode);
  } catch {
    // Storage remains the durable sync path when BroadcastChannel is unavailable.
  }
}

export function initializeTheme() {
  if (initialized) return; // Prevent duplicate listeners during HMR (MED-10).

  let savedTheme: string | null = null;

  try {
    savedTheme = window.localStorage.getItem(STORAGE_KEY);
  } catch {
    // Theme selection still works for the current session when storage is unavailable.
  }

  if (!systemThemeQuery) {
    systemThemeQuery = window.matchMedia("(prefers-color-scheme: light)");
    systemThemeQuery.addEventListener("change", onSystemThemeChange);
  }

  if (!initialized) {
    window.addEventListener("storage", onThemeStorageChange);

    try {
      themeBroadcast = new BroadcastChannel(BROADCAST_CHANNEL);
      themeBroadcast.addEventListener("message", (event: MessageEvent<unknown>) => {
        if (isThemeMode(event.data)) applyExternalTheme(event.data);
      });
    } catch {
      themeBroadcast = null;
    }

    initialized = true;
  }

  const initialTheme: ThemeMode = isThemeMode(savedTheme) ? savedTheme : "dark";
  applyTheme(initialTheme);
}

export function useTheme() {
  function setTheme(mode: ThemeMode) {
    applyTheme(mode);

    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // Keep the applied theme even when storage is unavailable.
    }

    publishTheme(mode);
  }

  function cycleTheme() {
    const nextTheme: Record<ThemeMode, ThemeMode> = {
      dark: "light",
      light: "system",
      system: "dark",
    };
    setTheme(nextTheme[theme.value]);
  }

  return {
    theme: readonly(theme),
    resolvedTheme: readonly(resolvedTheme),
    setTheme,
    cycleTheme,
  };
}

export function cleanupTheme() {
  if (systemThemeQuery) {
    systemThemeQuery.removeEventListener("change", onSystemThemeChange);
    systemThemeQuery = null;
  }
  window.removeEventListener("storage", onThemeStorageChange);
  if (themeBroadcast) {
    themeBroadcast.close();
    themeBroadcast = null;
  }
  initialized = false;
}
