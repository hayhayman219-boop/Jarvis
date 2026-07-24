//! Notion viewer: lets Jarvis list and read pages from the user's Notion
//! workspace via the official API. Uses an internal-integration token stored
//! (like the Home Assistant credentials) in the app's local
//! settings.json — never in the repo. The integration only sees pages the
//! user has explicitly shared with it.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const NOTION_VERSION: &str = "2022-06-28";
const API: &str = "https://api.notion.com/v1";
/// Cap page content so a huge page can't flood the LLM context or the UI.
const MAX_BLOCKS: usize = 120;

/// Reads and validates the integration token. Returns a user-facing error
/// string rather than an Option so callers can surface *why* it's unusable —
/// distinguishing "you haven't set it" from "what you pasted isn't a token"
/// (e.g. the setup-instructions URL landed in the field), which otherwise
/// only shows up as an opaque Notion 401.
fn notion_token() -> Result<String, String> {
    let home = std::env::var("HOME").map_err(|_| "Notion is not configured".to_string())?;
    let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
    let raw = std::fs::read_to_string(path)
        .map_err(|_| "Notion is not configured — add a token in Settings.".to_string())?;
    let json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Could not read settings: {e}"))?;
    let token = json
        .get("notionToken")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if token.is_empty() {
        return Err("Notion is not configured — add a token in Settings.".to_string());
    }
    // Internal-integration secrets are "ntn_…" (current) or "secret_…" (older).
    // Anything else is almost certainly a wrong paste (a URL, a page link),
    // and would just bounce off Notion as a 401 with a cryptic body.
    if !token.starts_with("ntn_") && !token.starts_with("secret_") {
        return Err(
            "That doesn't look like a Notion integration token. Create one at \
             notion.so/my-integrations and paste the secret (starts with \"ntn_\") \
             into Settings → Notion."
                .to_string(),
        );
    }
    Ok(token)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("failed to build Notion HTTP client")
}

#[derive(Debug, Clone, Serialize)]
pub struct NotionPage {
    pub id: String,
    pub title: String,
    pub url: String,
    pub last_edited: Option<String>,
}

/// Extracts a human title from a Notion object. Pages expose their title
/// under a "title"-typed property whose name varies ("Name", "Title", …),
/// so we scan the properties for the first title-typed one.
fn extract_title(obj: &serde_json::Value) -> String {
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        for (_key, val) in props {
            if val.get("type").and_then(|t| t.as_str()) == Some("title") {
                if let Some(arr) = val.get("title").and_then(|t| t.as_array()) {
                    let text: String = arr
                        .iter()
                        .filter_map(|rt| rt.get("plain_text").and_then(|p| p.as_str()))
                        .collect();
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
            }
        }
    }
    // Databases carry their title at the top level instead.
    if let Some(arr) = obj.get("title").and_then(|t| t.as_array()) {
        let text: String = arr
            .iter()
            .filter_map(|rt| rt.get("plain_text").and_then(|p| p.as_str()))
            .collect();
        if !text.trim().is_empty() {
            return text;
        }
    }
    "Untitled".to_string()
}

pub async fn list_pages_impl() -> Result<Vec<NotionPage>, String> {
    let token = notion_token()?;
    // Empty search body returns everything shared with the integration,
    // newest-edited first.
    let resp = client()
        .post(format!("{API}/search"))
        .bearer_auth(&token)
        .header("Notion-Version", NOTION_VERSION)
        .json(&serde_json::json!({
            "sort": { "direction": "descending", "timestamp": "last_edited_time" },
            "page_size": 50
        }))
        .send()
        .await
        .map_err(|e| format!("Notion unreachable: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Notion {status}: {body}"));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let results = json
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Unexpected Notion response".to_string())?;

    Ok(results
        .iter()
        .filter_map(|obj| {
            let id = obj.get("id")?.as_str()?.to_string();
            let url = obj
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            let last_edited = obj
                .get("last_edited_time")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());
            Some(NotionPage {
                id,
                title: extract_title(obj),
                url,
                last_edited,
            })
        })
        .collect())
}

/// Flattens a page's block children into readable plain text. Handles the
/// common block types (paragraphs, headings, lists, to-dos, quotes, code);
/// unknown types are skipped rather than erroring.
fn blocks_to_text(blocks: &[serde_json::Value]) -> String {
    let mut out = String::new();
    for block in blocks.iter().take(MAX_BLOCKS) {
        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let rich = block
            .get(btype)
            .and_then(|b| b.get("rich_text"))
            .and_then(|r| r.as_array());
        let text: String = rich
            .map(|arr| {
                arr.iter()
                    .filter_map(|rt| rt.get("plain_text").and_then(|p| p.as_str()))
                    .collect::<String>()
            })
            .unwrap_or_default();
        let line = match btype {
            "heading_1" => format!("\n# {text}\n"),
            "heading_2" => format!("\n## {text}\n"),
            "heading_3" => format!("\n### {text}\n"),
            "bulleted_list_item" => format!("• {text}\n"),
            "numbered_list_item" => format!("- {text}\n"),
            "to_do" => {
                let checked = block
                    .get("to_do")
                    .and_then(|t| t.get("checked"))
                    .and_then(|c| c.as_bool())
                    .unwrap_or(false);
                format!("[{}] {text}\n", if checked { "x" } else { " " })
            }
            "quote" => format!("> {text}\n"),
            "code" => format!("```\n{text}\n```\n"),
            "paragraph" => {
                if text.trim().is_empty() {
                    "\n".to_string()
                } else {
                    format!("{text}\n")
                }
            }
            _ => {
                if text.trim().is_empty() {
                    String::new()
                } else {
                    format!("{text}\n")
                }
            }
        };
        out.push_str(&line);
    }
    out.trim().to_string()
}

pub async fn read_page_impl(page_id: String) -> Result<String, String> {
    let token = notion_token()?;
    let resp = client()
        .get(format!("{API}/blocks/{page_id}/children?page_size={MAX_BLOCKS}"))
        .bearer_auth(&token)
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| format!("Notion unreachable: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Notion {status}: {body}"));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let blocks = json
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Unexpected Notion response".to_string())?;
    let text = blocks_to_text(blocks);
    Ok(if text.is_empty() {
        "(This page has no readable text content.)".to_string()
    } else {
        text
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_notion_pages() -> Result<Vec<NotionPage>, String> {
    list_pages_impl().await
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn read_notion_page(page_id: String) -> Result<String, String> {
    read_page_impl(page_id).await
}
