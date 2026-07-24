import { create } from "zustand";
import { setVoiceLoopEnabled } from "../lib/apiClient";
import { getSetting, setSetting } from "../lib/persistedStore";

export interface Location {
  name: string;
  latitude: number;
  longitude: number;
}

export type TemperatureUnit = "fahrenheit" | "celsius";

interface SettingsState {
  model: string | null;
  location: Location | null;
  temperatureUnit: TemperatureUnit;
  voiceRepliesEnabled: boolean;
  wakeWordEnabled: boolean;
  notionToken: string;
  /** Apple ID + app-specific password for iCloud CalDAV (Calendar/Reminders).
   * Read backend-side from settings.json; empty means "not connected". */
  appleId: string;
  appleAppPassword: string;
  /** Google Calendar "secret iCal" feed URLs (whitespace/comma separated).
   * Read backend-side; no OAuth needed. */
  googleCalUrls: string;
  /** Comic Vine API key (free) for Hacks' comic data + cover lookups.
   * Read backend-side from settings.json; empty means "not configured". */
  comicVineApiKey: string;
  /** Background news monitor: watch feeds while idle and notify on new
   * headlines. Read backend-side (news_monitor.rs) from settings.json. */
  newsMonitorEnabled: boolean;
  newsMonitorAloud: boolean;
  newsMonitorCategories: string[];
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setModel: (model: string) => void;
  setLocation: (location: Location) => void;
  setTemperatureUnit: (unit: TemperatureUnit) => void;
  setVoiceRepliesEnabled: (enabled: boolean) => void;
  setWakeWordEnabled: (enabled: boolean) => void;
  setNotionToken: (token: string) => void;
  setAppleId: (id: string) => void;
  setAppleAppPassword: (pass: string) => void;
  setGoogleCalUrls: (urls: string) => void;
  setComicVineApiKey: (key: string) => void;
  setNewsMonitorEnabled: (v: boolean) => void;
  setNewsMonitorAloud: (v: boolean) => void;
  setNewsMonitorCategories: (v: string[]) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  model: null,
  location: null,
  temperatureUnit: "fahrenheit",
  voiceRepliesEnabled: true,
  wakeWordEnabled: true,
  notionToken: "",
  appleId: "",
  appleAppPassword: "",
  googleCalUrls: "",
  comicVineApiKey: "",
  newsMonitorEnabled: false,
  newsMonitorAloud: false,
  newsMonitorCategories: ["ai", "tech"],
  hydrated: false,
  hydrate: async () => {
    const [
      model,
      location,
      temperatureUnit,
      voiceRepliesEnabled,
      wakeWordEnabled,
      notionToken,
      appleId,
      appleAppPassword,
      googleCalUrls,
      comicVineApiKey,
      newsMonitorEnabled,
      newsMonitorAloud,
      newsMonitorCategories,
    ] = await Promise.all([
      getSetting<string>("model"),
      getSetting<Location>("location"),
      getSetting<TemperatureUnit>("temperatureUnit"),
      getSetting<boolean>("voiceRepliesEnabled"),
      getSetting<boolean>("wakeWordEnabled"),
      getSetting<string>("notionToken"),
      getSetting<string>("appleId"),
      getSetting<string>("appleAppPassword"),
      getSetting<string>("googleCalUrls"),
      getSetting<string>("comicVineApiKey"),
      getSetting<boolean>("newsMonitorEnabled"),
      getSetting<boolean>("newsMonitorAloud"),
      getSetting<string[]>("newsMonitorCategories"),
    ]);
    set({
      model: model ?? null,
      location: location ?? null,
      temperatureUnit: temperatureUnit ?? "fahrenheit",
      voiceRepliesEnabled: voiceRepliesEnabled ?? true,
      wakeWordEnabled: wakeWordEnabled ?? true,
      notionToken: notionToken ?? "",
      appleId: appleId ?? "",
      appleAppPassword: appleAppPassword ?? "",
      googleCalUrls: googleCalUrls ?? "",
      comicVineApiKey: comicVineApiKey ?? "",
      newsMonitorEnabled: newsMonitorEnabled ?? false,
      newsMonitorAloud: newsMonitorAloud ?? false,
      newsMonitorCategories: newsMonitorCategories ?? ["ai", "tech"],
      hydrated: true,
    });
    // Keep the desktop backend's always-on listener in sync with the
    // persisted preference (no-op on web).
    void setVoiceLoopEnabled(wakeWordEnabled ?? true);
  },
  setModel: (model) => {
    set({ model });
    void setSetting("model", model);
  },
  setLocation: (location) => {
    set({ location });
    void setSetting("location", location);
  },
  setTemperatureUnit: (unit) => {
    set({ temperatureUnit: unit });
    void setSetting("temperatureUnit", unit);
  },
  setVoiceRepliesEnabled: (enabled) => {
    set({ voiceRepliesEnabled: enabled });
    void setSetting("voiceRepliesEnabled", enabled);
  },
  setWakeWordEnabled: (enabled) => {
    set({ wakeWordEnabled: enabled });
    void setSetting("wakeWordEnabled", enabled);
    void setVoiceLoopEnabled(enabled);
  },
  setNotionToken: (token) => {
    set({ notionToken: token });
    void setSetting("notionToken", token);
  },
  setAppleId: (id) => {
    set({ appleId: id });
    void setSetting("appleId", id);
  },
  setAppleAppPassword: (pass) => {
    set({ appleAppPassword: pass });
    void setSetting("appleAppPassword", pass);
  },
  setComicVineApiKey: (key) => {
    set({ comicVineApiKey: key });
    void setSetting("comicVineApiKey", key);
  },
  setGoogleCalUrls: (urls) => {
    set({ googleCalUrls: urls });
    void setSetting("googleCalUrls", urls);
  },
  setNewsMonitorEnabled: (v) => {
    set({ newsMonitorEnabled: v });
    void setSetting("newsMonitorEnabled", v);
  },
  setNewsMonitorAloud: (v) => {
    set({ newsMonitorAloud: v });
    void setSetting("newsMonitorAloud", v);
  },
  setNewsMonitorCategories: (v) => {
    set({ newsMonitorCategories: v });
    void setSetting("newsMonitorCategories", v);
  },
}));
