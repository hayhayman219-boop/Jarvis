// Bridges the Rust backend's window events to the frontend stores on the
// desktop app: streamed chat tokens, TTS speaking state, and wake-word
// interactions from the always-on voice loop (voice_loop.rs).
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./env";
import { cleanTranscript, isMeaningfulSpeech } from "./transcript";
import { useChatStore } from "../state/chatStore";
import { useSettingsStore } from "../state/settingsStore";

let registered = false;

export function registerDesktopEventListeners() {
  if (!isTauri || registered) return;
  registered = true;

  // Tokens arrive dozens of times per second during generation; applying
  // each one individually re-renders the chat per token and contributes to
  // UI lag while the CPU is already busy with the LLM + TTS. Batch them and
  // flush to the store at ~10Hz — visually identical, far cheaper.
  let pendingTokens = "";
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flushTokens = () => {
    flushTimer = null;
    if (pendingTokens) {
      const batch = pendingTokens;
      pendingTokens = "";
      useChatStore.getState().appendStreamToken(batch);
    }
  };
  void listen<{ content: string }>("chat-token", (event) => {
    pendingTokens += event.payload.content;
    if (flushTimer == null) {
      flushTimer = setTimeout(flushTokens, 100);
    }
  });

  void listen<{ speaking: boolean }>("tts-state", (event) => {
    useChatStore.getState().setStatus(event.payload.speaking ? "speaking" : "idle");
  });

  void listen<{ status: "listening" | "idle" }>("voice-status", (event) => {
    useChatStore.getState().setStatus(event.payload.status);
  });

  void listen<{ text: string; ai?: string }>("voice-command", (event) => {
    const model = useSettingsStore.getState().model;
    // Drop Whisper non-speech artifacts (music/ambient noise transcribed as
    // "[Music]" etc.) so they don't get sent as phantom user messages.
    if (model && isMeaningfulSpeech(event.payload.text)) {
      // The wake word chooses the persona: "Hey Hacks …" routes to Hacks,
      // "Jarvis …" to Jarvis. Also make that the active AI for follow-ups.
      const ai = event.payload.ai;
      if (ai) useChatStore.getState().setActiveAi(ai);
      void useChatStore.getState().sendMessage(model, cleanTranscript(event.payload.text), ai);
    }
  });
}
