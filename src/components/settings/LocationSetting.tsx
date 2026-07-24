import { useState } from "react";
import { geocodeCity } from "../../lib/apiClient";
import { useSettingsStore } from "../../state/settingsStore";

export function LocationSetting() {
  const location = useSettingsStore((s) => s.location);
  const setLocation = useSettingsStore((s) => s.setLocation);
  const [city, setCity] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit() {
    const trimmed = city.trim();
    if (!trimmed) return;
    setLoading(true);
    setError(null);
    try {
      const result = await geocodeCity(trimmed);
      setLocation({
        name: result.name,
        latitude: result.latitude,
        longitude: result.longitude,
      });
      setCity("");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span className="jarvis-label">Location</span>
      {location && (
        <span style={{ fontSize: "0.85rem" }}>{location.name}</span>
      )}
      <form
        style={{ display: "flex", gap: 4 }}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input
          value={city}
          onChange={(e) => setCity(e.currentTarget.value)}
          placeholder="City name..."
          style={{
            flex: 1,
            background: "var(--jarvis-bg)",
            color: "var(--jarvis-text)",
            border: "1px solid var(--jarvis-cyan-dim)",
            padding: "4px 8px",
            borderRadius: 4,
            width: 0,
          }}
        />
        <button
          type="submit"
          disabled={loading}
          style={{
            background: "transparent",
            border: "1px solid var(--jarvis-cyan)",
            color: "var(--jarvis-cyan)",
            borderRadius: 4,
            padding: "4px 8px",
          }}
        >
          Set
        </button>
      </form>
      {error && <span style={{ color: "var(--jarvis-red)", fontSize: "0.75rem" }}>{error}</span>}
    </div>
  );
}
