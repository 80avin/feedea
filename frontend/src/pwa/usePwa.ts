import { useSyncExternalStore } from "react";
import { registerSW } from "virtual:pwa-register";

type InstallPrompt = Event & { prompt: () => Promise<void>; userChoice: Promise<{ outcome: "accepted" | "dismissed" }> };

interface PwaState {
  installPrompt: InstallPrompt | null;
  isInstalled: boolean;
  needRefresh: boolean;
  offlineReady: boolean;
}

let state: PwaState = { installPrompt: null, isInstalled: false, needRefresh: false, offlineReady: false };
const listeners = new Set<() => void>();

function setState(patch: Partial<PwaState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

function getSnapshot(): PwaState {
  return state;
}

if (typeof window !== "undefined") {
  const mql = window.matchMedia("(display-mode: standalone)");
  const isStandalone = () => mql.matches || (navigator as unknown as { standalone?: boolean }).standalone === true;
  state.isInstalled = isStandalone();
  mql.addEventListener("change", () => setState({ isInstalled: isStandalone() }));
  window.addEventListener("appinstalled", () => setState({ isInstalled: true, installPrompt: null }));
  window.addEventListener("beforeinstallprompt", (e) => {
    e.preventDefault();
    setState({ installPrompt: e as InstallPrompt });
  });

  registerSW({
    immediate: true,
    onNeedRefresh: () => setState({ needRefresh: true }),
    onOfflineReady: () => setState({ offlineReady: true }),
  });
}

export function usePwa() {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export async function installApp(): Promise<boolean> {
  const prompt = state.installPrompt;
  if (!prompt) return false;
  await prompt.prompt();
  const { outcome } = await prompt.userChoice;
  if (outcome === "accepted") {
    setState({ installPrompt: null, isInstalled: true });
    return true;
  }
  return false;
}

export async function applyUpdate(): Promise<void> {
  const registration = await navigator.serviceWorker.getRegistration();
  registration?.waiting?.postMessage({ type: "SKIP_WAITING" });
  window.location.reload();
}
