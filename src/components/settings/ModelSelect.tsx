import { useEffect, useState } from "react";
import { listModels } from "../../lib/apiClient";
import { useSettingsStore } from "../../state/settingsStore";
import type { ModelInfo } from "../../lib/types";

export function ModelSelect() {
  const model = useSettingsStore((s) => s.model);
  const hydrated = useSettingsStore((s) => s.hydrated);
  const setModel = useSettingsStore((s) => s.setModel);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listModels()
      .then(setModels)
      .catch((err) => setError(String(err)));
  }, []);

  useEffect(() => {
    if (!hydrated || model || models.length === 0) return;
    const firstLocal = models.find((m) => !m.is_remote);
    setModel((firstLocal ?? models[0]).name);
  }, [hydrated, model, models]);

  if (error) {
    return <span style={{ color: "var(--jarvis-red)" }}>Ollama unreachable: {error}</span>;
  }

  const selectedIsRemote = models.find((m) => m.name === model)?.is_remote;

  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <span className="jarvis-label">Model</span>
      <select
        value={model ?? ""}
        onChange={(e) => setModel(e.currentTarget.value)}
        style={{
          background: "var(--jarvis-bg)",
          color: "var(--jarvis-text)",
          border: "1px solid var(--jarvis-cyan-dim)",
          padding: "6px 8px",
          borderRadius: 4,
        }}
      >
        {models.length === 0 && <option value="">No models found</option>}
        {models.map((m) => (
          <option key={m.name} value={m.name}>
            {m.name}
            {m.is_remote ? " (cloud)" : ""}
          </option>
        ))}
      </select>
      {selectedIsRemote && (
        <span style={{ fontSize: "0.7rem", color: "var(--jarvis-gold)" }}>
          ⚠ This model runs in the cloud, not locally on this machine.
        </span>
      )}
    </label>
  );
}
