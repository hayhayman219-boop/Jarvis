import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage, NewsCategory, ReactorStatus } from "../lib/types";
import { addToCart, chatStreamWeb, closeApp, exportPdf, getNews, getWeather, homeCommand, listFiles, listSubscriptions, lockScreen, openApp, openNews, openUrls, openWeb, readScreen, runCommand, setBrightness, setVolume, speak, stopSpeaking, takeScreenshot } from "../lib/apiClient";
import { buildBriefing } from "../lib/briefing";
import { findScene, sceneUrl } from "../data/ironManScenes";
import { useTimersStore } from "./timersStore";
import { useComicSearchStore } from "./comicSearchStore";
import { isTauri } from "../lib/env";
import * as webTts from "../lib/webTtsQueue";
import { getSetting, setSetting } from "../lib/persistedStore";
import { useSettingsStore } from "./settingsStore";
import { useScreensStore } from "./screensStore";
import { findScreenByPhrase } from "../screens/registry";
import { MAIN_AI, ALL_AIS, getAi, findAiByName } from "../data/subAIs";

const CHAT_HISTORY_STORE = "chat-history.json";
const ACTIVE_AI_KEY = "activeAiId";

function persistMessages(messages: ChatMessage[]) {
  void setSetting("messages", messages, CHAT_HISTORY_STORE);
}

// Per-AI generation counter, bumped whenever that AI's conversation is
// cleared (or a new send to it starts). An in-flight reply captures the value
// at send time and only commits if it still matches — so clearing mid-reply
// can't repopulate the chat with the stale pre-clear history it was built from.
const sendGeneration: Record<string, number> = {};
const bumpGeneration = (aiId: string): number =>
  (sendGeneration[aiId] = (sendGeneration[aiId] ?? 0) + 1);

// Sums all duration tokens in a phrase ("1 hour 30 min" -> ms). 0 if none.
function parseDurationMs(text: string): number {
  const re = /(\d+(?:\.\d+)?)\s*(hours?|hrs?|h|minutes?|mins?|m|seconds?|secs?|s)\b/gi;
  let ms = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const n = parseFloat(m[1]);
    const unit = m[2][0].toLowerCase();
    ms += unit === "h" ? n * 3_600_000 : unit === "m" ? n * 60_000 : n * 1000;
  }
  return ms;
}

// Parses a wall-clock time ("7:30am", "6 pm", "18:00") into the next such
// moment in the future. Null if no time is found.
function parseAlarmTime(text: string): Date | null {
  const m = text.match(/\b(\d{1,2})(?::(\d{2}))?\s*(a\.?m\.?|p\.?m\.?)?/i);
  if (!m) return null;
  let hour = parseInt(m[1], 10);
  const min = m[2] ? parseInt(m[2], 10) : 0;
  const ap = m[3]?.toLowerCase().replace(/\./g, "");
  if (ap === "pm" && hour < 12) hour += 12;
  if (ap === "am" && hour === 12) hour = 0;
  if (hour > 23 || min > 59) return null;
  const d = new Date();
  d.setHours(hour, min, 0, 0);
  if (d.getTime() <= Date.now()) d.setDate(d.getDate() + 1);
  return d;
}

function humanDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const bits: string[] = [];
  if (h) bits.push(`${h} hour${h === 1 ? "" : "s"}`);
  if (m) bits.push(`${m} minute${m === 1 ? "" : "s"}`);
  if (s) bits.push(`${s} second${s === 1 ? "" : "s"}`);
  return bits.join(" and ") || "0 seconds";
}

// Returns a copy of `messages` with the trailing assistant bubble of `aiId`
// (the streaming placeholder) set to `content`.
function setLastAssistant(
  messages: StoredMessage[],
  aiId: string,
  content: string,
): StoredMessage[] {
  const out = messages.slice();
  for (let i = out.length - 1; i >= 0; i--) {
    if (out[i].aiId === aiId && out[i].role === "assistant") {
      out[i] = { ...out[i], content };
      break;
    }
  }
  return out;
}

