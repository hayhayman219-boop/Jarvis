import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./env";
import type {
  AppleEvent,
  AppleReminder,
  ChatMessage,
  FileEntry,
  GeocodeResult,
  ModelInfo,
  NewsCategory,
  NewsItem,
  Reminder,
  SystemStats,
  WeatherResponse,
} from "./types";

const DESKTOP_ONLY_ERROR = "Computer control is only available in the desktop app, not this web/phone interface.";

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, init);
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(text || `Request to ${path} failed (${res.status})`);
  }
  return res.json() as Promise<T>;
}

const jsonHeaders = { "Content-Type": "application/json" };

export function listModels(): Promise<ModelInfo[]> {
  if (isTauri) return invoke("list_models");
  return apiFetch("/models");
}

export function chat(model: string, messages: ChatMessage[]): Promise<string> {
  if (isTauri) return invoke("chat", { model, messages });
  return apiFetch("/chat", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ model, messages }),
  });
}

/// Web-only streaming chat: reads the server's chunked plain-text response,
/// invoking `onToken` per chunk. (The desktop app streams via the
/// `chat_stream` Tauri command + window events instead — see chatStore.)
export async function chatStreamWeb(
  model: string,
  messages: ChatMessage[],
  onToken: (token: string) => void,
): Promise<string> {
  const res = await fetch("/api/chat-stream", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ model, messages }),
  });
  if (!res.ok || !res.body) {
    throw new Error((await res.text().catch(() => "")) || `chat stream failed (${res.status})`);
  }
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let full = "";
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    const chunk = decoder.decode(value, { stream: true });
    if (chunk) {
      full += chunk;
      onToken(chunk);
    }
  }
  return full;
}

export function setVoiceLoopEnabled(enabled: boolean): Promise<void> {
  if (isTauri) return invoke("set_voice_loop_enabled", { enabled });
  return Promise.resolve();
}

export function setVoiceLoopPaused(paused: boolean): Promise<void> {
  if (isTauri) return invoke("set_voice_loop_paused", { paused });
  return Promise.resolve();
}

export function getWeather(lat: number, lon: number, fahrenheit: boolean): Promise<WeatherResponse> {
  if (isTauri) return invoke("get_weather", { lat, lon, fahrenheit });
  return apiFetch(`/weather?lat=${lat}&lon=${lon}&fahrenheit=${fahrenheit}`);
}

export function geocodeCity(name: string): Promise<GeocodeResult> {
  if (isTauri) return invoke("geocode_city", { name });
  return apiFetch(`/geocode?name=${encodeURIComponent(name)}`);
}

export function listReminders(): Promise<Reminder[]> {
  if (isTauri) return invoke("list_reminders");
  return apiFetch("/reminders");
}

export function deleteReminder(id: number): Promise<void> {
  if (isTauri) return invoke("delete_reminder", { id });
  return apiFetch(`/reminders/${id}`, { method: "DELETE" });
}

export function listAppleEvents(): Promise<AppleEvent[]> {
  if (isTauri) return invoke("list_apple_events");
  return apiFetch("/apple/events");
}

export function listAppleReminders(): Promise<AppleReminder[]> {
  if (isTauri) return invoke("list_apple_reminders");
  return apiFetch("/apple/reminders");
}

export function listGoogleEvents(): Promise<AppleEvent[]> {
  if (isTauri) return invoke("list_google_events");
  return apiFetch("/google/events");
}

export function parseAndCreateReminder(model: string, utterance: string): Promise<Reminder> {
  if (isTauri) return invoke("parse_and_create_reminder", { model, utterance });
  return apiFetch("/reminders/parse", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ model, utterance }),
  });
}

export function transcribe(audio: number[], fast = false): Promise<string> {
  if (isTauri) return invoke("transcribe", { audio, fast });
  return apiFetch("/transcribe", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ audio, fast }),
  });
}

let currentAudio: HTMLAudioElement | null = null;

export async function speak(text: string): Promise<void> {
  if (isTauri) return invoke("speak", { text });

  const res = await fetch("/api/speak", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ text }),
  });
  if (!res.ok) {
    throw new Error((await res.text().catch(() => "")) || `speak failed (${res.status})`);
  }
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);

  currentAudio?.pause();
  currentAudio = new Audio(url);
  currentAudio.addEventListener("ended", () => URL.revokeObjectURL(url));
  await currentAudio.play();
}

