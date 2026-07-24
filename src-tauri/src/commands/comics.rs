//! Comic Vine lookups for the "Hacks" Sub AI: real issue data + cover art for
//! Iron Man (and any) comics. Uses the free Comic Vine API — the user pastes
//! their key into Settings (stored in the app's local settings.json).

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::commands::browser::urlencode;

fn comicvine_key() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let key = json.get("comicVineApiKey")?.as_str()?.trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

#[derive(Debug, Serialize)]
pub struct ComicResult {
    pub name: String,
    pub issue_number: String,
    pub cover_date: String,
    pub volume: String,
    pub image_url: String,
    pub detail_url: String,
}

#[derive(Deserialize)]
struct CvResponse {
    results: Vec<CvIssue>,
}

#[derive(Deserialize)]
struct CvIssue {
    name: Option<String>,
    issue_number: Option<String>,
    cover_date: Option<String>,
    volume: Option<CvVolume>,
    image: Option<CvImage>,
    site_detail_url: Option<String>,
}

#[derive(Deserialize)]
struct CvVolume {
    name: Option<String>,
}

#[derive(Deserialize)]
struct CvImage {
    medium_url: Option<String>,
    original_url: Option<String>,
}

pub async fn comicvine_search_impl(query: String) -> Result<Vec<ComicResult>, String> {
    let key = comicvine_key()
        .ok_or_else(|| "Add your free Comic Vine API key in Settings first.".to_string())?;
    // Comic Vine's `search` endpoint scoped to issues, newest-relevant first.
    let url = format!(
        "https://comicvine.gamespot.com/api/search/?api_key={key}&format=json&limit=12&resources=issue&query={}&field_list=name,issue_number,cover_date,image,volume,site_detail_url",
        urlencode(&query)
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    // Comic Vine blocks requests without a real User-Agent.
    let resp = client
        .get(&url)
        .header("User-Agent", "Jarvis2-Assistant/1.0")
        .send()
        .await
        .map_err(|e| format!("Comic Vine request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Comic Vine returned {}", resp.status()));
    }
    let parsed: CvResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Comic Vine response: {e}"))?;

    let results = parsed
        .results
        .into_iter()
        .map(|i| {
            let volume = i.volume.and_then(|v| v.name).unwrap_or_default();
            let image_url = i
                .image
                .and_then(|im| im.medium_url.or(im.original_url))
                .unwrap_or_default();
            ComicResult {
                name: i.name.unwrap_or_default(),
                issue_number: i.issue_number.unwrap_or_default(),
                cover_date: i.cover_date.unwrap_or_default(),
                volume,
                image_url,
                detail_url: i.site_detail_url.unwrap_or_default(),
            }
        })
        .filter(|c| !c.image_url.is_empty())
        .collect();
    Ok(results)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn comicvine_search(query: String) -> Result<Vec<ComicResult>, String> {
    comicvine_search_impl(query).await
}
