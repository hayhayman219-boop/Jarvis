//! On-demand news/leak headlines by category, sourced from public RSS/Atom
//! feeds — no API keys, nothing paid, in keeping with the local-first setup.
//! Results are cached for 10 minutes per category, both to be polite to the
//! feed hosts and because Reddit's RSS endpoints rate-limit aggressively
//! when hit in quick succession.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;

const CACHE_TTL: Duration = Duration::from_secs(600);
const PER_FEED_TIMEOUT: Duration = Duration::from_secs(8);
const ITEMS_PER_FEED: usize = 6;
const ITEMS_PER_CATEGORY: usize = 10;
// Some feed hosts 403 the default reqwest UA.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) Jarvis-personal-assistant/0.1";

/// Feed registry. URLs verified reachable + parseable at build time
/// (Serebii and PokéBeach were tried and are gone / bot-walled).
fn feeds_for(category: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match category {
        "pokemon" => Some(&[
            ("r/PokeLeaks", "https://www.reddit.com/r/PokeLeaks/.rss"),
            ("r/stunfisk", "https://www.reddit.com/r/stunfisk/.rss"),
            ("Nintendo Life", "https://www.nintendolife.com/feeds/news"),
        ]),
        "tech" => Some(&[
            ("The Verge", "https://www.theverge.com/rss/index.xml"),
            (
                "Ars Technica",
                "https://feeds.arstechnica.com/arstechnica/index",
            ),
        ]),
        "ai" => Some(&[
            ("Ars Technica AI", "https://arstechnica.com/ai/feed/"),
            (
                "VentureBeat AI",
                "https://venturebeat.com/category/ai/feed/",
            ),
            (
                "The Verge AI",
                "https://www.theverge.com/rss/ai-artificial-intelligence/index.xml",
            ),
        ]),
        "datacenter" => Some(&[
            (
                "Data Center Dynamics",
                "https://www.datacenterdynamics.com/rss/",
            ),
            (
                "Data Center Knowledge",
                "https://www.datacenterknowledge.com/rss.xml",
            ),
        ]),
        _ => None,
    }
}

pub const CATEGORIES: &[&str] = &["pokemon", "tech", "ai", "datacenter"];

#[derive(Debug, Clone, Serialize)]
pub struct NewsItem {
    pub source: String,
    pub title: String,
    pub url: String,
    /// RFC3339, when the feed provided one.
    pub published: Option<String>,
}

#[derive(Default)]
pub struct NewsCache {
    entries: Mutex<HashMap<String, (Instant, Vec<NewsItem>)>>,
}

impl NewsCache {
    pub fn new() -> Self {
        Self::default()
    }
}

async fn fetch_one_feed(source: &str, url: &str) -> Result<Vec<NewsItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(PER_FEED_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("{source}: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("{source}: {e}"))?;
    let feed = feed_rs::parser::parse(&bytes[..]).map_err(|e| format!("{source}: {e}"))?;

    Ok(feed
        .entries
        .into_iter()
        .take(ITEMS_PER_FEED)
        .filter_map(|entry| {
            let title = entry.title.map(|t| t.content.trim().to_string())?;
            let url = entry
                .links
                .first()
                .map(|l| l.href.clone())
                .unwrap_or_default();
            let published: Option<DateTime<Utc>> = entry.published.or(entry.updated);
            Some(NewsItem {
                source: source.to_string(),
                title,
                url,
                published: published.map(|d| d.to_rfc3339()),
            })
        })
        .collect())
}

pub async fn get_news_impl(cache: &NewsCache, category: String) -> Result<Vec<NewsItem>, String> {
    let feeds = feeds_for(&category)
        .ok_or_else(|| format!("Unknown news category '{category}' (known: {CATEGORIES:?})"))?;

    if let Some((fetched_at, items)) = cache.entries.lock().unwrap().get(&category) {
        if fetched_at.elapsed() < CACHE_TTL {
            return Ok(items.clone());
        }
    }

    let mut items: Vec<NewsItem> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    // Sequential on purpose: Reddit 429s parallel bursts from the same IP.
    for (source, url) in feeds {
        match fetch_one_feed(source, url).await {
            Ok(mut feed_items) => items.append(&mut feed_items),
            Err(e) => errors.push(e),
        }
    }
    if items.is_empty() {
        return Err(format!(
            "All feeds failed for '{category}': {}",
            errors.join("; ")
        ));
    }

    // Newest first; undated items sink to the end.
    items.sort_by(|a, b| b.published.cmp(&a.published));
    items.truncate(ITEMS_PER_CATEGORY);

    cache
        .entries
        .lock()
        .unwrap()
        .insert(category, (Instant::now(), items.clone()));
    Ok(items)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn get_news(
    cache: tauri::State<'_, NewsCache>,
    category: String,
) -> Result<Vec<NewsItem>, String> {
    get_news_impl(&cache, category).await
}
