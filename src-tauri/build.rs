fn main() {
    // Only run Tauri's build step for the desktop build. The headless server
    // (`--no-default-features`) has no Tauri dependency, so this would be
    // pointless there.
    if std::env::var("CARGO_FEATURE_DESKTOP").is_ok() {
        tauri_build::build();
    }
}
