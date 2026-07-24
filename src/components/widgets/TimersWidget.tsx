import { useEffect, useState } from "react";
import { useTimersStore } from "../../state/timersStore";

// A small stack of live countdowns, bottom-left. Only shown when timers/alarms
// are active. Re-renders once a second for the countdown.
function fmt(ms: number): string {
  if (ms < 0) ms = 0;
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function TimersWidget() {
  const timers = useTimersStore((s) => s.timers);
  const cancel = useTimersStore((s) => s.cancel);
  const [, tick] = useState(0);

  useEffect(() => {
    if (timers.length === 0) return;
    const id = setInterval(() => tick((n) => n + 1), 500);
    return () => clearInterval(id);
  }, [timers.length]);

  if (timers.length === 0) return null;

  return (
    <div
      style={{
        position: "absolute",
        left: 16,
        bottom: 16,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        zIndex: 5,
      }}
    >
      {timers.map((t) => {
        const remaining = t.endsAt - Date.now();
        return (
          <div
            key={t.id}
            className="jarvis-panel"
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              padding: "8px 12px",
              minWidth: 150,
              border: "1px solid var(--jarvis-cyan-dim)",
            }}
          >
            <span style={{ fontSize: "1.1rem" }}>{t.kind === "alarm" ? "⏰" : "⏳"}</span>
            <div style={{ display: "flex", flexDirection: "column", lineHeight: 1.2 }}>
              <span
                style={{
                  fontFamily: "monospace",
                  fontSize: "1.15rem",
                  color: "var(--jarvis-cyan)",
                  fontWeight: 700,
                }}
              >
                {t.kind === "alarm"
                  ? new Date(t.endsAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
                  : fmt(remaining)}
              </span>
              <span style={{ fontSize: "0.7rem", color: "var(--jarvis-text-dim)" }}>{t.label}</span>
            </div>
            <button
              onClick={() => cancel(t.id)}
              aria-label="Cancel"
              title="Cancel"
              style={{
                marginLeft: "auto",
                background: "transparent",
                border: "1px solid var(--jarvis-cyan-dim)",
                color: "var(--jarvis-text-dim)",
                borderRadius: 4,
                width: 22,
                height: 22,
                cursor: "pointer",
                fontSize: "0.7rem",
              }}
            >
              ✕
            </button>
          </div>
        );
      })}
    </div>
  );
}
