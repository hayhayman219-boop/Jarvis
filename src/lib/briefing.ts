import { getWeather, listGoogleEvents, listReminders, getNews } from "./apiClient";
import { useSettingsStore } from "../state/settingsStore";
import type { AppleEvent } from "./types";

// Assembles the spoken "Daily Briefing": greeting + weather + today's calendar
// + reminders due today + a few headlines. Every section degrades gracefully
// so a failing feed (e.g. no calendar configured) just gets skipped.

function greeting(): string {
  const h = new Date().getHours();
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}

function isToday(iso: string): boolean {
  const d = new Date(iso.length <= 10 ? `${iso}T00:00:00` : iso);
  const now = new Date();
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  );
}

function eventTime(e: AppleEvent): string {
  if (e.all_day) return "all day";
  const d = new Date(e.start);
  return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export async function buildBriefing(): Promise<string> {
  const parts: string[] = [`${greeting()}, sir.`];
  const { location, temperatureUnit, newsMonitorCategories } = useSettingsStore.getState();

  // Weather
  if (location) {
    try {
      const w = await getWeather(
        location.latitude,
        location.longitude,
        temperatureUnit !== "celsius",
      );
      parts.push(
        `It's currently ${Math.round(w.temperature)}${w.temperature_unit} and ${w.condition.toLowerCase()} in ${location.name}.`,
      );
    } catch {
      /* skip */
    }
  }

  // Today's calendar
  try {
    const events = (await listGoogleEvents())
      .filter((e) => isToday(e.start))
      .sort((a, b) => a.start.localeCompare(b.start));
    if (events.length === 0) {
      parts.push("You have nothing on your calendar today.");
    } else {
      const list = events
        .slice(0, 5)
        .map((e) => `${e.summary} at ${eventTime(e)}`)
        .join("; ");
      parts.push(
        `You have ${events.length} event${events.length === 1 ? "" : "s"} today: ${list}.`,
      );
    }
  } catch {
    /* skip */
  }

  // Reminders due today
  try {
    const due = (await listReminders()).filter((r) => !r.fired && isToday(r.due_at));
    if (due.length > 0) {
      const list = due.slice(0, 5).map((r) => r.text).join("; ");
      parts.push(
        `${due.length} reminder${due.length === 1 ? "" : "s"} for today: ${list}.`,
      );
    }
  } catch {
    /* skip */
  }

  // Headlines
  try {
    const category = newsMonitorCategories?.[0] ?? "tech";
    const news = await getNews(category as never);
    if (news.length > 0) {
      const heads = news.slice(0, 3).map((n) => n.title).join("; ");
      parts.push(`Top ${category} headlines: ${heads}.`);
    }
  } catch {
    /* skip */
  }

  parts.push("That's your briefing.");
  return parts.join(" ");
}
