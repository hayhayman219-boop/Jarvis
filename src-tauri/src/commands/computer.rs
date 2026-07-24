//! Computer control: lets Jarvis open/close applications, run arbitrary
//! shell commands, and list files on this machine. Unlike the deliberately
//! conservative Home Assistant bridge, this is full-power by design — the
//! user explicitly wants unrestricted shell access here. To offset that,
//! it's Tauri-only: there is no route for this in `bin/server/main.rs`, so
//! it's unreachable from the web/phone interface, which has no auth yet.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use sysinfo::System;
use tokio::process::Command;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const OUTPUT_LIMIT: usize = 4000;

fn truncate(mut s: String) -> String {
    if s.len() > OUTPUT_LIMIT {
        s.truncate(OUTPUT_LIMIT);
        s.push_str("\n...[truncated]");
    }
    s
}

fn expand_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = trimmed.strip_prefix("~/") {
            return PathBuf::from(home).join(rest);
        }
        if trimmed == "~" {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(trimmed)
}

/// Spoken/friendly app names rarely match the actual Linux binary
/// ("Chrome" -> `google-chrome`, "vs code" -> `code`), so a bare
/// `Command::new("Chrome")` fails with "not found" even when it's installed.
/// Maps common friendly names to their real executables; the first candidate
/// that exists on PATH wins. Extend as new apps come up.
fn app_aliases(lower: &str) -> &'static [&'static str] {
    match lower {
        "chrome" | "google chrome" => &["google-chrome-stable", "google-chrome"],
        "chromium" => &["chromium", "chromium-browser"],
        "firefox" => &["firefox", "firefox-esr"],
        "brave" => &["brave-browser", "brave"],
        "edge" | "microsoft edge" => &["microsoft-edge-stable", "microsoft-edge"],
        "vs code" | "vscode" | "code" | "visual studio code" => &["code"],
        "terminal" => &["gnome-terminal", "konsole", "kitty", "alacritty", "x-terminal-emulator"],
        "files" | "file manager" => &["nautilus", "dolphin", "thunar", "nemo"],
        "calculator" => &["gnome-calculator", "kcalc"],
        "spotify" => &["spotify"],
        "discord" => &["discord"],
        "slack" => &["slack"],
        _ => &[],
    }
}

/// Locates an executable on PATH by name, returning its full path if found.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Resolves a friendly/spoken app name to a real, PATH-resolvable binary,
/// trying (in order): the name verbatim, its lowercase form, and any known
/// aliases. Returns the binary that will actually launch.
fn resolve_app_binary(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let mut candidates: Vec<String> = vec![name.to_string(), lower.clone()];
    candidates.extend(app_aliases(&lower).iter().map(|s| s.to_string()));
    candidates
        .into_iter()
        .find(|c| find_on_path(c).is_some())
}

/// Launches the app directly as a command (how most Linux apps are started
/// from a terminal/launcher) rather than through a shell, so spaces or
/// punctuation in the name can't be misread as shell syntax. Resolves
/// friendly names ("Chrome") to their real binaries first.
pub async fn open_app_impl(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("No application name given.".to_string());
    }
    let binary = resolve_app_binary(name)
        .ok_or_else(|| format!("Couldn't find an app named '{name}' installed on this machine."))?;
    Command::new(&binary)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Couldn't launch '{name}' ({binary}): {e}"))?;
    Ok(format!("Opened {name}."))
}

pub fn close_app_impl(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("No application name given.".to_string());
    }
    let lower = name.to_lowercase();
    let sys = System::new_all();
    let matches: Vec<_> = sys
        .processes()
        .values()
        .filter(|p| p.name().to_string_lossy().to_lowercase().contains(&lower))
        .collect();
    if matches.is_empty() {
        return Ok(format!("No running process matched '{name}'."));
    }
    let count = matches.len();
    for p in matches {
        p.kill();
    }
    Ok(format!("Closed {count} process(es) matching '{name}'."))
}

pub async fn run_command_impl(command: &str) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("No command given.".to_string());
    }
    #[cfg(not(target_os = "windows"))]
    let mut shell = {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    #[cfg(target_os = "windows")]
    let mut shell = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    };
    let output = tokio::time::timeout(COMMAND_TIMEOUT, shell.output())
    .await
    .map_err(|_| format!("Command timed out after {}s.", COMMAND_TIMEOUT.as_secs()))?
    .map_err(|e| format!("Failed to run command: {e}"))?;

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    if combined.trim().is_empty() {
        combined = format!(
            "(no output, exit code {})",
            output.status.code().unwrap_or(-1)
        );
    }
    Ok(truncate(combined))
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

pub fn list_files_impl(path: &str) -> Result<Vec<FileEntry>, String> {
    let dir = expand_path(path);
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("Couldn't read '{}': {e}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        files.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: meta.is_dir(),
            size: meta.len(),
        });
    }
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(files)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn open_app(name: String) -> Result<String, String> {
    open_app_impl(&name).await
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn close_app(name: String) -> Result<String, String> {
    close_app_impl(&name)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn run_command(command: String) -> Result<String, String> {
    run_command_impl(&command).await
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn list_files(path: String) -> Result<Vec<FileEntry>, String> {
    list_files_impl(&path)
}
