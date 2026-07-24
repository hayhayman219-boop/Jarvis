import { useState } from "react";
import { listAppleEvents, listGoogleEvents } from "../../lib/apiClient";
import { usePolling } from "../../hooks/usePolling";
import type { AppleEvent } from "../../lib/types";

const POLL_INTERVAL_MS = 3 * 60 * 1000;

function formatWhen(ev: AppleEvent): string {
  if (ev.all_day) {
    const d = new Date(`${ev.start}T00:00:00`);
    return `${d.toLocaleDateString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
    })} · all day`;
  }
  const d = new Date(ev.start);
  return d.toLocaleString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function ScheduleWidget() {
  const [events, setEvents] = useState<AppleEvent[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  usePolling(
    () => {
      // Pull from both calendar sources; show whatever's configured. Only
      // surface an error if BOTH fail (e.g. neither is set up yet).
      Promise.allSettled([listGoogleEvents(), listAppleEvents()]).then((results) => {
        const merged: AppleEvent[] = [];
        const errors: string[] = [];
        for (const r of results) {
          if (r.status === "fulfilled") merged.push(...r.value);
          else errors.push(String(r.reason));
        }
        if (merged.length === 0 && errors.length === results.length) {
          setError(errors.join(" · "));
          setEvents([]);
          return;
        }
        merged.sort((a, b) => a.start.localeCompare(b.start));
        setEvents(merged);
        setError(null);
      });
    },
    POLL_INTERVAL_MS,
    [],
  );

  return (
    <div className="jarvis-panel" style={{ padding: 12, display: "flex", flexDirection: "column", gap: 8 }}>
      <span className="jarvis-label">Schedule</span>
      {error && (
        <div style={{ fontSize: "0.75rem", color: "var(--jarvis-text-dim)", lineHeight: 1.4 }}>
          {error.replace(/^Error:\s*/, "")}
        </div>
      )}
      {!error && !events && (
        <div style={{ fontSize: "0.8rem", color: "var(--jarvis-text-dim)" }}>Loading…</div>
      )}
      {!error && events && events.length === 0 && (
        <div style={{ fontSize: "0.8rem", color: "var(--jarvis-text-dim)" }}>
          Nothing scheduled in the next three weeks.
        </div>
      )}
      {!error &&
        events?.slice(0, 8).map((ev, i) => (
          <div key={i} style={{ fontSize: "0.8rem" }}>
            <div style={{ color: "var(--jarvis-text-bright)" }}>{ev.summary}</div>
            <div style={{ color: "var(--jarvis-text-dim)", fontSize: "0.72rem" }}>
              {formatWhen(ev)}
              {ev.calendar ? ` · ${ev.calendar}` : ""}
            </div>
          </div>
        ))}
    </div>
  );
}
