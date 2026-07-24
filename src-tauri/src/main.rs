// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The desktop binary only does anything with the `desktop` feature (the
    // default). Building it `--no-default-features` yields a no-op, so the
    // shared crate can be compiled headless without this bin erroring.
    #[cfg(feature = "desktop")]
    jarvis_lib::run();
}
