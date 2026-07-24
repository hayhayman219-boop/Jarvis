import { useSettingsStore } from "../../state/settingsStore";
import { useSystemStatusStore } from "../../state/systemStatusStore";

export function StatusStrip() {
  const location = useSettingsStore((s) => s.location);
  const wakeWordEnabled = useSettingsStore((s) => s.wakeWordEnabled);
  const ollamaReachable = useSystemStatusStore((s) => s.ollamaReachable);

  return (
    <div
      style={{
        position: "absolute",
        bottom: 24,
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        gap: 40,
        fontSize: 12,
        letterSpacing: 2,
        color: "var(--jarvis-text-dim)",
      }}
    >
      <div>
        NETWORK{" "}
        <span style={{ color: ollamaReachable ? "var(--jarvis-cyan)" : "var(--jarvis-red)" }}>
          {ollamaReachable ? "SECURE" : "OFFLINE"}
        </span>
      </div>
      <div>
        LOCATION <span style={{ color: "var(--jarvis-cyan)" }}>{location?.name.toUpperCase() ?? "NOT SET"}</span>
      </div>
      <div>
        VOICE LINK{" "}
        <span style={{ color: "var(--jarvis-cyan)" }}>{wakeWordEnabled ? "ACTIVE" : "STANDBY"}</span>
      </div>
    </div>
  );
}
