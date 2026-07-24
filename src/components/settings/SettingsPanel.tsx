import { ModelSelect } from "./ModelSelect";
import { LocationSetting } from "./LocationSetting";
import { useSettingsStore } from "../../state/settingsStore";

export function SettingsPanel() {
  const voiceRepliesEnabled = useSettingsStore((s) => s.voiceRepliesEnabled);
  const setVoiceRepliesEnabled = useSettingsStore((s) => s.setVoiceRepliesEnabled);
  const wakeWordEnabled = useSettingsStore((s) => s.wakeWordEnabled);
  const setWakeWordEnabled = useSettingsStore((s) => s.setWakeWordEnabled);

  return (
    <div
      className="jarvis-panel"
      style={{ padding: 16, display: "flex", flexDirection: "column", gap: 16 }}
    >
      <span className="jarvis-label" style={{ fontSize: "0.85rem" }}>Settings</span>
      <ModelSelect />
      <LocationSetting />
      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "0.8rem" }}>
        <input
          type="checkbox"
          checked={voiceRepliesEnabled}
          onChange={(e) => setVoiceRepliesEnabled(e.currentTarget.checked)}
        />
        Speak replies aloud
      </label>
      <div>
        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "0.8rem" }}>
          <input
            type="checkbox"
            checked={wakeWordEnabled}
            onChange={(e) => setWakeWordEnabled(e.currentTarget.checked)}
          />
          Listen for "Jarvis" / "Stop"
        </label>
        <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)", marginTop: 4 }}>
          Uses the mic continuously in the background. Without headphones, Jarvis may
          occasionally mishear its own voice while speaking.
        </div>
      </div>
      <VoiceInfo />
      <NotionSettings />
      <ComicVineSettings />
      <GoogleCalendarSettings />
      <AppleSettings />
      <NewsMonitorSettings />
    </div>
  );
}

const NEWS_CATEGORIES = ["ai", "tech", "pokemon", "datacenter"] as const;

function NewsMonitorSettings() {
  const enabled = useSettingsStore((s) => s.newsMonitorEnabled);
  const aloud = useSettingsStore((s) => s.newsMonitorAloud);
  const categories = useSettingsStore((s) => s.newsMonitorCategories);
  const setEnabled = useSettingsStore((s) => s.setNewsMonitorEnabled);
  const setAloud = useSettingsStore((s) => s.setNewsMonitorAloud);
  const setCategories = useSettingsStore((s) => s.setNewsMonitorCategories);

  const toggleCategory = (cat: string) => {
    setCategories(
      categories.includes(cat)
        ? categories.filter((c) => c !== cat)
        : [...categories, cat],
    );
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">News monitoring</span>
      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "0.8rem" }}>
        <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.currentTarget.checked)} />
        Watch news in the background &amp; notify on breaking headlines
      </label>
      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "0.8rem", opacity: enabled ? 1 : 0.5 }}>
        <input type="checkbox" checked={aloud} disabled={!enabled} onChange={(e) => setAloud(e.currentTarget.checked)} />
        Also announce aloud
      </label>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 10, opacity: enabled ? 1 : 0.5 }}>
        {NEWS_CATEGORIES.map((cat) => (
          <label key={cat} style={{ display: "flex", alignItems: "center", gap: 4, fontSize: "0.75rem" }}>
            <input
              type="checkbox"
              checked={categories.includes(cat)}
              disabled={!enabled}
              onChange={() => toggleCategory(cat)}
            />
            {cat}
          </label>
        ))}
      </div>
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        Checks your selected feeds every 15 minutes and pops a desktop
        notification for genuinely new stories (never re-alerts ones you've seen).
      </div>
    </div>
  );
}

function GoogleCalendarSettings() {
  const urls = useSettingsStore((s) => s.googleCalUrls);
  const setUrls = useSettingsStore((s) => s.setGoogleCalUrls);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Google Calendar</span>
      <textarea
        value={urls}
        placeholder="Secret iCal URL(s) or .ics file path(s) — one per line"
        onChange={(e) => setUrls(e.currentTarget.value)}
        rows={3}
        style={{ ...settingsInputStyle, resize: "vertical", fontFamily: "inherit" }}
      />
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        One source per line. For live updates: Google Calendar → Settings → your
        calendar → Integrate calendar → copy the <em>Secret address in iCal
        format</em>. You can also point at exported <code>.ics</code> files by
        their path. Recurring events are expanded automatically.
      </div>
    </div>
  );
}

function AppleSettings() {
  const appleId = useSettingsStore((s) => s.appleId);
  const appleAppPassword = useSettingsStore((s) => s.appleAppPassword);
  const setAppleId = useSettingsStore((s) => s.setAppleId);
  const setAppleAppPassword = useSettingsStore((s) => s.setAppleAppPassword);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Apple — Calendar &amp; Reminders</span>
      <input
        value={appleId}
        placeholder="Apple ID (email)"
        onChange={(e) => setAppleId(e.currentTarget.value)}
        style={settingsInputStyle}
      />
      <input
        type="password"
        value={appleAppPassword}
        placeholder="App-specific password (xxxx-xxxx-xxxx-xxxx)"
        onChange={(e) => setAppleAppPassword(e.currentTarget.value)}
        style={settingsInputStyle}
      />
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        Generate an app-specific password at appleid.apple.com → Sign-In and
        Security → App-Specific Passwords (your normal password won't work with
        two-factor auth). Your iCloud Calendar and Reminders then show on the
        main screen.
      </div>
    </div>
  );
}

function NotionSettings() {
  const token = useSettingsStore((s) => s.notionToken);
  const setToken = useSettingsStore((s) => s.setNotionToken);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Notion</span>
      <input
        type="password"
        value={token}
        placeholder="Integration token (secret_…)"
        onChange={(e) => setToken(e.currentTarget.value)}
        style={{ padding: "6px 8px", borderRadius: 4 }}
      />
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        Create an internal integration at notion.so/my-integrations, paste its
        token here, then in Notion share each page with it (page ••• menu →
        Connections). Jarvis can only see pages you share.
      </div>
    </div>
  );
}

function ComicVineSettings() {
  const key = useSettingsStore((s) => s.comicVineApiKey);
  const setKey = useSettingsStore((s) => s.setComicVineApiKey);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Comic Vine (Hacks)</span>
      <input
        type="password"
        value={key}
        placeholder="API key"
        onChange={(e) => setKey(e.currentTarget.value)}
        style={{ padding: "6px 8px", borderRadius: 4 }}
      />
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        Free key from comicvine.gamespot.com/api — lets Hacks pull real comic
        data and cover art. Ask Hacks to "look up" a comic or "show me the cover".
      </div>
    </div>
  );
}

const settingsInputStyle: React.CSSProperties = {
  background: "var(--jarvis-bg)",
  color: "var(--jarvis-text)",
  border: "1px solid var(--jarvis-cyan-dim)",
  padding: "4px 8px",
  borderRadius: 4,
  width: "100%",
};

function VoiceInfo() {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Voice</span>
      <div style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>
        Speech is fully local (Piper) — no cloud, no account, no quota. Each AI
        has its own voice: Jarvis speaks as Jenny (British female), Hacks as
        Ryan. Voices are set per persona in the app.
      </div>
    </div>
  );
}
