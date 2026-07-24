//! Chrome / browser control: open the news (or arbitrary web pages) in Chrome
//! on command. Distinct from `news.rs`, which fetches RSS headlines into the
//! chat context — this actually launches the browser to the sites.

use std::process::{Command, Stdio};

/// Chrome first (the user's stated preference), then Chromium, then the system
/// default via xdg-open.
#[cfg(target_os = "linux")]
const CHROME_BINS: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "chromium",
    "chromium-browser",
];

const NEWS_DEFAULT: &[&str] = &[
    "https://news.google.com/",
    "https://www.theverge.com/",
    "https://arstechnica.com/",
];

fn is_web_url(u: &str) -> bool {
    u.starts_with("http://") || u.starts_with("https://")
}

#[cfg(target_os = "linux")]
fn which(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn open_urls_impl(urls: Vec<String>) -> Result<String, String> {
    if urls.is_empty() {
        return Err("No URLs to open.".to_string());
    }
    // Only ever hand http(s) URLs to the browser — since these can arrive from
    // loosely-parsed voice commands, refuse anything else so this can't be
    // turned into a way to launch local files or other schemes.
    for u in &urls {
        if !is_web_url(u) {
            return Err(format!("Refusing to open a non-web URL: {u}"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Chrome/Chromium opens every URL as tabs in one window when passed together.
        for bin in CHROME_BINS {
            if which(bin)
                && Command::new(bin)
                    .args(&urls)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .is_ok()
            {
                return Ok(format!("Opened {} tab(s) in Chrome.", urls.len()));
            }
        }
        let opened = urls
            .iter()
            .filter(|u| Command::new("xdg-open").arg(u).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok())
            .count();
        return if opened > 0 {
            Ok(format!("Opened {opened} link(s) in your browser."))
        } else {
            Err("Couldn't find Chrome or a default browser to open.".to_string())
        };
    }
    #[cfg(target_os = "macos")]
    {
        if Command::new("open").arg("-a").arg("Google Chrome").args(&urls).spawn().is_ok() {
            return Ok(format!("Opened {} tab(s) in Chrome.", urls.len()));
        }
        let opened = urls.iter().filter(|u| Command::new("open").arg(u).spawn().is_ok()).count();
        return if opened > 0 {
            Ok(format!("Opened {opened} link(s) in your browser."))
        } else {
            Err("Couldn't open a browser.".to_string())
        };
    }
    #[cfg(target_os = "windows")]
    {
        // `start` is a cmd builtin; the empty "" is the required window title.
        let opened = urls
            .iter()
            .filter(|u| {
                Command::new("cmd").args(["/C", "start", "chrome", u]).spawn().is_ok()
                    || Command::new("cmd").args(["/C", "start", "", u]).spawn().is_ok()
            })
            .count();
        return if opened > 0 {
            Ok(format!("Opened {opened} link(s) in your browser."))
        } else {
            Err("Couldn't open a browser.".to_string())
        };
    }
    #[allow(unreachable_code)]
    Err("Opening a browser isn't supported on this platform.".to_string())
}

fn news_urls(category: Option<&str>) -> Vec<String> {
    let list: Vec<&str> = match category {
        Some("pokemon") => vec!["https://www.serebii.net/", "https://nintendoeverything.com/"],
        Some("ai") => vec![
            "https://news.google.com/search?q=artificial%20intelligence",
            "https://arstechnica.com/ai/",
        ],
        Some("datacenter") => vec![
            "https://www.datacenterdynamics.com/en/",
            "https://news.google.com/search?q=data%20center",
        ],
        _ => NEWS_DEFAULT.to_vec(),
    };
    list.into_iter().map(|s| s.to_string()).collect()
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Maps a spoken site name to a URL. Covers the common ones; anything else
/// falls through to domain-detection / search in `open_web_impl`.
fn site_alias(name: &str) -> Option<&'static str> {
    let n = name.trim().to_lowercase();
    let n = n.trim_start_matches("the ").trim();
    Some(match n {
        "youtube" | "yt" => "https://www.youtube.com/",
        "gmail" | "email" | "my email" | "mail" => "https://mail.google.com/",
        "github" => "https://github.com/",
        "reddit" => "https://www.reddit.com/",
        "twitter" | "x" => "https://x.com/",
        "amazon" => "https://www.amazon.com/",
        "netflix" => "https://www.netflix.com/",
        "twitch" => "https://www.twitch.tv/",
        "spotify" => "https://open.spotify.com/",
        "discord" => "https://discord.com/app",
        "wikipedia" => "https://www.wikipedia.org/",
        "chatgpt" | "chat gpt" => "https://chatgpt.com/",
        "maps" | "google maps" => "https://maps.google.com/",
        "drive" | "google drive" => "https://drive.google.com/",
        "calendar" | "google calendar" => "https://calendar.google.com/",
        "docs" | "google docs" => "https://docs.google.com/",
        "facebook" => "https://www.facebook.com/",
        "instagram" | "insta" => "https://www.instagram.com/",
        "linkedin" => "https://www.linkedin.com/",
        "ebay" => "https://www.ebay.com/",
        "walmart" => "https://www.walmart.com/",
        "steam" => "https://store.steampowered.com/",
        "gmail.com" => "https://mail.google.com/",
        _ => return None,
    })
}

fn looks_like_domain(t: &str) -> bool {
    !t.contains(' ')
        && [".com", ".org", ".net", ".io", ".dev", ".gov", ".edu", ".co", ".tv", ".app", ".ai"]
            .iter()
            .any(|tld| t.to_lowercase().contains(tld))
}

/// Fuller Chrome control: search the web, open a known site by name, go to a
/// URL, or (fallback) search for whatever was said.
pub fn open_web_impl(target: &str, force_search: bool) -> Result<String, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("Nothing to open.".to_string());
    }
    if force_search {
        open_urls_impl(vec![format!("https://www.google.com/search?q={}", urlencode(t))])?;
        return Ok(format!("Searching for \"{t}\"."));
    }
    if let Some(url) = site_alias(t) {
        open_urls_impl(vec![url.to_string()])?;
        return Ok(format!("Opening {t}."));
    }
    if looks_like_domain(t) {
        let url = if t.starts_with("http") { t.to_string() } else { format!("https://{t}") };
        open_urls_impl(vec![url])?;
        return Ok(format!("Opening {t}."));
    }
    // Not a known site or URL — search for it.
    open_urls_impl(vec![format!("https://www.google.com/search?q={}", urlencode(t))])?;
    Ok(format!("Searching for \"{t}\"."))
}

pub fn open_news_impl(category: Option<String>) -> Result<String, String> {
    open_urls_impl(news_urls(category.as_deref()))?;
    let label = category
        .as_deref()
        .map(|c| format!("{c} news"))
        .unwrap_or_else(|| "the news".to_string());
    Ok(format!("Opening {label} in Chrome."))
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn open_urls(urls: Vec<String>) -> Result<String, String> {
    open_urls_impl(urls)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn open_news(category: Option<String>) -> Result<String, String> {
    open_news_impl(category)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn open_web(target: String, search: bool) -> Result<String, String> {
    open_web_impl(&target, search)
}
