pub mod commands;
pub mod db;
#[cfg(feature = "desktop")]
pub mod news_monitor;
#[cfg(feature = "desktop")]
pub mod scheduler;
pub mod state;
#[cfg(feature = "desktop")]
pub mod voice_loop;

// Everything below is used only by the Tauri desktop `run()` entry point, so
// it's all gated behind `desktop`. The headless server binary reuses the
// `*_impl` functions and the plain structs directly, none of which need Tauri.
#[cfg(feature = "desktop")]
use commands::news::NewsCache;
#[cfg(feature = "desktop")]
use commands::system::SystemState;
#[cfg(feature = "desktop")]
use commands::voice::{self, VoiceState};
#[cfg(feature = "desktop")]
use commands::{
    apple, browser, cart, comics, computer, desktop, export, google, home, news, notion, ollama,
    reminders, system, vision, weather,
};
#[cfg(feature = "desktop")]
use state::AppState;
#[cfg(feature = "desktop")]
use tauri::Manager;

/// Resolves a resource path relative to this crate's source directory.
/// Used directly by the standalone HTTP server (`bin/server.rs`, which has
/// no Tauri `AppHandle`), and as the dev-mode fallback for the Tauri app
/// below (bundle.resources aren't always resolved via `resource_dir()`
/// while running `tauri dev`).
pub fn resolve_manifest_resource_path(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[cfg(feature = "desktop")]
fn resolve_resource_path(app: &tauri::AppHandle, relative: &str) -> std::path::PathBuf {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let candidate = resource_dir.join(relative);
        if candidate.exists() {
            return candidate;
        }
    }
    resolve_manifest_resource_path(relative)
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Registered first (per plugin docs): launching Jarvis while an
        // instance is already running focuses the existing window instead of
        // starting a duplicate — duplicates fight over the microphone and
        // led to a "deaf" second window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        // Hide-on-close instead of destroy: Jarvis is a persistent background
        // service (always-on wake word + double-clap listener), so closing the
        // window should tuck it away, not tear down the webview. Keeping the
        // window alive is what lets a double-clap (or "Hey Jarvis") bring it
        // back — otherwise there's nothing to re-show.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // WebKitGTK (the Linux webview) denies getUserMedia by default, so
            // the in-app webcam/mic-capture screens fail with "not allowed".
            // Reach into the native webview to enable media streams and auto-
            // grant permission requests. No-op / not compiled on other OSes.
            #[cfg(target_os = "linux")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.with_webview(|webview| {
                    use webkit2gtk::{
                        PermissionRequestExt, SettingsExt, WebViewExt,
                    };
                    let wv = webview.inner();
                    if let Some(settings) = WebViewExt::settings(&wv) {
                        settings.set_enable_media_stream(true);
                        settings.set_enable_mediasource(true);
                    }
                    wv.connect_permission_request(|_wv, req| {
                        req.allow();
                        true
                    });
                });
            }

            let app_data_dir = app.path().app_data_dir()?;
            let conn = db::init_db(app_data_dir)?;
            app.manage(AppState {
                db: std::sync::Mutex::new(conn),
            });
            scheduler::start(app.handle().clone());
            news_monitor::start(app.handle().clone());
            app.manage(SystemState::new());
            app.manage(NewsCache::new());

            #[cfg(feature = "desktop")]
            {
            // Command transcription: prefer the most accurate model that's
            // actually been downloaded. large-v3-turbo is dramatically better
            // than small.en (still CPU-runnable); small.en and base.en are
            // the lighter fallbacks while the ~1.6GB turbo model downloads.
            let whisper_model_path = [
                "resources/whisper/ggml-large-v3-turbo.bin",
                "resources/whisper/ggml-small.en.bin",
                "resources/whisper/ggml-base.en.bin",
            ]
            .iter()
            .map(|rel| resolve_resource_path(app.handle(), rel))
            .find(|p| p.exists())
            .unwrap_or_else(|| {
                resolve_resource_path(app.handle(), "resources/whisper/ggml-base.en.bin")
            });
            eprintln!("[whisper] command model: {whisper_model_path:?}");
            // Wake/stop model: base.en is far more reliable than tiny.en at
            // hearing "Jarvis" (tiny mangles it), while still fast enough to run
            // continuously in the background. Falls back to tiny.en if absent.
            let whisper_fast_model_path = [
                "resources/whisper/ggml-base.en.bin",
                "resources/whisper/ggml-tiny.en.bin",
            ]
            .iter()
            .map(|rel| resolve_resource_path(app.handle(), rel))
            .find(|p| p.exists())
            .unwrap_or_else(|| {
                resolve_resource_path(app.handle(), "resources/whisper/ggml-tiny.en.bin")
            });
            eprintln!("[whisper] wake model: {whisper_fast_model_path:?}");
            let piper_bin = resolve_resource_path(app.handle(), "resources/piper/piper");
            let piper_lib_dir = resolve_resource_path(app.handle(), "resources/piper");
            let piper_voice = resolve_resource_path(
                app.handle(),
                "resources/piper/voices/en_GB-jenny_dioco-medium.onnx",
            );
            match VoiceState::init(
                whisper_model_path,
                whisper_fast_model_path,
                piper_bin,
                piper_lib_dir,
                piper_voice,
                Some(app.handle().clone()),
            ) {
                Ok(voice_state) => {
                    app.manage(voice_state);
                    app.manage(voice_loop::VoiceLoopControl::new());
                    voice_loop::start(app.handle().clone());
                }
                Err(e) => {
                    eprintln!("Voice features disabled: {e}");
                }
            }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ollama::list_models,
            ollama::chat,
            #[cfg(feature = "desktop")]
            ollama::chat_stream,
            weather::get_weather,
            weather::geocode_city,
            reminders::create_reminder,
            reminders::list_reminders,
            reminders::delete_reminder,
            reminders::parse_and_create_reminder,
            #[cfg(feature = "desktop")]
            voice::start_recording,
            #[cfg(feature = "desktop")]
            voice::stop_recording,
            #[cfg(feature = "desktop")]
            voice::transcribe,
            #[cfg(feature = "desktop")]
            voice::speak,
            #[cfg(feature = "desktop")]
            voice::stop_speaking,
            system::get_system_stats,
            news::get_news,
            home::home_command,
            home::list_home_entities,
            notion::list_notion_pages,
            notion::read_notion_page,
            apple::list_apple_events,
            apple::list_apple_reminders,
            google::list_google_events,
            browser::open_urls,
            browser::open_news,
            browser::open_web,
            cart::add_to_cart,
            comics::comicvine_search,
            export::export_pdf,
            vision::read_screen,
            desktop::set_volume,
            desktop::set_brightness,
            desktop::lock_screen,
            desktop::take_screenshot,
            computer::open_app,
            computer::close_app,
            computer::run_command,
            computer::list_files,
            #[cfg(feature = "desktop")]
            voice_loop::set_voice_loop_enabled,
            #[cfg(feature = "desktop")]
            voice_loop::set_voice_loop_paused
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
