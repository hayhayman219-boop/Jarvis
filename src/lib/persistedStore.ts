import { load, type Store } from "@tauri-apps/plugin-store";
import { isTauri } from "./env";

const DEFAULT_STORE = "settings.json";
const storeCache = new Map<string, Promise<Store>>();

function getStore(storeName: string): Promise<Store> {
  if (!storeCache.has(storeName)) {
    storeCache.set(storeName, load(storeName, { autoSave: true, defaults: {} }));
  }
  return storeCache.get(storeName)!;
}

// In the browser (web app) the Tauri store plugin has no IPC bridge, so
// persistence falls back to localStorage, namespaced per store file.
function webKey(storeName: string, key: string): string {
  return `jarvis:${storeName}:${key}`;
}

export async function getSetting<T>(key: string, storeName = DEFAULT_STORE): Promise<T | undefined> {
  if (!isTauri) {
    const raw = localStorage.getItem(webKey(storeName, key));
    if (raw == null) return undefined;
    try {
      return JSON.parse(raw) as T;
    } catch {
      return undefined;
    }
  }
  const store = await getStore(storeName);
  return store.get<T>(key);
}

export async function setSetting<T>(key: string, value: T, storeName = DEFAULT_STORE): Promise<void> {
  if (!isTauri) {
    localStorage.setItem(webKey(storeName, key), JSON.stringify(value));
    return;
  }
  const store = await getStore(storeName);
  await store.set(key, value);
}
