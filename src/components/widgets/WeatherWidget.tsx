import { useState } from "react";
import { getWeather } from "../../lib/apiClient";
import { useSettingsStore } from "../../state/settingsStore";
import { usePolling } from "../../hooks/usePolling";
import type { WeatherResponse } from "../../lib/types";

const POLL_INTERVAL_MS = 15 * 60 * 1000;

export function WeatherWidget() {
  const location = useSettingsStore((s) => s.location);
  const temperatureUnit = useSettingsStore((s) => s.temperatureUnit);
  const setTemperatureUnit = useSettingsStore((s) => s.setTemperatureUnit);
  const [weather, setWeather] = useState<WeatherResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  usePolling(
    () => {
      if (!location) return;
      getWeather(location.latitude, location.longitude, temperatureUnit === "fahrenheit")
        .then((w) => {
          setWeather(w);
          setError(null);
        })
        .catch((err) => setError(String(err)));
    },
    POLL_INTERVAL_MS,
    [location?.latitude, location?.longitude, temperatureUnit],
  );

  if (!location) {
    return (
      <div className="jarvis-panel" style={{ padding: 12 }}>
        <span className="jarvis-label">Weather</span>
        <div style={{ fontSize: "0.85rem", marginTop: 4 }}>Set a location in Settings.</div>
      </div>
    );
  }

  return (
    <div className="jarvis-panel" style={{ padding: 12 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span className="jarvis-label">Weather — {location.name}</span>
        <button
          onClick={() =>
            setTemperatureUnit(temperatureUnit === "fahrenheit" ? "celsius" : "fahrenheit")
          }
          style={{
            background: "transparent",
            border: "1px solid var(--jarvis-cyan-dim)",
            color: "var(--jarvis-text-dim)",
            borderRadius: 4,
            padding: "1px 6px",
            fontSize: "0.7rem",
            cursor: "pointer",
          }}
        >
          {temperatureUnit === "fahrenheit" ? "°F" : "°C"}
        </button>
      </div>
      {error && <div style={{ color: "var(--jarvis-red)", fontSize: "0.8rem" }}>{error}</div>}
      {weather && !error && (
        <div style={{ marginTop: 4 }}>
          <div style={{ fontSize: "1.4rem", color: "var(--jarvis-cyan)" }}>
            {weather.temperature.toFixed(1)}
            {weather.temperature_unit}
          </div>
          <div style={{ fontSize: "0.85rem", color: "var(--jarvis-text-dim)" }}>
            {weather.condition} · wind {weather.windspeed.toFixed(0)} {weather.windspeed_unit}
          </div>
        </div>
      )}
    </div>
  );
}
