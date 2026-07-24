import { create } from "zustand";
import { setVoiceLoopPaused, stopSpeaking, transcribe } from "../lib/apiClient";
import { startCapture, stopCapture } from "../lib/micCapture";
import { isTauri } from "../lib/env";
import * as webTts from "../lib/webTtsQueue";
import { cleanTranscript, isMeaningfulSpeech } from "../lib/transcript";
import { useChatStore } from "./chatStore";
import { useSettingsStore } from "./settingsStore";

const WAKE_WINDOW_MS = 2500;
const COMMAND_WINDOW_MS = 5000;
const STOP_WINDOW_MS = 2000;
const LOOP_GAP_MS = 300;
// If less than this much text follows "jarvis" in the wake-word clip, treat
// it as the user having said only the wake word (pausing before their
// actual command) rather than a command spoken in the same breath.
const MIN_INLINE_COMMAND_CHARS = 3;

// A generation counter, not a simple running/stop-requested boolean pair,
// because React StrictMode double-invokes effects in dev (mount -> cleanup
// -> mount again) faster than an in-flight async tick can observe a stop
// request. With a boolean guard, the second mount's start() would see the
// first loop's "running" flag still true and no-op, while the first loop
// then sees the stop request and exits — leaving nothing running. Each
// start() bumps the generation and captures it; a loop keeps going only
// while its captured generation is still the current one.
let voiceLoopGeneration = 0;

interface VoiceStoreState {
  isRecording: boolean;
  micBusy: boolean;
  error: string | null;
}

// `micBusy` is the single source of truth for "something currently owns the
// microphone" — both manual push-to-talk and the (web-only) background
// wake-word loop check/set it, so they never both capture at once.
export const useVoiceStore = create<VoiceStoreState>(() => ({
  isRecording: false,
  micBusy: false,
  error: null,
}));

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function recordWindow(ms: number, fast = false): Promise<string> {
  await startCapture();
  await sleep(ms);
  const audio = await stopCapture();
  const text = await transcribe(audio, fast);
  return text.trim();
}

export async function manualStartListening() {
  if (useVoiceStore.getState().micBusy) return;
  useVoiceStore.setState({ micBusy: true, isRecording: true, error: null });
  try {
    // Keep the backend wake-word loop from also reacting to this speech.
    await setVoiceLoopPaused(true);
    await startCapture();
  } catch (err) {
    useVoiceStore.setState({ error: String(err), micBusy: false, isRecording: false });
    await setVoiceLoopPaused(false);
  }
}

export async function manualStopAndTranscribe(): Promise<string | null> {
  if (!useVoiceStore.getState().isRecording) return null;
  useVoiceStore.setState({ isRecording: false });
  useChatStore.getState().setStatus("thinking");
  try {
    const audio = await stopCapture();
    const text = (await transcribe(audio)).trim();
    return text || null;
  } catch (err) {
    useVoiceStore.setState({ error: String(err) });
    return null;
  } finally {
    useChatStore.getState().setStatus("idle");
    useVoiceStore.setState({ micBusy: false });
    await setVoiceLoopPaused(false);
  }
}

export async function interruptSpeaking() {
  if (isTauri) {
    await stopSpeaking();
  } else {
    webTts.stop();
  }
  useChatStore.getState().setStatus("idle");
}

export function startBackgroundVoiceLoop() {
  // Desktop wake-word listening lives in the Rust backend (voice_loop.rs)
  // with a persistent mic stream; this JS loop only serves the web app,
  // where the microphone belongs to the visiting browser.
  if (isTauri) return;
  voiceLoopGeneration += 1;
  void backgroundLoopTick(voiceLoopGeneration);
}

export function stopBackgroundVoiceLoop() {
  voiceLoopGeneration += 1;
}

async function backgroundLoopTick(myGeneration: number) {
  while (voiceLoopGeneration === myGeneration) {
    const { micBusy } = useVoiceStore.getState();
    const { wakeWordEnabled, model } = useSettingsStore.getState();
    const status = useChatStore.getState().status;

    if (micBusy || !wakeWordEnabled || !model) {
      await sleep(LOOP_GAP_MS);
      continue;
    }

    if (status === "speaking") {
      useVoiceStore.setState({ micBusy: true });
      try {
        const heard = await recordWindow(STOP_WINDOW_MS, true);
        if (/\bstop\b/i.test(heard)) {
          await interruptSpeaking();
        }
      } catch (err) {
        // Transient mic errors during background listening shouldn't surface
        // as user-facing errors — just log, skip this tick, and try again.
        console.error("[wake-word] stop-word listen failed", err);
      } finally {
        useVoiceStore.setState({ micBusy: false });
        await sleep(LOOP_GAP_MS);
      }
    } else if (status === "idle") {
      useVoiceStore.setState({ micBusy: true });
      try {
        await startCapture();
        await sleep(WAKE_WINDOW_MS);
        const wakeAudio = await stopCapture();
        const fastHeard = (await transcribe(wakeAudio, true)).trim();

        if (/\bjarvis\b/i.test(fastHeard)) {
          useChatStore.getState().setStatus("listening");

          // Re-transcribe the same clip with the accurate model rather than
          // reusing the fast/rough one — this is what becomes the command
          // if the user said it in the same breath as "Jarvis".
          const accurateHeard = (await transcribe(wakeAudio, false)).trim();
          const inlineCommand = accurateHeard
            .replace(/^.*?\bjarvis\b[,:]?\s*/i, "")
            .trim();

          const command =
            inlineCommand.length >= MIN_INLINE_COMMAND_CHARS
              ? inlineCommand
              : // User paused after the wake word — give them a fresh window
                // to say the actual command instead of missing it.
                await recordWindow(COMMAND_WINDOW_MS);

          useChatStore.getState().setStatus("idle");
          if (command && isMeaningfulSpeech(command)) {
            useChatStore.getState().sendMessage(model, cleanTranscript(command));
          }
        }
      } catch (err) {
        // Same as above: log, ignore, and retry next tick.
        console.error("[wake-word] wake-listen failed", err);
      } finally {
        useVoiceStore.setState({ micBusy: false });
        await sleep(LOOP_GAP_MS);
      }
    } else {
      await sleep(LOOP_GAP_MS);
    }
  }
}