/// Deterministic news-intent detection: when the message plainly asks for
/// news/leaks in a known category, we fetch headlines and stuff them into
/// the context rather than relying on LLM tool-calling (which small local
/// models handle unreliably, and which would add a whole extra generation
/// round-trip). Kept intentionally strict — a false positive costs a feed
/// fetch and prompt space on an unrelated question.
function detectNewsCategories(message: string): NewsCategory[] {
  const lower = message.toLowerCase();
  const wantsNews = /\b(news|leaks?|headlines?)\b|what'?s (new|happening|going on)/.test(lower);
  if (!wantsNews) return [];

  const categories: NewsCategory[] = [];
  if (/pok[eé]|tcg/.test(lower)) categories.push("pokemon");
  if (/\bai\b|\ba\.i\.?\b|artificial intelligence|\bllms?\b|openai|anthropic|chatgpt|claude|gemini|ollama/.test(lower))
    categories.push("ai");
  if (/data ?cent(er|re)/.test(lower)) categories.push("datacenter");
  if (/\btech(nology)?\b/.test(lower)) categories.push("tech");
  // Bare "any news?" with no category: default to general tech.
  if (categories.length === 0) categories.push("tech");
  return categories.slice(0, 2);
}

function ageOf(published: string | null, now: Date): string {
  if (!published) return "";
  const ms = now.getTime() - new Date(published).getTime();
  const hours = Math.floor(ms / 3_600_000);
  if (hours < 1) return " (<1h ago)";
  if (hours < 48) return ` (${hours}h ago)`;
  return ` (${Math.floor(hours / 24)}d ago)`;
}

