export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export type ReactorStatus = "idle" | "listening" | "thinking" | "speaking";

export interface WeatherResponse {
  temperature: number;
  temperature_unit: string;
  windspeed: number;
  windspeed_unit: string;
  condition: string;
}

export interface GeocodeResult {
  name: string;
  latitude: number;
  longitude: number;
  country: string | null;
}

export interface Reminder {
  id: number;
  text: string;
  due_at: string;
  fired: boolean;
}

export interface AppleEvent {
  summary: string;
  /** RFC3339 (UTC) for timed events, or "YYYY-MM-DD" for all-day ones. */
  start: string;
  end: string | null;
  all_day: boolean;
  calendar: string;
}

export interface AppleReminder {
  summary: string;
  due: string | null;
  calendar: string;
}

export interface ModelInfo {
  name: string;
  is_remote: boolean;
}

export interface SystemStats {
  cpu_percent: number;
  memory_percent: number;
  disk_percent: number;
  temperature_c: number | null;
}

export type NewsCategory = "pokemon" | "tech" | "ai" | "datacenter";

export interface NewsItem {
  source: string;
  title: string;
  url: string;
  published: string | null;
}

export interface FileEntry {
  name: string;
  is_dir: boolean;
  size: number;
}
