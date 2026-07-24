import { useState } from "react";
import { deleteReminder, listReminders, parseAndCreateReminder } from "../../lib/apiClient";
import { useSettingsStore } from "../../state/settingsStore";
import { useChatStore } from "../../state/chatStore";
import { usePolling } from "../../hooks/usePolling";
import type { Reminder } from "../../lib/types";

const POLL_INTERVAL_MS = 30 * 1000;

export function RemindersWidget() {
  const model = useSettingsStore((s) => s.model);
  const setStatus = useChatStore((s) => s.setStatus);
  const [reminders, setReminders] = useState<Reminder[]>([]);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const refresh = () => {
    listReminders()
      .then(setReminders)
      .catch((err) => setError(String(err)));
  };

  usePolling(refresh, POLL_INTERVAL_MS, []);

  async function submit() {
    const trimmed = text.trim();
    if (!trimmed || !model) return;
    setSubmitting(true);
    setError(null);
    setStatus("thinking");
    try {
      await parseAndCreateReminder(model, trimmed);
      setText("");
      refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
      setStatus("idle");
    }
  }

  async function remove(id: number) {
    await deleteReminder(id);
    refresh();
  }

  return (
    <div className="jarvis-panel" style={{ padding: 12, display: "flex", flexDirection: "column", gap: 8 }}>
      <span className="jarvis-label">Reminders</span>
      <form
        style={{ display: "flex", gap: 4 }}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input
          value={text}
          disabled={submitting || !model}
          onChange={(e) => setText(e.currentTarget.value)}
          placeholder="Remind me to..."
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
          disabled={submitting || !model}
          style={{
            background: "transparent",
            border: "1px solid var(--jarvis-cyan)",
            color: "var(--jarvis-cyan)",
            borderRadius: 4,
            padding: "4px 8px",
          }}
        >
          {submitting ? "Thinking..." : "Add"}
        </button>
      </form>
      {submitting && (
        <span style={{ fontSize: "0.75rem", color: "var(--jarvis-text-dim)" }}>
          Jarvis is parsing that with the local model — this can take several seconds...
        </span>
      )}
      {error && <span style={{ color: "var(--jarvis-red)", fontSize: "0.75rem" }}>{error}</span>}
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {reminders.length === 0 && (
          <span style={{ fontSize: "0.8rem", color: "var(--jarvis-text-dim)" }}>No reminders set.</span>
        )}
        {reminders.map((r) => (
          <div
            key={r.id}
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              fontSize: "0.8rem",
              opacity: r.fired ? 0.5 : 1,
            }}
          >
            <span>
              {r.text}
              <br />
              <span style={{ color: "var(--jarvis-text-dim)" }}>
                {new Date(r.due_at).toLocaleString()}
              </span>
            </span>
            <button
              onClick={() => remove(r.id)}
              style={{
                background: "transparent",
                border: "1px solid var(--jarvis-cyan-dim)",
                color: "var(--jarvis-text-dim)",
                borderRadius: 4,
                padding: "2px 6px",
                cursor: "pointer",
              }}
            >
              ✕
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