/// The local model has no live data of its own, so each request carries a
/// small context message with the current time, configured location,
/// (when a location is set) fresh Open-Meteo conditions, and — when the
/// message asks about news/leaks — just-fetched RSS headlines. Otherwise
/// "Jarvis, how's the weather?" / "any Pokémon leaks?" gets the stock
/// "I don't have real-time access" apology.
async function buildLiveContext(userMessage: string): Promise<ChatMessage> {
  const { location, temperatureUnit } = useSettingsStore.getState();
  const now = new Date();
  let context = `LIVE DATA (fetched moments ago — this overrides your training knowledge, which is months out of date; use it naturally, never disclaim real-time access): current date/time is ${now.toLocaleString(undefined, { weekday: "long", year: "numeric", month: "long", day: "numeric", hour: "numeric", minute: "2-digit" })}.`;
  if (location) {
    context += ` The user is in ${location.name}.`;
    try {
      const weather = await getWeather(
        location.latitude,
        location.longitude,
        temperatureUnit === "fahrenheit",
      );
      context += ` Current weather there: ${weather.condition}, ${weather.temperature.toFixed(0)}${weather.temperature_unit}, wind ${weather.windspeed.toFixed(0)} ${weather.windspeed_unit}.`;
    } catch {
      // Weather fetch failing shouldn't block the chat.
    }
  } else {
    context += " No location is configured (the user can set one in Settings).";
  }

  for (const category of detectNewsCategories(userMessage)) {
    try {
      const items = await getNews(category);
      const now = new Date();
      const lines = items
        .slice(0, 8)
        .map((item) => `- [${item.source}] ${item.title}${ageOf(item.published, now)}`)
        .join("\n");
      context += `\n\nFresh ${category} headlines, fetched just now — answer news questions from these, conversationally summarizing the few most relevant:\n${lines}`;
    } catch {
      context += `\n\n(Tried to fetch ${category} headlines but the feeds were unreachable — say so if asked.)`;
    }
  }

  // Device control: when the message sounds like "turn on/off X" / "toggle
  // X", the command is executed against Home Assistant BEFORE the model
  // replies, and the outcome is injected so the reply is a truthful
  // confirmation rather than a hallucinated one.
  if (/\b(turn|switch|power)\s+(on|off)\b|\btoggle\b/i.test(userMessage)) {
    try {
      const outcome = await homeCommand(userMessage);
      if (outcome) {
        context += `\n\nHOME CONTROL (already performed just now — confirm it naturally, don't pretend to do it again): ${outcome}`;
      }
    } catch (err) {
      context += `\n\nHOME CONTROL FAILED: ${err}. Tell the user plainly.`;
    }
  }

  // Computer control (Phase 3, desktop-only — see apiClient.ts): open/close
  // apps, run shell commands, list files. Anchored at the start of the
  // message (unlike the mid-sentence home-control match above) because
  // "open"/"close"/"run" are common English words far beyond device
  // control; a false match here just fails harmlessly (e.g. "open the pod
  // bay doors" -> ENOENT), except run-command, which is additionally gated
  // behind the literal word "command" so an offhand "let's run the numbers"
  // can't trigger arbitrary shell execution.
  // "Open the news" launches Chrome to news sites (distinct from *asking about*
  // the news, handled by the RSS block above). Checked before the generic
  // open-app matcher so "open the news" doesn't try to launch an app literally
  // named "the news".
  const wantsOpenNews = /\b(open|show|pull up|bring up|launch|go to)\b.{0,25}\bnews\b/i.test(userMessage);
  if (wantsOpenNews) {
    try {
      const category = detectNewsCategories(userMessage)[0];
      const outcome = await openNews(category);
      context += `\n\nNEWS (already opened in Chrome just now — confirm naturally, don't pretend to do it again): ${outcome}`;
    } catch (err) {
      context += `\n\nFailed to open the news: ${err}. Tell the user plainly.`;
    }
  }

  // Fuller Chrome control: search the web, go to a site, or open a new tab.
  const searchMatch = userMessage.match(
    /^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:search(?:\s+for)?|google|look up)\s+(.+?)[.!?]*$/i,
  );
  const gotoMatch = userMessage.match(
    /^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:go to|navigate to|visit|take me to)\s+(.+?)[.!?]*$/i,
  );
  const newTabMatch = /\bnew (?:browser )?tab\b/i.test(userMessage);
  // Which "open X" targets should go to the browser instead of an app.
  const WEBSITE_ROUTE =
    /\b(youtube|yt|gmail|github|reddit|wikipedia|chatgpt|facebook|instagram|insta|linkedin|ebay|walmart|amazon|netflix|(?:google\s+)?(?:maps|drive|docs))\b|\.(com|org|net|io|dev|co|tv|app|ai)\b/i;
  let webHandled = false;
  if (!wantsOpenNews && (searchMatch || gotoMatch || newTabMatch)) {
    try {
      const outcome = searchMatch
        ? await openWeb(searchMatch[1].trim(), true)
        : gotoMatch
          ? await openWeb(gotoMatch[1].trim(), false)
          : await openWeb("google.com", false);
      context += `\n\nBROWSER (already done in Chrome — confirm naturally, don't pretend): ${outcome}`;
      webHandled = true;
    } catch (err) {
      context += `\n\nBROWSER FAILED: ${err}. Tell the user plainly.`;
      webHandled = true;
    }
  }

  // Cart: "add <item> to (my) (<retailer>) cart" — opens the retailer in Chrome
  // (Amazon adds directly via its cart-add link; others open a search).
  const cartMatch = userMessage.match(
    /\badd\s+(.+?)\s+to\s+(?:my\s+|the\s+)?(amazon|walmart|newegg|micro\s?center|apple|best\s?buy|target)?\s*cart\b/i,
  );
  if (cartMatch) {
    try {
      const item = cartMatch[1].trim();
      const retailer = (cartMatch[2] || "amazon").replace(/\s+/g, "").toLowerCase();
      const outcome = await addToCart(retailer, item);
      context += `\n\nCART (already done in Chrome — confirm naturally, don't pretend): ${outcome}`;
    } catch (err) {
      context += `\n\nCART FAILED: ${err}. Tell the user plainly.`;
    }
  }

  const openAppMatch = userMessage.match(/^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:open|launch)\s+(.+?)[.!?]*$/i);
  const closeAppMatch = userMessage.match(/^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:close|quit)\s+(.+?)[.!?]*$/i);
  const runCommandMatch = userMessage.match(
    /^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:run|execute)\s+(?:the\s+)?command[:\s]+(.+?)[.!?]*$/i,
  );
  const listFilesMatch = userMessage.match(
    /^\s*(?:hey\s+)?(?:jarvis[,:]?\s+)?(?:list|show)\s+(?:the\s+)?files\s+(?:in|inside|from)\s+(.+?)[.!?]*$/i,
  );

  if (!wantsOpenNews && !webHandled && (openAppMatch || closeAppMatch || runCommandMatch || listFilesMatch)) {
    try {
      let outcome: string;
      if (openAppMatch && WEBSITE_ROUTE.test(openAppMatch[1])) {
        // "open youtube" / "open github.com" → browser, not a local app.
        outcome = await openWeb(openAppMatch[1].trim(), false);
      } else if (openAppMatch) {
        outcome = await openApp(openAppMatch[1].trim());
      } else if (closeAppMatch) {
        outcome = await closeApp(closeAppMatch[1].trim());
      } else if (runCommandMatch) {
        outcome = await runCommand(runCommandMatch[1].trim());
      } else {
        const entries = await listFiles(listFilesMatch![1].trim());
        const names = entries.slice(0, 40).map((e) => (e.is_dir ? `${e.name}/` : e.name));
        outcome = names.length > 0 ? names.join(", ") : "(empty directory)";
      }
      context += `\n\nCOMPUTER CONTROL (already performed just now — confirm naturally, don't pretend to do it again): ${outcome}`;
    } catch (err) {
      context += `\n\nCOMPUTER CONTROL FAILED: ${err}. Tell the user plainly.`;
    }
  }

  // Desktop control (computer control++): volume, brightness, screen lock,
  // screenshot. Only considered for short, imperative messages — these fire
  // real side effects, so gating on length keeps conversational mentions
  // ("I need to mute my thoughts") from tripping them.
  const lc = userMessage.toLowerCase();
  const wordCount = userMessage.trim().split(/\s+/).length;
  if (wordCount <= 9) {
    let desktopOutcome: string | null = null;
    try {
      const volumeSet = lc.match(/\bvolume\b[^0-9]*(\d{1,3})/);
      const brightnessSet = lc.match(/\bbright(?:ness)?\b[^0-9]*(\d{1,3})|\b(\d{1,3})\s*%?\s*bright/);
      if (/\bunmute\b/.test(lc)) desktopOutcome = await setVolume("unmute");
      else if (/\bmute\b/.test(lc)) desktopOutcome = await setVolume("mute");
      else if (volumeSet) desktopOutcome = await setVolume(volumeSet[1]);
      else if (/(volume up|louder|turn (it |the volume )?up|raise .*volume|increase .*volume)/.test(lc))
        desktopOutcome = await setVolume("up");
      else if (/(volume down|quieter|turn (it |the volume )?down|lower .*volume|decrease .*volume)/.test(lc))
        desktopOutcome = await setVolume("down");
      else if (brightnessSet) desktopOutcome = await setBrightness(brightnessSet[1] ?? brightnessSet[2]);
      else if (/(brightness up|brighten|raise .*bright|increase .*bright)/.test(lc))
        desktopOutcome = await setBrightness("up");
      else if (/(brightness down|dim (the )?(screen|display|monitor|it)|lower .*bright|decrease .*bright)/.test(lc))
        desktopOutcome = await setBrightness("down");
      else if (/\block\b.*(screen|computer|session|desktop)|lock (it|up)\b/.test(lc))
        desktopOutcome = await lockScreen();
      else if (/(take|grab|capture|get).{0,12}screenshot|^screenshot\b|screenshot (of|the)/.test(lc))
        desktopOutcome = await takeScreenshot();
      if (desktopOutcome) {
        context += `\n\nDESKTOP CONTROL (already performed just now — confirm naturally, don't pretend to do it again): ${desktopOutcome}`;
      }
    } catch (err) {
      context += `\n\nDESKTOP CONTROL FAILED: ${err}. Tell the user plainly.`;
    }
  }

  return { role: "system", content: context };
}

