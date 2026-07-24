import { create } from "zustand";
import { getSystemStats, listModels } from "../lib/apiClient";

interface SystemStatusState {
  cpuPercent: number;
  memoryPercent: number;
  diskPercent: number;
  temperatureC: number | null;
  ollamaReachable: boolean;
  modelIsRemote: boolean;
  refresh: (selectedModel: string | null) => Promise<void>;
}

export const useSystemStatusStore = create<SystemStatusState>((set) => ({
  cpuPercent: 0,
  memoryPercent: 0,
  diskPercent: 0,
  temperatureC: null,
  ollamaReachable: true,
  modelIsRemote: false,
  refresh: async (selectedModel) => {
    getSystemStats()
      .then((stats) =>
        set({
          cpuPercent: stats.cpu_percent,
          memoryPercent: stats.memory_percent,
          diskPercent: stats.disk_percent,
          temperatureC: stats.temperature_c,
        }),
      )
      .catch(() => {});
    listModels()
      .then((models) => {
        set({
          ollamaReachable: true,
          modelIsRemote: models.find((m) => m.name === selectedModel)?.is_remote ?? false,
        });
      })
      .catch(() => set({ ollamaReachable: false }));
  },
}));
