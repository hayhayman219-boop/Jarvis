use serde::Serialize;
use std::sync::Mutex;
use sysinfo::{Components, Disks, System};

pub struct SystemState {
    pub sys: Mutex<System>,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            sys: Mutex::new(System::new_all()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SystemStats {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub disk_percent: f32,
    /// Hottest relevant temperature sensor in °C, when the platform exposes one.
    pub temperature_c: Option<f32>,
}

/// Reads the CPU temperature straight from the kernel thermal zones — more
/// reliable than sysinfo's component list, which often surfaces the wrong
/// sensor (or none). Prefers the package/CPU zone, else the hottest plausible.
fn read_cpu_temp() -> Option<f32> {
    let mut best: Option<f32> = None;
    let mut cpu: Option<f32> = None;
    let entries = std::fs::read_dir("/sys/class/thermal").ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let ty = std::fs::read_to_string(e.path().join("type"))
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if let Ok(raw) = std::fs::read_to_string(e.path().join("temp")) {
            if let Ok(milli) = raw.trim().parse::<f32>() {
                let c = milli / 1000.0;
                if (1.0..130.0).contains(&c) {
                    best = Some(best.map_or(c, |m: f32| m.max(c)));
                    if ty.contains("x86_pkg") || ty.contains("cpu") || ty.contains("coretemp") {
                        cpu = Some(cpu.map_or(c, |m: f32| m.max(c)));
                    }
                }
            }
        }
    }
    cpu.or(best)
}

pub fn get_system_stats_impl(state: &SystemState) -> Result<SystemStats, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    // CPU usage is a delta between two samples — a single refresh reads 0 or
    // stale. Sample, wait the minimum interval, sample again for a real number.
    sys.refresh_cpu_usage();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_percent = sys.global_cpu_usage();
    let memory_percent = if sys.total_memory() > 0 {
        (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
    } else {
        0.0
    };

    // Disk: usage of the root filesystem (fall back to the largest disk).
    let disks = Disks::new_with_refreshed_list();
    let disk_percent = disks
        .list()
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .or_else(|| disks.list().iter().max_by_key(|d| d.total_space()))
        .map(|d| {
            let total = d.total_space();
            if total > 0 {
                ((total - d.available_space()) as f32 / total as f32) * 100.0
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);

    // Temperature: kernel thermal zones first (reliable), sysinfo as fallback.
    let temperature_c = read_cpu_temp().or_else(|| {
        Components::new_with_refreshed_list()
            .list()
            .iter()
            .filter_map(|c| c.temperature())
            .fold(None, |m: Option<f32>, t| Some(m.map_or(t, |mx| mx.max(t))))
    });

    Ok(SystemStats {
        cpu_percent,
        memory_percent,
        disk_percent,
        temperature_c,
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub fn get_system_stats(state: tauri::State<SystemState>) -> Result<SystemStats, String> {
    get_system_stats_impl(&state)
}
