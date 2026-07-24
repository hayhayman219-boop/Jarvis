// Sentence-by-sentence TTS playback for the web app: tokens from the
// streaming chat are fed in, complete sentences are sent to the server's
// /api/speak endpoint (Piper synthesis), and the resulting clips play
// sequentially so Jarvis starts talking a sentence into the reply instead
// of after the whole generation. The desktop app has an equivalent queue on
// the Rust side (TtsQueue in voice.rs); this one exists because the web
// app's speakers are in the visitor's browser, not on the server.
import { sanitizeForSpeech } from "./sanitizeForSpeech";

// Mirrors MIN_SENTENCE_LEN in src-tauri/src/commands/ollama.rs: don't split
// on a terminator until the sentence has some substance, so abbreviations
// ("Dr.", "e.g.") don't produce choppy one-word clips.
const MIN_SENTENCE_LEN = 24;

let pendingText = "";
let queue: string[] = [];
let pumping = false;
let stopped = false;
let currentAudio: HTMLAudioElement | null = null;
let onSpeakingChange: (speaking: boolean) => void = () => {};

export function configure(callback: (speaking: boolean) => void) {
  onSpeakingChange = callback;
}

function drainCompleteSentences(): string[] {
  const out: string[] = [];
  for (;;) {
    let splitAt = -1;
    for (let i = 0; i < pendingText.length; i++) {
      if (i + 1 < MIN_SENTENCE_LEN) continue;
      const c = pendingText[i];
      if (c === "\n") {
        splitAt = i + 1;
        break;
      }
      if ((c === "." || c === "!" || c === "?") && i + 1 < pendingText.length && /\s/.test(pendingText[i + 1])) {
        splitAt = i + 1;
        break;
      }
    }
    if (splitAt < 0) break;
    const sentence = pendingText.slice(0, splitAt).trim();
    pendingText = pendingText.slice(splitAt);
    if (sentence) out.push(sentence);
  }
  return out;
}

export function feed(token: string) {
  pendingText += token;
  const sentences = drainCompleteSentences();
  if (sentences.length > 0) {
    queue.push(...sentences);
    void pump();
  }
}

/** Flush whatever partial sentence remains once the stream ends. */
export function finish() {
  const tail = pendingText.trim();
  pendingText = "";
  if (tail) {
    queue.push(tail);
    void pump();
  }
}

export function isActive(): boolean {
  return pumping || queue.length > 0;
}

export function stop() {
  stopped = true;
  queue = [];
  pendingText = "";
  currentAudio?.pause();
  currentAudio = null;
}

async function pump() {
  if (pumping) return;
  pumping = true;
  stopped = false;
  onSpeakingChange(true);

  while (queue.length > 0 && !stopped) {
    const sentence = sanitizeForSpeech(queue.shift()!);
    if (!sentence) continue;
    try {
      const res = await fetch("/api/speak", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: sentence }),
      });
      if (!res.ok) continue;
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      await new Promise<void>((resolve) => {
        const audio = new Audio(url);
        currentAudio = audio;
        audio.onended = () => {
          URL.revokeObjectURL(url);
          resolve();
        };
        audio.onerror = () => resolve();
        audio.onpause = () => resolve(); // stop() pauses mid-clip
        audio.play().catch(() => resolve());
      });
    } catch {
      // Skip the sentence on synthesis/network failure; keep the queue moving.
    }
  }

  pumping = false;
  currentAudio = null;
  onSpeakingChange(false);
}
