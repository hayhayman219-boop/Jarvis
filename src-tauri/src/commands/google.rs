//! Google Calendar via the private "secret address in iCal format" URL.
//!
//! Google exposes each calendar as a private iCalendar feed at a URL that
//! carries its own token (Google Calendar → Settings → *Integrate calendar* →
//! *Secret address in iCal format*). Fetching it needs no OAuth, so Jarvis just
//! GETs the feed(s) and parses them with the shared iCalendar parser. The user
//! pastes one or more of these URLs (whitespace/newline/comma separated) into
//! settings.
//!
//! Trade-off vs. the OAuth API: the iCal feed does NOT expand recurring events
//! — a repeating event appears once at its series start — so only one-off
//! events land on the exact day. Expanding recurrences would require the
//! OAuth-authenticated API.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};

use crate::commands::apple::{expand_ics_events, AppleEvent};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const LOOKBACK_DAYS: i64 = 1;
const LOOKAHEAD_DAYS: i64 = 45;
const MAX_EVENTS: usize = 40;

/// Calendar sources — either a Google "secret iCal" URL (http…) or a local
/// .ics file path (/… or ~/…), whitespace/comma separated in settings.
fn calendar_sources() -> Result<Vec<String>, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "Google Calendar is not configured".to_string())?;
    let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
    let raw = std::fs::read_to_string(path)
        .map_err(|_| "Google Calendar is not configured — add a secret iCal URL in Settings.".to_string())?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Could not read settings: {e}"))?;
    let field = json
        .get("googleCalUrls")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sources: Vec<String> = field
        .split(['\n', '\r'])
        .map(|s| s.trim())
        .filter(|s| s.starts_with("http") || s.starts_with('/') || s.starts_with('~'))
        .map(|s| s.to_string())
        .collect();
    if sources.is_empty() {
        return Err(
            "Google Calendar is not configured — paste your calendar's \"Secret address in \
             iCal format\" (Google Calendar → Settings → Integrate calendar) into Settings → \
             Google Calendar."
                .to_string(),
        );
    }
    Ok(sources)
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

/// Pulls the human calendar name out of an iCal feed (X-WR-CALNAME), if present.
fn cal_name(ics: &str) -> String {
    for line in ics.lines() {
        if let Some(rest) = line.strip_prefix("X-WR-CALNAME:") {
            let name = rest.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    "Google".to_string()
}

pub async fn list_events_impl() -> Result<Vec<AppleEvent>, String> {
    let sources = calendar_sources()?;
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let today = Utc::now().date_naive();
    let from = today - ChronoDuration::days(LOOKBACK_DAYS);
    let until = today + ChronoDuration::days(LOOKAHEAD_DAYS);

    let mut events: Vec<AppleEvent> = Vec::new();
    let mut last_err: Option<String> = None;
    for src in &sources {
        let ics: Option<String> = if src.starts_with("http") {
            match client.get(src).send().await {
                Ok(resp) if resp.status().is_success() => Some(resp.text().await.unwrap_or_default()),
                Ok(resp) => {
                    last_err = Some(format!("Google feed returned {}", resp.status()));
                    None
                }
                Err(e) => {
                    last_err = Some(format!("Could not reach Google feed: {e}"));
                    None
                }
            }
        } else {
            match std::fs::read_to_string(expand_home(src)) {
                Ok(s) => Some(s),
                Err(e) => {
                    last_err = Some(format!("Could not read {src}: {e}"));
                    None
                }
            }
        };
        if let Some(ics) = ics {
            let name = cal_name(&ics);
            // Recurrences are expanded within the window here.
            events.extend(expand_ics_events(&ics, &name, from, until));
        }
    }

    // If every source failed, surface why rather than silently showing nothing.
    if events.is_empty() {
        if let Some(e) = last_err {
            return Err(e);
        }
    }

    events.sort_by(|a, b| a.start.cmp(&b.start));
    events.truncate(MAX_EVENTS);
    Ok(events)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_google_events() -> Result<Vec<AppleEvent>, String> {
    list_events_impl().await
}
