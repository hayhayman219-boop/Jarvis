//! Notion viewer: lets Jarvis list and read pages from the user's Notion
//! workspace via the official API. Uses an internal-integration token stored
//! (like the Home Assistant credentials) in the app's local
//! settings.json — never in the repo. The integration only sees pages the
//! user has explicitly shared with it.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
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
            // Skip database ROWS (their parent is a database) — they clutter
            // the list; you read them inside their database instead.
            if obj.get("parent").and_then(|p| p.get("type")).and_then(|t| t.as_str())
                == Some("database_id")
            {
                return None;
            }
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

fn rich_text(block: &serde_json::Value, btype: &str) -> String {
    block
        .get(btype)
        .and_then(|b| b.get("rich_text"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|rt| rt.get("plain_text").and_then(|p| p.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// One block -> one readable line (the common text block types).
fn block_to_line(block: &serde_json::Value) -> String {
    let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let text = rich_text(block, btype);
    match btype {
        "heading_1" => format!("\n# {text}\n"),
        "heading_2" => format!("\n## {text}\n"),
        "heading_3" => format!("\n### {text}\n"),
        "bulleted_list_item" => format!("- {text}\n"),
        "numbered_list_item" => format!("- {text}\n"),
        "to_do" => {
            let checked = block
                .get("to_do")
                .and_then(|t| t.get("checked"))
                .and_then(|c| c.as_bool())
                .unwrap_or(false);
            format!("- [{}] {text}\n", if checked { "x" } else { " " })
        }
        "quote" => format!("> {text}\n"),
        "code" => format!("```\n{text}\n```\n"),
        "callout" | "toggle" | "paragraph" => {
            if text.trim().is_empty() {
                String::new()
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
    }
}

fn format_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n:.2}")
    }
}

/// A single database-row property rendered as text (title, number, select,
/// date, checkbox, formula, …). Returns None for empty/unsupported values.
fn prop_text(prop: &serde_json::Value) -> Option<String> {
    let joined = |arr: &serde_json::Value| -> String {
        arr.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("plain_text").and_then(|p| p.as_str()))
                    .collect::<String>()
            })
            .unwrap_or_default()
    };
    let val = match prop.get("type").and_then(|t| t.as_str())? {
        "title" => joined(prop.get("title")?),
        "rich_text" => joined(prop.get("rich_text")?),
        "number" => prop.get("number").and_then(|n| n.as_f64()).map(format_num)?,
        "select" => prop.get("select")?.get("name")?.as_str()?.to_string(),
        "status" => prop.get("status")?.get("name")?.as_str()?.to_string(),
        "multi_select" => prop
            .get("multi_select")?
            .as_array()?
            .iter()
            .filter_map(|o| o.get("name").and_then(|n| n.as_str()))
            .collect::<Vec<_>>()
            .join(", "),
        "date" => prop.get("date")?.get("start")?.as_str()?.to_string(),
        "checkbox" => {
            if prop.get("checkbox")?.as_bool()? {
                "yes".to_string()
            } else {
                "no".to_string()
            }
        }
        "url" => prop.get("url")?.as_str()?.to_string(),
        "formula" => {
            let f = prop.get("formula")?;
            f.get("number")
                .and_then(|n| n.as_f64())
                .map(format_num)
                .or_else(|| f.get("string").and_then(|s| s.as_str()).map(|s| s.to_string()))?
        }
        _ => return None,
    };
    let val = val.trim().to_string();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// A database row -> "Title — Prop: val, Prop2: val2".
fn row_to_line(row: &serde_json::Value) -> String {
    let Some(props) = row.get("properties").and_then(|p| p.as_object()) else {
        return String::new();
    };
    let mut title = String::new();
    let mut extras: Vec<String> = Vec::new();
    for (key, val) in props {
        let is_title = val.get("type").and_then(|t| t.as_str()) == Some("title");
        if let Some(text) = prop_text(val) {
            if is_title {
                title = text;
            } else {
                extras.push(format!("{key}: {text}"));
            }
        }
    }
    match (title.is_empty(), extras.is_empty()) {
        (false, false) => format!("{title} — {}", extras.join(", ")),
        (false, true) => title,
        (true, false) => extras.join(", "),
        (true, true) => String::new(),
    }
}

/// Renders a database as a titled list of its rows.
async fn render_database(token: &str, db_id: &str) -> Result<String, String> {
    let meta = client()
        .get(format!("{API}/databases/{db_id}"))
        .bearer_auth(token)
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !meta.status().is_success() {
        return Err("not a database".to_string());
    }
    let title = extract_title(&meta.json::<serde_json::Value>().await.map_err(|e| e.to_string())?);
    let resp = client()
        .post(format!("{API}/databases/{db_id}/query"))
        .bearer_auth(token)
        .header("Notion-Version", NOTION_VERSION)
        .json(&serde_json::json!({ "page_size": 100 }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err("query failed".to_string());
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let rows = json.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let mut out = format!("\n## {title}\n");
    for row in &rows {
        let line = row_to_line(row);
        if !line.trim().is_empty() {
            out.push_str(&format!("- {line}\n"));
        }
    }
    Ok(out)
}

/// Recursively renders a block's children into text — descending into layout
/// containers (columns, toggles) and rendering any embedded databases, so a
/// page built out of those (like a budget template) actually shows content.
fn render_block_children<'a>(
    token: &'a str,
    block_id: &'a str,
    depth: u8,
) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> {
    Box::pin(async move {
        if depth > 3 {
            return String::new();
        }
        let resp = match client()
            .get(format!("{API}/blocks/{block_id}/children?page_size={MAX_BLOCKS}"))
            .bearer_auth(token)
            .header("Notion-Version", NOTION_VERSION)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => return String::new(),
        };
        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(_) => return String::new(),
        };
        let blocks = json.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
        let mut out = String::new();
        for block in blocks.iter().take(MAX_BLOCKS) {
            let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
            match btype {
                "child_database" => {
                    if let Ok(t) = render_database(token, id).await {
                        out.push_str(&t);
                    }
                }
                "column_list" | "column" | "toggle" | "synced_block" | "callout" => {
                    out.push_str(&block_to_line(block));
                    out.push_str(&render_block_children(token, id, depth + 1).await);
                }
                _ => out.push_str(&block_to_line(block)),
            }
        }
        out
    })
}

