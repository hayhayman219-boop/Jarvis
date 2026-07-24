import { useState } from "react";
import { SettingsPanel } from "../settings/SettingsPanel";
import { WeatherWidget } from "../widgets/WeatherWidget";
import { RemindersWidget } from "../widgets/RemindersWidget";

export function SidePanel() {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        onClick={() => setOpen((o) => !o)}
        aria-label="Toggle settings panel"
        style={{
          position: "absolute",
          // Top-right corner: clears the panel's left-aligned "SETTINGS"
          // heading when open (the panel is 320px wide from the right edge,
          // so right:260 used to sit directly over that heading).
          top: 16,
          right: 16,
          background: "rgba(4,8,14,0.6)",
          border: "1px solid var(--jarvis-cyan-dim)",
          color: "var(--jarvis-cyan)",
          borderRadius: 4,
          width: 32,
          height: 32,
          cursor: "pointer",
          fontSize: 16,
          zIndex: 10,
        }}
      >
        ⚙
      </button>
      {open && (
        <div
          style={{
            position: "absolute",
            top: 0,
            right: 0,
            bottom: 0,
            width: 320,
            display: "flex",
            flexDirection: "column",
            gap: 12,
            padding: 16,
            background: "rgba(4,8,11,0.92)",
            borderLeft: "1px solid var(--jarvis-cyan-dim)",
            overflowY: "auto",
            zIndex: 9,
          }}
        >
          <SettingsPanel />
          <WeatherWidget />
          <RemindersWidget />
        </div>
      )}
    </>
  );
}