/** A chat message tagged with the AI persona it belongs to, so each panel
 * (Jarvis, Hacks, …) can show only its own conversation. */
export type StoredMessage = ChatMessage & { aiId: string };

interface ChatState {
  messages: StoredMessage[];
  status: ReactorStatus;
  /** True while a reply is streaming into the trailing assistant message. */
  streaming: boolean;
  /** Which persona is currently generating/speaking, so streamed tokens and
   * the Stop button route to the right panel. Null when nothing is streaming. */
  streamingAiId: string | null;
  /** Which AI persona is active for voice (Main "jarvis" or a Sub AI). */
  activeAiId: string;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setStatus: (status: ReactorStatus) => void;
  setActiveAi: (id: string) => void;
  appendStreamToken: (token: string) => void;
  /** Send a message to a specific AI (defaults to the voice-active one). */
  sendMessage: (model: string, content: string, aiId?: string) => Promise<void>;
  /** Clear one AI's conversation, or all of them when no id is given. */
  clearHistory: (aiId?: string) => void;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  status: "idle",
  streaming: false,
  streamingAiId: null,
  activeAiId: MAIN_AI.id,
  hydrated: false,
  hydrate: async () => {
    const [messages, activeAiId] = await Promise.all([
      getSetting<StoredMessage[]>("messages", CHAT_HISTORY_STORE),
      getSetting<string>(ACTIVE_AI_KEY),
    ]);
    // Migrate pre-per-AI history (untagged) into Jarvis's conversation.
    const tagged = (messages ?? []).map((m) => ({ ...m, aiId: m.aiId ?? MAIN_AI.id }));
    set({ messages: tagged, activeAiId: activeAiId ?? MAIN_AI.id, hydrated: true });
  },
  setActiveAi: (id) => {
    set({ activeAiId: id });
    void setSetting(ACTIVE_AI_KEY, id);
  },
  setStatus: (status) => set({ status }),
  appendStreamToken: (token) =>
    set((s) => {
      if (!s.streaming || s.streamingAiId === null) return s;
      // Append to the trailing assistant bubble of the AI that's streaming, so
      // tokens land in the correct panel even with two panels open.
      const messages = s.messages.slice();
      for (let i = messages.length - 1; i >= 0; i--) {
        const m = messages[i];
        if (m.aiId !== s.streamingAiId) continue;
        if (m.role !== "assistant") break;
        messages[i] = { ...m, content: m.content + token };
        return { ...s, messages };
      }
      return s;
    }),
  sendMessage: async (model, content, targetAiId) => {
    const { voiceRepliesEnabled } = useSettingsStore.getState();
    // Sub AI switching by voice — "talk to Hacks", "switch to Jarvis", or
    // addressing one by name ("Hacks, tell me…"). Only when no panel targeted
    // this send explicitly (a typed message in a panel routes to that panel).
    if (targetAiId === undefined) {
      const names = ALL_AIS.map((a) => a.name).join("|");
      const m =
        content.match(
          new RegExp(`\\b(?:switch to|talk to|bring in|summon|activate|go to|back to)\\s+(${names})\\b`, "i"),
        ) ?? content.match(new RegExp(`^\\s*(?:hey\\s+)?(${names})\\b[,:]?`, "i"));
      if (m) {
        const ai = findAiByName(m[1]);
        if (ai && ai.id !== get().activeAiId) get().setActiveAi(ai.id);
      }
    }
    // The AI this message belongs to: the explicit panel target, else the
    // voice-active persona (possibly just switched above).
    const aiId = targetAiId ?? get().activeAiId;
    const persona = getAi(aiId);

    // Daily Briefing — "good morning" / "brief me" → a spoken rundown of
    // weather, today's calendar, reminders, and headlines. Deterministic.
    if (
      /\bbriefing\b/i.test(content) ||
      /\bbrief me\b/i.test(content) ||
      /^(?:\s*(?:hey\s+)?jarvis[\s,!.]*)?good (?:morning|afternoon|evening)\b/i.test(content) ||
      /\bwhat'?s (?:on )?my day\b/i.test(content)
    ) {
      const userMsg: StoredMessage = { role: "user", content, aiId };
      set((s) => ({
        messages: [...s.messages, userMsg, { role: "assistant", content: "Compiling your briefing…", aiId }],
      }));
      try {
        const text = await buildBriefing();
        set((s) => ({ messages: setLastAssistant(s.messages, aiId, text) }));
        void speak(text).catch(() => {});
      } catch (e) {
        set((s) => ({ messages: setLastAssistant(s.messages, aiId, `I couldn't compile the briefing: ${e}`) }));
      }
      persistMessages(get().messages);
      return;
    }

    // Timers & Alarms — "set a timer for 10 minutes" / "set an alarm for 7:30".
    if (
      (/\b(timer|alarm)\b/i.test(content) && /\b(set|start|create|make|for|put)\b/i.test(content)) ||
      /\bwake me\b/i.test(content)
    ) {
      const userMsg: StoredMessage = { role: "user", content, aiId };
      const isAlarm = /\b(alarm|wake me)\b/i.test(content);
      let confirm = "";
      if (isAlarm) {
        const when = parseAlarmTime(content);
        if (when) {
          useTimersStore.getState().addAlarm(when);
          confirm = `Alarm set for ${when.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}.`;
        }
      } else {
        const ms = parseDurationMs(content);
        if (ms > 0) {
          useTimersStore.getState().addTimer(ms);
          confirm = `Timer set for ${humanDuration(ms)}.`;
        }
      }
      if (!confirm) {
        confirm = "I couldn't work out the time — try 'set a timer for 10 minutes' or 'set an alarm for 7:30am'.";
      }
      set((s) => ({ messages: [...s.messages, userMsg, { role: "assistant", content: confirm, aiId }] }));
      persistMessages(get().messages);
      void speak(confirm).catch(() => {});
      return;
    }

    // Comic lookup (Hacks) — "look up / show me the cover of <comic>" fetches
    // real Comic Vine data + cover art into the Comic Covers pop-up screen.
    if (
      /\b(cover|comic|issue)s?\b/i.test(content) &&
      /\b(look up|show me|find|pull up|search|get)\b/i.test(content)
    ) {
      const q = content
        .replace(/.*\b(?:covers?\s+(?:of|for)|look up|search(?:\s+for)?|find|pull up|show me|get)\b/i, "")
        .replace(/\b(the|a|comic|comics|cover|covers|issue|please|for me|on comic ?vine)\b/gi, "")
        .replace(/[?.!]/g, "")
        .trim();
      if (q.length >= 2) {
        const userMsg: StoredMessage = { role: "user", content, aiId };
        useScreensStore.getState().openScreen("comic-covers");
        void useComicSearchStore.getState().search(q);
        set((s) => ({
          messages: [...s.messages, userMsg, { role: "assistant", content: `Pulling up covers for **${q}**.`, aiId }],
        }));
        persistMessages(get().messages);
        return;
      }
    }

    // Scene & clip launcher (Hacks) — "play/show/watch the Mark I escape" opens
    // that Iron Man scene in Chrome. Only fires on a known scene alias, so it
    // can't hijack ordinary "show me…" requests.
    if (/\b(play|show|watch|pull up|put on|open)\b/i.test(content)) {
      const scene = findScene(content);
      if (scene) {
        const userMsg: StoredMessage = { role: "user", content, aiId };
        void openUrls([sceneUrl(scene)]).catch(() => {});
        const reply = `Pulling up **${scene.title}** in Chrome.`;
        set((s) => ({ messages: [...s.messages, userMsg, { role: "assistant", content: reply, aiId }] }));
        persistMessages(get().messages);
        return;
      }
    }

    // "Save/make that into a PDF" — export THIS AI's last reply to Downloads.
    // Fully deterministic (no LLM call), so it works even if Ollama is busy.
    if (/\bpdf\b/i.test(content) && /\b(save|export|make|turn|convert|create|download|put|add|generate)\b/i.test(content)) {
      const userMsg: StoredMessage = { role: "user", content, aiId };
      const lastReply = [...get().messages]
        .reverse()
        .find((m) => m.aiId === aiId && m.role === "assistant" && m.content.trim().length > 0);
      if (!lastReply) {
        set((s) => ({
          messages: [
            ...s.messages,
            userMsg,
            { role: "assistant", content: "There's nothing to export yet — ask me something first.", aiId },
          ],
        }));
        persistMessages(get().messages);
        return;
      }
      set((s) => ({ messages: [...s.messages, userMsg] }));
      // Title from the reply's first heading, else the persona name.
      const heading = lastReply.content.match(/^#{1,3}\s+(.+)$/m);
      const title = (heading ? heading[1] : `${persona.name} notes`).trim();
      try {
        const path = await exportPdf(title, lastReply.content);
        set((s) => ({
          messages: [...s.messages, { role: "assistant", content: `Done — saved that as a PDF to ${path}`, aiId }],
        }));
      } catch (e) {
        set((s) => ({
          messages: [...s.messages, { role: "assistant", content: `I couldn't create the PDF: ${e}`, aiId }],
        }));
      }
      persistMessages(get().messages);
      return;
    }
    // Open/close a pop-up screen on request (deterministic — a window command
    // shouldn't depend on the LLM). Matches any registered screen by alias,
    // gated on an open/close verb so ordinary mentions don't trigger it. The
    // reply still flows normally.
    let screenNote = "";
    const screen = findScreenByPhrase(content);
    if (screen) {
      const lc = content.toLowerCase();
      if (/\b(close|hide|dismiss|get rid of|turn off|stop)\b/.test(lc)) {
        useScreensStore.getState().closeScreen(screen.id);
        screenNote = `\n\nSCREEN CONTROL: the ${screen.title} screen was just closed for the user — confirm naturally.`;
      } else if (
        /\b(open|show|pull up|bring up|display|launch|access|use|activate|start|view|look|see|watch|check|get on)\b/.test(lc)
      ) {
        useScreensStore.getState().openScreen(screen.id);
        screenNote = `\n\nSCREEN CONTROL (already done): the ${screen.title} screen is now open and live on the user's display — you CAN do this and just did. Confirm it naturally; never say you lack camera/screen access.`;
      }
    }
    // Screen OCR — "read my screen" / "what does this say" → capture + OCR the
    // display and hand the text to the model as context so it can answer.
    let ocrNote = "";
    if (
      (/\bscreen\b/i.test(content) &&
        /\b(read|see|look|analy[sz]e|what'?s on|check|describe|tell me about)\b/i.test(content)) ||
      /what does (this|it|that|the screen) say/i.test(content) ||
      /\b(ocr|read this to me)\b/i.test(content)
    ) {
      try {
        const text = await readScreen();
        ocrNote = `\n\nSCREEN CONTENTS — OCR of what is currently on the user's screen. Use it to answer their request:\n"""\n${text.slice(0, 4000)}\n"""`;
      } catch (e) {
        ocrNote = `\n\n(Screen OCR failed: ${e}. Tell the user you couldn't read the screen and why.)`;
      }
    }

    // Subscriptions — pull the user's Notion "Monthly Budget" so Jarvis can
    // answer "what are my subscriptions / how much do I spend" naturally.
    let subsNote = "";
    if (
      /\bsubscriptions?\b/i.test(content) ||
      /\bmonthly budget\b/i.test(content) ||
      /what am i (?:paying|subscribed)/i.test(content)
    ) {
      try {
        const subs = await listSubscriptions();
        if (subs.length) {
          const lines = subs
            .map((s) => `- ${s.name}: ${s.amount != null ? "$" + s.amount.toFixed(2) : "n/a"}`)
            .join("\n");
          const total = subs.reduce((a, s) => a + (s.amount ?? 0), 0);
          subsNote = `\n\nUSER'S SUBSCRIPTIONS (from their Notion "Monthly Budget"):\n${lines}\nTotal: $${total.toFixed(2)}/month. Use this to answer; don't invent items.`;
        } else {
          subsNote = `\n\n(No subscriptions found in the user's Notion budget.)`;
        }
      } catch (e) {
        subsNote = `\n\n(Couldn't read subscriptions from Notion: ${e}.)`;
      }
    }

    const userMessage: StoredMessage = { role: "user", content, aiId };
    // Only THIS AI's own conversation is its context/history.
    const ownHistory = [...get().messages.filter((m) => m.aiId === aiId), userMessage];
    // Tag this reply so a mid-flight Clear (of this AI) can void its completion.
    const myGeneration = bumpGeneration(aiId);
    const liveContext = await buildLiveContext(content);
    if (screenNote) liveContext.content += screenNote;
    if (ocrNote) liveContext.content += ocrNote;
    if (subsNote) liveContext.content += subsNote;
    // Cap the history sent to the model (full history stays in the UI and
    // on disk). Persisted chats grow without bound, and an over-long prompt
    // gets truncated from the top — which was silently discarding the
    // system prompt and live context, so the model answered news questions
    // from stale training data. Live context goes as late as possible
    // (freshest, most-authoritative, last to be truncated) but BEFORE the
    // final user message: verified empirically that llama3.2's chat
    // template generates an empty reply when the conversation ends on a
    // system message. Strip the aiId tag — the model only wants role/content.
    const recentHistory: ChatMessage[] = ownHistory
      .slice(-12)
      .map((m) => ({ role: m.role, content: m.content }));
    const systemPrompt: ChatMessage = { role: "system", content: persona.systemPrompt };
    const outgoing = [
      systemPrompt,
      ...recentHistory.slice(0, -1),
      liveContext,
      recentHistory[recentHistory.length - 1],
    ];

    // Append this AI's user message + an empty assistant placeholder that
    // stream tokens fill in, leaving every OTHER AI's messages untouched.
    set((s) => ({
      messages: [...s.messages, userMessage, { role: "assistant", content: "", aiId }],
      status: "thinking",
      streaming: true,
      streamingAiId: aiId,
    }));

    try {
      let full: string;
      if (isTauri) {
        // Tokens arrive via "chat-token" window events (see desktopEvents),
        // and spoken sentences are queued backend-side when speak is on.
        full = await invoke<string>("chat_stream", {
          model,
          messages: outgoing,
          speak: voiceRepliesEnabled,
          // Per-persona voice: Hacks speaks in Ryan, Jarvis in the default.
          voice: persona.voice ?? null,
        });
      } else {
        full = await chatStreamWeb(model, outgoing, (token) => {
          get().appendStreamToken(token);
          if (voiceRepliesEnabled) webTts.feed(token);
        });
        if (voiceRepliesEnabled) webTts.finish();
      }

      // This AI's chat was cleared/superseded while streaming — discard.
      if (myGeneration !== sendGeneration[aiId]) return;
      // When speech is active, the speaking->idle transition is driven by
      // TTS events (desktop) or the web queue callback; only force idle here
      // when nothing is being spoken.
      const nothingSpeaking = isTauri ? !voiceRepliesEnabled : !voiceRepliesEnabled || !webTts.isActive();
      const finalMessages = setLastAssistant(get().messages, aiId, full);
      set((s) => ({
        messages: finalMessages,
        streaming: false,
        streamingAiId: s.streamingAiId === aiId ? null : s.streamingAiId,
        status: nothingSpeaking ? "idle" : s.status,
      }));
      persistMessages(finalMessages);
    } catch (err) {
      if (myGeneration !== sendGeneration[aiId]) return;
      const finalMessages = setLastAssistant(
        get().messages,
        aiId,
        `[Error reaching Jarvis's local mind: ${err}]`,
      );
      set((s) => ({
        messages: finalMessages,
        status: "idle",
        streaming: false,
        streamingAiId: s.streamingAiId === aiId ? null : s.streamingAiId,
      }));
      persistMessages(finalMessages);
    }
  },
  clearHistory: (aiId) => {
    // Void any in-flight reply so its completion can't repopulate the chat,
    // and silence speech if the cleared AI was the one talking.
    const wasStreaming = get().streamingAiId;
    if (aiId) {
      bumpGeneration(aiId);
      const messages = get().messages.filter((m) => m.aiId !== aiId);
      const stopThis = wasStreaming === aiId;
      set((s) => ({
        messages,
        streaming: stopThis ? false : s.streaming,
        streamingAiId: stopThis ? null : s.streamingAiId,
        status: stopThis ? "idle" : s.status,
      }));
      persistMessages(messages);
      if (stopThis) {
        if (isTauri) void stopSpeaking().catch(() => {});
        else webTts.stop();
      }
      return;
    }
    // Clear everything.
    ALL_AIS.forEach((a) => bumpGeneration(a.id));
    set({ messages: [], streaming: false, status: "idle", streamingAiId: null });
    persistMessages([]);
    if (isTauri) void stopSpeaking().catch(() => {});
    else webTts.stop();
  },
}));

if (!isTauri) {
  webTts.configure((speaking) => {
    useChatStore.getState().setStatus(speaking ? "speaking" : "idle");
  });
}
