//! Background news monitor. While enabled in settings, Jarvis periodically
//! checks the configured news feeds and fires a desktop notification (and can
//! speak) when a genuinely new headline appears — so breaking news reaches the
//! user even when they aren't actively chatting. Seen headlines persist to disk
//! so a restart doesn't re-announce everything, and the first pass after a
//! fresh start learns the current headlines silently rather than blasting one
//! notification per existing story.

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::commands::news::{self, NewsCache};
#[cfg(feature = "desktop")]
use crate::commands::voice::{self, VoiceState};

/// Give the app a moment to finish starting before the first check. Both
/// intervals are env-overridable (seconds) for testing / tuning.
const STARTUP_DELAY_SECS: u64 = 45;
const POLL_INTERVAL_SECS: u64 = 15 * 60;
/// Cap notifications per cycle so a burst of new stories can't spam the user.
const MAX_ALERTS_PER_CYCLE: usize = 3;

fn dur_env(key: &str, default_secs: u64) -> Duration {
    let secs = std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(default_secs);
    Duration::from_secs(secs)
}
/// Bound the persisted "seen" list so it can't grow without limit.
const MAX_SEEN: usize = 600;

struct MonitorConfig {
    enabled: bool,
    categories: Vec<String>,
    announce_aloud: bool,
}

fn settings_json() -> serde_json::Value {
    std::env::var("HOME")
        .ok()
        .and_then(|h| {
            std::fs::read_to_string(
                PathBuf::from(h).join(".local/share/com.jarvis.assistant/settings.json"),
            )
            .ok()
        })
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null)
}

fn read_config() -> MonitorConfig {
    let s = settings_json();
    let enabled = s
        .get("newsMonitorEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let announce_aloud = s
        .get("newsMonitorAloud")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let categories = s
        .get("newsMonitorCategories")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["ai".to_string(), "tech".to_string()]);
    MonitorConfig {
        enabled,
        categories,
        announce_aloud,
    }
}

fn seen_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("news_seen.json"))
}

fn load_seen(app: &AppHandle) -> Vec<String> {
    seen_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_seen(app: &AppHandle, seen: &[String]) {
    if let Some(p) = seen_path(app) {
        if let Ok(json) = serde_json::to_string(seen) {
            let _ = std::fs::write(p, json);
        }
    }
}

fn headline_key(item: &news::NewsItem) -> String {
    if item.url.is_empty() {
        format!("{}|{}", item.source, item.title)
    } else {
        item.url.clone()
    }
}

pub fn start(app: AppHandle) {
    let startup = dur_env("JARVIS_NEWS_STARTUP_SECS", STARTUP_DELAY_SECS);
    let poll = dur_env("JARVIS_NEWS_POLL_SECS", POLL_INTERVAL_SECS);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(startup).await;
        let mut seen = load_seen(&app);
        let mut seeding = seen.is_empty();

        loop {
            let cfg = read_config();
            if cfg.enabled {
                let mut fresh: Vec<(String, news::NewsItem)> = Vec::new();
                {
                    let cache = app.state::<NewsCache>();
                    for cat in &cfg.categories {
                        if let Ok(items) = news::get_news_impl(cache.inner(), cat.clone()).await {
                            for item in items {
                                let key = headline_key(&item);
                                if !seen.iter().any(|k| k == &key) {
                                    seen.push(key);
                                    if !seeding {
                                        fresh.push((cat.clone(), item));
                                    }
                                }
                            }
                        }
                    }
                }

                eprintln!(
                    "[news-monitor] checked {} categor{} — {} new headline(s){}",
                    cfg.categories.len(),
                    if cfg.categories.len() == 1 { "y" } else { "ies" },
                    fresh.len(),
                    if seeding { " (seeding, silent)" } else { "" }
                );

                if !seeding && !fresh.is_empty() {
                    for (cat, item) in fresh.iter().take(MAX_ALERTS_PER_CYCLE) {
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("Jarvis — {cat} news"))
                            .body(format!("{} · {}", item.source, item.title))
                            .show();
                    }
                    #[cfg(feature = "desktop")]
                    if cfg.announce_aloud {
                        if let Some(voice) = app.try_state::<VoiceState>() {
                            let spoken = if fresh.len() == 1 {
                                format!("Breaking news: {}", fresh[0].1.title)
                            } else {
                                format!(
                                    "{} new headlines. Top story: {}",
                                    fresh.len(),
                                    fresh[0].1.title
                                )
                            };
                            let _ = voice::speak_impl(voice.inner(), spoken);
                        }
                    }
                }

                if seen.len() > MAX_SEEN {
                    let drop = seen.len() - MAX_SEEN;
                    seen.drain(0..drop);
                }
                save_seen(&app, &seen);
                seeding = false;
            }
            tokio::time::sleep(poll).await;
        }
    });
}
