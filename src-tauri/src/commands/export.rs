//! Export a chat reply (Markdown) to a nicely formatted PDF in the user's
//! Downloads folder. Renders via headless Chrome (already the user's browser),
//! so there are no heavy PDF/LaTeX Rust dependencies to carry onto the phone
//! node — this whole module is desktop-only anyway.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
const CHROME_BINS: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "chromium",
    "chromium-browser",
];
#[cfg(target_os = "macos")]
const CHROME_BINS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
];
#[cfg(target_os = "windows")]
const CHROME_BINS: &[&str] = &[
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn downloads_dir() -> PathBuf {
    let base = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dl = base.join("Downloads");
    if dl.is_dir() {
        dl
    } else {
        base
    }
}

/// Turns a title into a safe, readable file stem: keeps alphanumerics, spaces,
/// dashes and underscores; collapses the rest to a dash.
fn safe_stem(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    s.truncate(80);
    let s = s.trim().to_string();
    if s.is_empty() {
        "Jarvis Export".to_string()
    } else {
        s
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Minimal, self-contained Markdown -> HTML for the presentation replies Sub
/// AIs produce: headings (#, ##, ###), bullet lists (-/*), **bold**, and
/// [label](url) links. Anything fancier degrades gracefully to plain text.
fn markdown_to_html(md: &str) -> String {
    fn inline(text: &str) -> String {
        let mut out = String::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < text.len() {
            // [label](url)
            if bytes[i] == b'[' {
                if let Some(rc) = text[i + 1..].find(']') {
                    let label_end = i + 1 + rc;
                    let after = label_end + 1;
                    if after < text.len() && bytes[after] == b'(' {
                        if let Some(rp) = text[after + 1..].find(')') {
                            let label = &text[i + 1..label_end];
                            let url = &text[after + 1..after + 1 + rp];
                            out.push_str(&format!(
                                "<a href=\"{}\">{}</a>",
                                html_escape(url),
                                html_escape(label)
                            ));
                            i = after + 1 + rp + 1;
                            continue;
                        }
                    }
                }
            }
            // **bold**
            if text[i..].starts_with("**") {
                if let Some(rc) = text[i + 2..].find("**") {
                    let inner = &text[i + 2..i + 2 + rc];
                    out.push_str(&format!("<strong>{}</strong>", html_escape(inner)));
                    i = i + 2 + rc + 2;
                    continue;
                }
            }
            let ch = text[i..].chars().next().unwrap();
            out.push_str(&html_escape(&ch.to_string()));
            i += ch.len_utf8();
        }
        out
    }

    let mut body = String::new();
    let mut in_list = false;
    let close_list = |body: &mut String, in_list: &mut bool| {
        if *in_list {
            body.push_str("</ul>\n");
            *in_list = false;
        }
    };

    for raw in md.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            close_list(&mut body, &mut in_list);
            body.push_str(&format!("<h3>{}</h3>\n", inline(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            close_list(&mut body, &mut in_list);
            body.push_str(&format!("<h2>{}</h2>\n", inline(rest)));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            close_list(&mut body, &mut in_list);
            body.push_str(&format!("<h1>{}</h1>\n", inline(rest)));
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if !in_list {
                body.push_str("<ul>\n");
                in_list = true;
            }
            body.push_str(&format!("<li>{}</li>\n", inline(rest)));
        } else if trimmed.is_empty() {
            close_list(&mut body, &mut in_list);
        } else {
            close_list(&mut body, &mut in_list);
            body.push_str(&format!("<p>{}</p>\n", inline(trimmed)));
        }
    }
    close_list(&mut body, &mut in_list);
    body
}

fn wrap_html(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{t}</title>\
<style>\
@page {{ margin: 18mm; }}\
body {{ font-family: -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; \
color: #14181d; line-height: 1.5; font-size: 12pt; }}\
h1 {{ font-size: 22pt; border-bottom: 2px solid #2fd4ff; padding-bottom: 6px; }}\
h2 {{ font-size: 16pt; color: #0a6c86; margin-top: 20px; }}\
h3 {{ font-size: 13pt; color: #333; }}\
a {{ color: #0a6c86; text-decoration: none; }}\
ul {{ margin: 6px 0 6px 18px; }} li {{ margin: 3px 0; }}\
p {{ margin: 8px 0; }}\
.jarvis-title {{ color:#888; font-size: 9pt; letter-spacing: 1px; text-transform: uppercase; }}\
</style></head><body><div class=\"jarvis-title\">{t}</div>{b}</body></html>",
        t = html_escape(title),
        b = body,
    )
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

fn run_chrome_print(html_path: &Path, pdf_path: &Path) -> Result<(), String> {
    let file_url = format!("file://{}", html_path.to_string_lossy());
    let pdf_arg = format!("--print-to-pdf={}", pdf_path.to_string_lossy());
    let args = [
        "--headless=new",
        "--disable-gpu",
        "--no-pdf-header-footer",
        "--no-margins",
        "--virtual-time-budget=2000",
        &pdf_arg,
        &file_url,
    ];

    let mut last_err = "No Chrome/Chromium binary found to render the PDF.".to_string();
    for bin in CHROME_BINS {
        #[cfg(target_os = "linux")]
        if !which(bin) {
            continue;
        }
        #[cfg(not(target_os = "linux"))]
        if !std::path::Path::new(bin).exists() {
            continue;
        }
        match Command::new(bin)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() && pdf_path.exists() => return Ok(()),
            Ok(status) => last_err = format!("{bin} exited with {status} without producing a PDF"),
            Err(e) => last_err = format!("{bin} failed to start: {e}"),
        }
    }
    Err(last_err)
}

/// Renders `markdown` to a PDF titled `title` in the Downloads folder,
/// returning the saved path. `title` also seeds the filename.
pub fn export_markdown_pdf_impl(title: String, markdown: String) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("There's nothing to export yet.".to_string());
    }
    let html = wrap_html(&title, &markdown_to_html(&markdown));

    // Temp HTML source for Chrome to render.
    let pid = std::process::id();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let html_path = std::env::temp_dir().join(format!("jarvis-export-{pid}-{stamp}.html"));
    std::fs::write(&html_path, html).map_err(|e| format!("Failed to stage HTML: {e}"))?;

    // Unique, non-clobbering destination in Downloads.
    let dir = downloads_dir();
    let stem = safe_stem(&title);
    let mut pdf_path = dir.join(format!("{stem}.pdf"));
    let mut n = 2;
    while pdf_path.exists() {
        pdf_path = dir.join(format!("{stem} ({n}).pdf"));
        n += 1;
    }

    let result = run_chrome_print(&html_path, &pdf_path);
    let _ = std::fs::remove_file(&html_path);
    result?;

    Ok(pdf_path.to_string_lossy().to_string())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn export_pdf(title: String, content: String) -> Result<String, String> {
    export_markdown_pdf_impl(title, content)
}
