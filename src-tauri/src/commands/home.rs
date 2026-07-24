//! Home Assistant bridge: lets Jarvis read device states and flip
//! switches/lights by voice. Talks to HA's REST API using a long-lived
//! access token stored in the app's local settings.json — never in the repo.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// Entity domains Jarvis is allowed to control. Deliberately conservative —
/// no locks/covers/climate until explicitly wanted.
const CONTROLLABLE_DOMAINS: &[&str] = &["light", "switch", "input_boolean", "fan", "media_player"];

fn ha_config() -> Option<(String, String)> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let url = json
        .get("homeAssistantUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8123")
        .trim_end_matches('/')
        .to_string();
    let token = json.get("homeAssistantToken")?.as_str()?.trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some((url, token))
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("failed to build Home Assistant HTTP client")
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityState {
    pub entity_id: String,
    pub friendly_name: String,
    pub state: String,
}

pub async fn list_entities_impl() -> Result<Vec<EntityState>, String> {
    let (url, token) =
        ha_config().ok_or_else(|| "Home Assistant is not configured".to_string())?;
    let resp = client()
        .get(format!("{url}/api/states"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("Home Assistant unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Home Assistant returned {}", resp.status()));
    }
    let states: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    Ok(states
        .into_iter()
        .filter_map(|s| {
            let entity_id = s.get("entity_id")?.as_str()?.to_string();
            let domain = entity_id.split('.').next().unwrap_or("");
            if !CONTROLLABLE_DOMAINS.contains(&domain) {
                return None;
            }
            let state = s.get("state")?.as_str()?.to_string();
            let friendly_name = s
                .get("attributes")
                .and_then(|a| a.get("friendly_name"))
                .and_then(|n| n.as_str())
                .unwrap_or(&entity_id)
                .to_string();
            Some(EntityState {
                entity_id,
                friendly_name,
                state,
            })
        })
        .collect())
}

async fn call_service(domain: &str, service: &str, entity_id: &str) -> Result<(), String> {
    let (url, token) =
        ha_config().ok_or_else(|| "Home Assistant is not configured".to_string())?;
    let resp = client()
        .post(format!("{url}/api/services/{domain}/{service}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "entity_id": entity_id }))
        .send()
        .await
        .map_err(|e| format!("Home Assistant unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Home Assistant returned {}", resp.status()));
    }
    Ok(())
}

/// How well `query` matches an entity's friendly name: fraction of query
/// words present in the name (both lowercased). "the workshop light" vs
/// "Workshop Light" scores 1.0 after stop-words are dropped.
fn match_score(query: &str, friendly_name: &str) -> f32 {
    const STOP_WORDS: &[&str] = &["the", "a", "an", "my", "please", "up", "on", "off"];
    let name = friendly_name.to_lowercase();
    let name_words: Vec<&str> = name.split_whitespace().collect();
    let query_words: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP_WORDS.contains(w))
        .map(|w| w.to_string())
        .collect();
    if query_words.is_empty() {
        return 0.0;
    }
    let hits = query_words
        .iter()
        .filter(|qw| name_words.iter().any(|nw| *nw == qw.as_str()))
        .count();
    hits as f32 / query_words.len() as f32
}

/// Parses a spoken command like "turn on the workshop light" / "toggle the
/// lab fan", finds the best-matching controllable entity, and performs the
/// action. Returns Ok(None) when the utterance isn't a control command at
/// all; Ok(Some(summary)) with a human-readable outcome otherwise.
pub async fn home_command_impl(utterance: &str) -> Result<Option<String>, String> {
    let lower = utterance.to_lowercase();

    let (service, target_text) = if let Some(pos) = lower
        .find("turn on")
        .or_else(|| lower.find("switch on"))
        .or_else(|| lower.find("power on"))
    {
        ("turn_on", lower[pos..].splitn(3, ' ').nth(2).unwrap_or("").to_string())
    } else if let Some(pos) = lower
        .find("turn off")
        .or_else(|| lower.find("switch off"))
        .or_else(|| lower.find("power off"))
    {
        ("turn_off", lower[pos..].splitn(3, ' ').nth(2).unwrap_or("").to_string())
    } else if let Some(pos) = lower.find("toggle") {
        ("toggle", lower[pos..].splitn(2, ' ').nth(1).unwrap_or("").to_string())
    } else {
        return Ok(None);
    };

    let target_text = target_text.trim();
    if target_text.is_empty() {
        return Ok(Some(
            "I heard a control command but no device name — say e.g. 'turn on the workshop light'."
                .to_string(),
        ));
    }

    let entities = list_entities_impl().await?;
    if entities.is_empty() {
        return Ok(Some(
            "No controllable devices are set up in Home Assistant yet.".to_string(),
        ));
    }

    let best = entities
        .iter()
        .map(|e| (match_score(target_text, &e.friendly_name), e))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    match best {
        Some((score, entity)) if score >= 0.5 => {
            let domain = entity.entity_id.split('.').next().unwrap_or("homeassistant");
            call_service(domain, service, &entity.entity_id).await?;
            let verb = match service {
                "turn_on" => "Turned on",
                "turn_off" => "Turned off",
                _ => "Toggled",
            };
            Ok(Some(format!("{verb} {}.", entity.friendly_name)))
        }
        _ => {
            let known: Vec<String> = entities.iter().map(|e| e.friendly_name.clone()).collect();
            Ok(Some(format!(
                "No device matched '{target_text}'. Devices I can control: {}.",
                known.join(", ")
            )))
        }
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn home_command(utterance: String) -> Result<Option<String>, String> {
    home_command_impl(&utterance).await
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_home_entities() -> Result<Vec<EntityState>, String> {
    list_entities_impl().await
}
