//! Desktop control: volume, brightness, screen lock, and screenshots by voice.
//! Targets this machine's stack — PipeWire (`wpctl`), systemd-logind for
//! backlight + lock (no root needed for the active session), and COSMIC's
//! `cosmic-screenshot`.

use std::process::Command;

fn run(bin: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("{bin} failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(target_os = "linux")]
const SINK: &str = "@DEFAULT_AUDIO_SINK@";

#[cfg(target_os = "linux")]
fn current_volume() -> f32 {
    // `wpctl get-volume` prints e.g. "Volume: 0.65" (or "... [MUTED]").
    Command::new("wpctl")
        .args(["get-volume", SINK])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(0.0)
}

pub fn set_volume_impl(action: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        match action {
            "mute" => {
                run("wpctl", &["set-mute", SINK, "1"])?;
                return Ok("Muted.".to_string());
            }
            "unmute" => run("wpctl", &["set-mute", SINK, "0"])?,
            "up" => {
                let v = (current_volume() + 0.1).min(1.0);
                run("wpctl", &["set-volume", SINK, &format!("{v:.2}")])?;
            }
            "down" => {
                let v = (current_volume() - 0.1).max(0.0);
                run("wpctl", &["set-volume", SINK, &format!("{v:.2}")])?;
            }
            _ => {
                let pct: f32 = action
                    .parse()
                    .map_err(|_| format!("Unknown volume action: {action}"))?;
                let v = (pct / 100.0).clamp(0.0, 1.0);
                let _ = run("wpctl", &["set-mute", SINK, "0"]);
                run("wpctl", &["set-volume", SINK, &format!("{v:.2}")])?;
            }
        }
        return Ok(format!(
            "Volume at {}%.",
            (current_volume() * 100.0).round() as i32
        ));
    }
    #[cfg(target_os = "macos")]
    {
        let script = match action {
            "mute" => "set volume with output muted".to_string(),
            "unmute" => "set volume without output muted".to_string(),
            "up" => "set volume output volume ((output volume of (get volume settings)) + 10)".to_string(),
            "down" => "set volume output volume ((output volume of (get volume settings)) - 10)".to_string(),
            _ => {
                let pct: i32 = action
                    .parse()
                    .map_err(|_| format!("Unknown volume action: {action}"))?;
                format!("set volume output volume {}", pct.clamp(0, 100))
            }
        };
        run("osascript", &["-e", &script])?;
        return Ok("Volume adjusted.".to_string());
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = action;
    #[allow(unreachable_code)]
    Err("Volume control isn't wired up on this platform yet.".to_string())
}

#[cfg(target_os = "linux")]
fn backlight_device() -> Option<String> {
    std::fs::read_dir("/sys/class/backlight")
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .next()
}

pub fn set_brightness_impl(action: &str) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
    let dev = backlight_device().ok_or_else(|| "No backlight device found.".to_string())?;
    let base = format!("/sys/class/backlight/{dev}");
    let max: u64 = std::fs::read_to_string(format!("{base}/max_brightness"))
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "unreadable max_brightness".to_string())?;
    let cur: u64 = std::fs::read_to_string(format!("{base}/brightness"))
        .map_err(|e| e.to_string())?
        .trim()
        .parse()
        .map_err(|_| "unreadable brightness".to_string())?;
    let step = (max / 10).max(1);
    let floor = (max / 100).max(1); // never let voice control blank the screen
    let new = match action {
        "up" => (cur + step).min(max),
        "down" => cur.saturating_sub(step).max(floor),
        _ => {
            let pct: u64 = action
                .parse()
                .map_err(|_| format!("Unknown brightness action: {action}"))?;
            ((max * pct.min(100)) / 100).max(floor)
        }
    };
    // logind lets the active session set its own backlight without root.
    run(
        "busctl",
        &[
            "call",
            "org.freedesktop.login1",
            "/org/freedesktop/login1/session/auto",
            "org.freedesktop.login1.Session",
            "SetBrightness",
            "ssu",
            "backlight",
            &dev,
            &new.to_string(),
        ],
    )?;
    return Ok(format!("Brightness set to {}%.", new * 100 / max));
    }
    #[cfg(not(target_os = "linux"))]
    let _ = action;
    #[allow(unreachable_code)]
    Err("Brightness control isn't wired up on this platform yet.".to_string())
}

pub fn lock_screen_impl() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        // Ask logind to lock the active session (COSMIC honors the Lock signal).
        if run(
            "busctl",
            &[
                "call",
                "org.freedesktop.login1",
                "/org/freedesktop/login1/session/auto",
                "org.freedesktop.login1.Session",
                "Lock",
            ],
        )
        .is_ok()
        {
            return Ok("Screen locked.".to_string());
        }
        run("loginctl", &["lock-session"])?;
        return Ok("Screen locked.".to_string());
    }
    #[cfg(target_os = "macos")]
    {
        run(
            "/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession",
            &["-suspend"],
        )?;
        return Ok("Screen locked.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        run("rundll32.exe", &["user32.dll,LockWorkStation"])?;
        return Ok("Screen locked.".to_string());
    }
    #[allow(unreachable_code)]
    Err("Locking the screen isn't supported on this platform.".to_string())
}

pub fn take_screenshot_impl() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let dir = format!("{home}/Pictures/Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let out = Command::new("cosmic-screenshot")
            .args(["--interactive=false", "--notify=true", &format!("--save-dir={dir}")])
            .output()
            .map_err(|e| format!("screenshot failed: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "screenshot: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return Ok(if stdout.is_empty() {
            format!("Screenshot saved to {dir}.")
        } else {
            format!("Screenshot saved: {stdout}")
        });
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        let dir = format!("{home}/Pictures/Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = format!("{dir}/Screenshot_{ts}.png");
        run("screencapture", &["-x", &path])?;
        return Ok(format!("Screenshot saved: {path}"));
    }
    #[allow(unreachable_code)]
    Err("Screenshots aren't wired up on this platform yet.".to_string())
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn set_volume(action: String) -> Result<String, String> {
    set_volume_impl(&action)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn set_brightness(action: String) -> Result<String, String> {
    set_brightness_impl(&action)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn lock_screen() -> Result<String, String> {
    lock_screen_impl()
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn take_screenshot() -> Result<String, String> {
    take_screenshot_impl()
}
