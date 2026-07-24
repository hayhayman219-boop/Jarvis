import { create } from "zustand";
import { speak } from "../lib/apiClient";

// Timers & alarms. A timer counts down a duration; an alarm fires at a wall
// clock time. Both live only in memory (ephemeral by nature) and are checked
// by a single interval; on expiry they beep, speak, and drop off the HUD.

export interface JarvisTimer {
  id: string;
  label: string;
  endsAt: number; // epoch ms
  kind: "timer" | "alarm";
}

interface TimersState {
  timers: JarvisTimer[];
  addTimer: (durationMs: number, label?: string) => JarvisTimer;
  addAlarm: (at: Date, label?: string) => JarvisTimer;
  cancel: (id: string) => void;
  fire: (id: string) => void;
}

let audioCtx: AudioContext | null = null;
function beep() {
  try {
    audioCtx ??= new AudioContext();
    // Three short rising tones — an alert that cuts through.
    const now = audioCtx.currentTime;
    [660, 880, 1046].forEach((freq, i) => {
      const osc = audioCtx!.createOscillator();
      const gain = audioCtx!.createGain();
      osc.frequency.value = freq;
      osc.type = "sine";
      gain.gain.setValueAtTime(0.001, now + i * 0.22);
      gain.gain.exponentialRampToValueAtTime(0.25, now + i * 0.22 + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.001, now + i * 0.22 + 0.2);
      osc.connect(gain);
      gain.connect(audioCtx!.destination);
      osc.start(now + i * 0.22);
      osc.stop(now + i * 0.22 + 0.22);
    });
  } catch {
    /* audio not available */
  }
}

const uid = () => Math.random().toString(36).slice(2, 9);

export const useTimersStore = create<TimersState>((set, get) => ({
  timers: [],
  addTimer: (durationMs, label) => {
    const t: JarvisTimer = {
      id: uid(),
      label: label?.trim() || "Timer",
      endsAt: Date.now() + durationMs,
      kind: "timer",
    };
    set((s) => ({ timers: [...s.timers, t] }));
    return t;
  },
  addAlarm: (at, label) => {
    const t: JarvisTimer = {
      id: uid(),
      label: label?.trim() || "Alarm",
      endsAt: at.getTime(),
      kind: "alarm",
    };
    set((s) => ({ timers: [...s.timers, t] }));
    return t;
  },
  cancel: (id) => set((s) => ({ timers: s.timers.filter((t) => t.id !== id) })),
  fire: (id) => {
    const t = get().timers.find((x) => x.id === id);
    if (!t) return;
    set((s) => ({ timers: s.timers.filter((x) => x.id !== id) }));
    beep();
    const spoken = t.kind === "alarm" ? `Alarm: ${t.label}.` : `Your ${t.label} is done.`;
    void speak(spoken).catch(() => {});
  },
}));

// One global ticker checks for due timers ~4x/second.
let started = false;
export function startTimersTicker() {
  if (started) return;
  started = true;
  setInterval(() => {
    const now = Date.now();
    const due = useTimersStore.getState().timers.filter((t) => t.endsAt <= now);
    for (const t of due) useTimersStore.getState().fire(t.id);
  }, 250);
}