export function stopSpeaking(): Promise<void> {
  if (isTauri) return invoke("stop_speaking");
  currentAudio?.pause();
  currentAudio = null;
  return Promise.resolve();
}

/** Renders `content` (Markdown) to a PDF in the user's Downloads folder,
 * returning the saved path. Desktop only. */
export function exportPdf(title: string, content: string): Promise<string> {
  if (isTauri) return invoke<string>("export_pdf", { title, content });
  return Promise.reject(new Error("PDF export is only available in the desktop app."));
}

/** Screenshots the screen and OCRs it, returning the recognized text. */
export function readScreen(): Promise<string> {
  if (isTauri) return invoke<string>("read_screen");
  return Promise.reject(new Error("Screen OCR is only available in the desktop app."));
}

export interface ComicResult {
  name: string;
  issue_number: string;
  cover_date: string;
  volume: string;
  image_url: string;
  detail_url: string;
}

/** Searches Comic Vine for issues (name/cover/date) — used by Hacks. */
export function comicVineSearch(query: string): Promise<ComicResult[]> {
  if (isTauri) return invoke<ComicResult[]>("comicvine_search", { query });
  return Promise.reject(new Error("Comic lookup is only available in the desktop app."));
}

export function getSystemStats(): Promise<SystemStats> {
  if (isTauri) return invoke("get_system_stats");
  return apiFetch("/system-stats");
}

export function getNews(category: NewsCategory): Promise<NewsItem[]> {
  if (isTauri) return invoke("get_news", { category });
  return apiFetch(`/news?category=${encodeURIComponent(category)}`);
}

/** Executes a spoken device command ("turn on the workshop light") against
 * Home Assistant. Resolves to a human-readable outcome, or null when the
 * utterance isn't a device-control command. */
export function homeCommand(utterance: string): Promise<string | null> {
  if (isTauri) return invoke("home_command", { utterance });
  return apiFetch("/home/command", {
    method: "POST",
    headers: jsonHeaders,
    body: JSON.stringify({ utterance }),
  });
}

export interface NotionPage {
  id: string;
  title: string;
  url: string;
  last_edited: string | null;
}

/** Lists Notion pages the integration can see (newest-edited first). */
export function listNotionPages(): Promise<NotionPage[]> {
  if (isTauri) return invoke("list_notion_pages");
  return apiFetch("/notion/pages");
}

/** Reads a Notion page's content as flattened plain text. */
export function readNotionPage(pageId: string): Promise<string> {
  if (isTauri) return invoke("read_notion_page", { pageId });
  return apiFetch(`/notion/page?id=${encodeURIComponent(pageId)}`);
}

/** Computer control (Phase 3): desktop-only, no web/phone route exists for
 * these, since the web server has no auth and this is unrestricted shell
 * access. On web, these reject immediately with an explanatory error. */
export function openApp(name: string): Promise<string> {
  if (isTauri) return invoke("open_app", { name });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function openNews(category?: string): Promise<string> {
  if (isTauri) return invoke("open_news", { category: category ?? null });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function openWeb(target: string, search: boolean): Promise<string> {
  if (isTauri) return invoke("open_web", { target, search });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function addToCart(retailer: string, item: string): Promise<string> {
  if (isTauri) return invoke("add_to_cart", { retailer, item });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function openUrls(urls: string[]): Promise<string> {
  if (isTauri) return invoke("open_urls", { urls });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function setVolume(action: string): Promise<string> {
  if (isTauri) return invoke("set_volume", { action });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function setBrightness(action: string): Promise<string> {
  if (isTauri) return invoke("set_brightness", { action });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function lockScreen(): Promise<string> {
  if (isTauri) return invoke("lock_screen");
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function takeScreenshot(): Promise<string> {
  if (isTauri) return invoke("take_screenshot");
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function closeApp(name: string): Promise<string> {
  if (isTauri) return invoke("close_app", { name });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function runCommand(command: string): Promise<string> {
  if (isTauri) return invoke("run_command", { command });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}

export function listFiles(path: string): Promise<FileEntry[]> {
  if (isTauri) return invoke("list_files", { path });
  return Promise.reject(new Error(DESKTOP_ONLY_ERROR));
}