pub async fn read_page_impl(page_id: String) -> Result<String, String> {
    let token = notion_token()?;
    // Render the page's blocks (descending into columns/toggles and rendering
    // embedded databases). If it turns up empty, the id may BE a database —
    // render its rows directly.
    let mut text = render_block_children(&token, &page_id, 0).await;
    if text.trim().is_empty() {
        if let Ok(t) = render_database(&token, &page_id).await {
            text = t;
        }
    }
    let text = text.trim().to_string();
    Ok(if text.is_empty() {
        "(This page has no readable text content.)".to_string()
    } else {
        text
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub name: String,
    pub amount: Option<f64>,
}

/// The Notion database to read subscriptions from. Prefers an explicit
/// `notionSubscriptionsDb` id in settings; otherwise searches the workspace for
/// a database whose title looks budget/expense/subscription related (e.g. the
/// user's "Expenses (Monthly)"), so no manual configuration is needed.
async fn subscriptions_db_id(token: &str) -> Result<String, String> {
    if let Ok(home) = std::env::var("HOME") {
        let path = PathBuf::from(home).join(".local/share/com.jarvis.assistant/settings.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(id) = json.get("notionSubscriptionsDb").and_then(|v| v.as_str()) {
                    let id = id.trim();
                    if !id.is_empty() {
                        return Ok(id.to_string());
                    }
                }
            }
        }
    }
    // Fall back to finding a likely database by title.
    let resp = client()
        .post(format!("{API}/search"))
        .bearer_auth(token)
        .header("Notion-Version", NOTION_VERSION)
        .json(&serde_json::json!({
            "filter": { "property": "object", "value": "database" },
            "page_size": 50
        }))
        .send()
        .await
        .map_err(|e| format!("Notion unreachable: {e}"))?;
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let results = json
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    for db in &results {
        let title = extract_title(db).to_lowercase();
        if ["expense", "subscription", "budget"].iter().any(|w| title.contains(w)) {
            if let Some(id) = db.get("id").and_then(|i| i.as_str()) {
                return Ok(id.to_string());
            }
        }
    }
    Err("No budget/subscriptions database is shared with the integration. Share it in Notion, or set its id in Settings.".to_string())
}

/// Reads the item name (title property) and monthly amount (a number/formula
/// property, preferring one named like "amount"/"cost"/"price") from each row.
fn parse_subscription(row: &serde_json::Value) -> Option<Subscription> {
    let props = row.get("properties")?.as_object()?;
    let mut name = String::new();
    let mut amount: Option<f64> = None;
    let mut preferred_amount: Option<f64> = None;
    for (key, val) in props {
        match val.get("type").and_then(|t| t.as_str()) {
            Some("title") => {
                name = val
                    .get("title")
                    .and_then(|t| t.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|rt| rt.get("plain_text").and_then(|p| p.as_str()))
                            .collect::<String>()
                    })
                    .unwrap_or_default();
            }
            Some("number") => {
                let n = val.get("number").and_then(|n| n.as_f64());
                let k = key.to_lowercase();
                if k.contains("amount") || k.contains("cost") || k.contains("price") {
                    preferred_amount = n;
                } else if amount.is_none() {
                    amount = n;
                }
            }
            Some("formula") => {
                if amount.is_none() {
                    amount = val
                        .get("formula")
                        .and_then(|f| f.get("number"))
                        .and_then(|n| n.as_f64());
                }
            }
            _ => {}
        }
    }
    if name.trim().is_empty() {
        return None;
    }
    Some(Subscription {
        name,
        amount: preferred_amount.or(amount),
    })
}

pub async fn list_subscriptions_impl() -> Result<Vec<Subscription>, String> {
    let token = notion_token()?;
    let db_id = subscriptions_db_id(&token).await?;
    let resp = client()
        .post(format!("{API}/databases/{db_id}/query"))
        .bearer_auth(&token)
        .header("Notion-Version", NOTION_VERSION)
        .json(&serde_json::json!({ "page_size": 100 }))
        .send()
        .await
        .map_err(|e| format!("Notion unreachable: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Notion {status}: {body}"));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let rows = json
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Unexpected Notion response".to_string())?;
    Ok(rows.iter().filter_map(parse_subscription).collect())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn list_notion_subscriptions() -> Result<Vec<Subscription>, String> {
    list_subscriptions_impl().await
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
