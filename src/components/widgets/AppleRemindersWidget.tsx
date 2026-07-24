import { useState } from "react";
import { listAppleReminders } from "../../lib/apiClient";
import { usePolling } from "../../hooks/usePolling";
import type { AppleReminder } from "../../lib/types";

const POLL_INTERVAL_MS = 5 * 60 * 1000;

function formatDue(due: string): string {
  // All-day due dates come through as "YYYY-MM-DD"; timed ones as RFC3339.
  const d = due.length === 10 ? new Date(`${due}T00:00:00`) : new Date(due);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// Read-only view of incomplete iCloud (Apple) Reminders. Distinct from the
// local voice-created reminders — these mirror the Reminders app.
export function AppleRemindersWidget() {
  const [reminders, setReminders] = useState<AppleReminder[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  usePolling(
    () => {
      listAppleReminders()
        .then((r) => {
          setReminders(r);
          setError(null);
        })
        .catch((e) => setError(String(e)));
    },
    POLL_INTERVAL_MS,
    [],
  );

  return (
    <div className="jarvis-panel" style={{ padding: 12, display: "flex", flexDirection: "column", gap: 6 }}>
      <span className="jarvis-label">Reminders</span>
      {error && (
        <div style={{ fontSize: "0.75rem", color: "var(--jarvis-text-dim)", lineHeight: 1.4 }}>
          {error.replace(/^Error:\s*/, "")}
        </div>
      )}
      {!error && !reminders && (
        <div style={{ fontSize: "0.8rem", color: "var(--jarvis-text-dim)" }}>Loading…</div>
      )}
      {!error && reminders && reminders.length === 0 && (
        <div style={{ fontSize: "0.8rem", color: "var(--jarvis-text-dim)" }}>All caught up.</div>
      )}
      {!error &&
        reminders?.slice(0, 12).map((r, i) => (
          <div key={i} style={{ display: "flex", alignItems: "baseline", gap: 8, fontSize: "0.8rem" }}>
            <span style={{ color: "var(--jarvis-cyan-dim)" }}>○</span>
            <span style={{ flex: 1 }}>{r.summary}</span>
            {r.due && (
              <span style={{ color: "var(--jarvis-text-dim)", fontSize: "0.7rem", whiteSpace: "nowrap" }}>
                {formatDue(r.due)}
              </span>
            )}
          </div>
        ))}
    </div>
  );
}
