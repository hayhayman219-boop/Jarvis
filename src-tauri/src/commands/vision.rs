//! Screen OCR: capture the screen and run it through Tesseract so Jarvis can
//! "read" what's on the display and answer questions about it ("what does this
//! error say?"). Desktop-only; uses the same screenshot tooling as desktop.rs.

use std::path::PathBuf;
use std::process::Command;

/// Captures the current screen to a temp PNG and returns its path.
#[cfg(target_os = "linux")]
fn capture_screen() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("jarvis-ocr");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = Command::new("cosmic-screenshot")
        .args([
            "--interactive=false",
            "--notify=false",
            &format!("--save-dir={}", dir.display()),
        ])
        .output()
        .map_err(|e| format!("screenshot failed (is cosmic-screenshot installed?): {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "screenshot failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !printed.is_empty() && PathBuf::from(&printed).exists() {
        return Ok(PathBuf::from(printed));
    }
    // Fall back to the newest PNG in the temp dir.
    newest_png(&dir).ok_or_else(|| "screenshot produced no file".to_string())
}

#[cfg(target_os = "macos")]
fn capture_screen() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("jarvis-ocr");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("shot_{ts}.png"));
    let status = Command::new("screencapture")
        .args(["-x", &path.to_string_lossy()])
        .status()
        .map_err(|e| format!("screencapture failed: {e}"))?;
    if status.success() && path.exists() {
        Ok(path)
    } else {
        Err("screencapture produced no file".to_string())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn capture_screen() -> Result<PathBuf, String> {
    Err("Screen capture isn't wired up on this platform yet.".to_string())
}

#[cfg(target_os = "linux")]
fn newest_png(dir: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "png").unwrap_or(false))
        .max_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
        })
}

/// Screenshots the screen and OCRs it, returning the recognized text.
pub fn read_screen_impl() -> Result<String, String> {
    let shot = capture_screen()?;

    // `tesseract <image> stdout` prints recognized text to stdout.
    let out = Command::new("tesseract")
        .args([&shot.to_string_lossy().to_string(), "stdout", "-l", "eng"])
        .output();
    let _ = std::fs::remove_file(&shot);

    let out = out.map_err(|_| {
        "Screen OCR needs Tesseract. Install it with: sudo apt install tesseract-ocr".to_string()
    })?;
    if !out.status.success() {
        return Err(format!(
            "OCR failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Collapse the ragged whitespace OCR emits into tidy lines.
    let cleaned: String = text
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if cleaned.trim().is_empty() {
        Err("I captured the screen but couldn't read any text on it.".to_string())
    } else {
        Ok(cleaned)
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn read_screen() -> Result<String, String> {
    read_screen_impl()
}
