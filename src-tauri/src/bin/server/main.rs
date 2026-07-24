//! Standalone HTTP server exposing the same capabilities as the Tauri
//! desktop app's commands, so Jarvis can also be reached as a local web app
//! from other devices on the same network. Reuses the `*_impl` functions in
//! `jarvis_lib::commands::*` — no business logic is duplicated here, this
//! file is just HTTP routing/(de)serialization glue.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use jarvis_lib::commands::apple::{self, AppleEvent, AppleReminder};
use jarvis_lib::commands::google;
use jarvis_lib::commands::home;
use jarvis_lib::commands::notion::{self, NotionPage};
use jarvis_lib::commands::news::{self, NewsCache, NewsItem};
use jarvis_lib::commands::ollama::{self, ChatMessage, ModelInfo};
use jarvis_lib::commands::reminders::{self, Reminder};
use jarvis_lib::commands::system::{self, SystemState, SystemStats};
#[cfg(feature = "desktop")]
use jarvis_lib::commands::voice::{self, VoiceState};
use jarvis_lib::commands::weather::{self, GeocodeResult, WeatherResponse};
use jarvis_lib::state::AppState;
use jarvis_lib::{db, resolve_manifest_resource_path};

mod tls;

const PORT: u16 = 4488;

struct ServerState {
    app: AppState,
    #[cfg(feature = "desktop")]
    voice: Option<VoiceState>,
    system: SystemState,
    news: NewsCache,
}

type SharedState = Arc<ServerState>;

struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0).into_response()
    }
}

impl From<String> for ApiError {
    fn from(value: String) -> Self {
        ApiError(value)
    }
}

/// Same directory the Tauri desktop app uses (`com.jarvis.assistant` is
/// this app's identifier in `tauri.conf.json`), so reminders/data are
/// shared between the desktop app and this web server rather than forked.
fn shared_app_data_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    PathBuf::from(home).join(".local/share/com.jarvis.assistant")
}

async fn list_models_handler() -> Result<Json<Vec<ModelInfo>>, ApiError> {
    Ok(Json(ollama::list_models().await?))
}

#[derive(Deserialize)]
struct ChatBody {
    model: String,
    messages: Vec<ChatMessage>,
}

async fn chat_handler(Json(body): Json<ChatBody>) -> Result<Json<String>, ApiError> {
    Ok(Json(ollama::chat(body.model, body.messages).await?))
}

/// Streams the reply as plain-text chunks (one or more tokens per chunk) so
/// the browser can render — and speak — the response as it generates
/// instead of waiting for the whole thing.
async fn chat_stream_handler(Json(body): Json<ChatBody>) -> Response {
    let (tx, rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<axum::body::Bytes, std::io::Error>>();

    tokio::spawn(async move {
        let result = ollama::chat_stream_tokens(body.model, body.messages, |token| {
            let _ = tx.send(Ok(axum::body::Bytes::from(token.as_bytes().to_vec())));
        })
        .await;
        if let Err(e) = result {
            let _ = tx.send(Ok(axum::body::Bytes::from(format!("\n[error] {e}"))));
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

#[derive(Deserialize)]
struct WeatherQuery {
    lat: f64,
    lon: f64,
    fahrenheit: bool,
}

async fn weather_handler(Query(q): Query<WeatherQuery>) -> Result<Json<WeatherResponse>, ApiError> {
    Ok(Json(
        weather::get_weather(q.lat, q.lon, q.fahrenheit).await?,
    ))
}

#[derive(Deserialize)]
struct GeocodeQuery {
    name: String,
}

async fn geocode_handler(Query(q): Query<GeocodeQuery>) -> Result<Json<GeocodeResult>, ApiError> {
    Ok(Json(weather::geocode_city(q.name).await?))
}

async fn list_reminders_handler(
    State(state): State<SharedState>,
) -> Result<Json<Vec<Reminder>>, ApiError> {
    Ok(Json(reminders::list_reminders_impl(&state.app)?))
}

#[derive(Deserialize)]
struct CreateReminderBody {
    text: String,
    due_at: String,
}

async fn create_reminder_handler(
    State(state): State<SharedState>,
    Json(body): Json<CreateReminderBody>,
) -> Result<Json<Reminder>, ApiError> {
    Ok(Json(reminders::create_reminder_impl(
        &state.app,
        body.text,
        body.due_at,
    )?))
}

async fn delete_reminder_handler(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    reminders::delete_reminder_impl(&state.app, id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ParseReminderBody {
    model: String,
    utterance: String,
}

async fn parse_reminder_handler(
    State(state): State<SharedState>,
    Json(body): Json<ParseReminderBody>,
) -> Result<Json<Reminder>, ApiError> {
    Ok(Json(
        reminders::parse_and_create_reminder_impl(&state.app, body.model, body.utterance).await?,
    ))
}

#[cfg(feature = "desktop")]
fn require_voice(state: &ServerState) -> Result<&VoiceState, ApiError> {
    state
        .voice
        .as_ref()
        .ok_or_else(|| ApiError("Voice features unavailable on this server".to_string()))
}

#[cfg(feature = "desktop")]
#[derive(Deserialize)]
struct TranscribeBody {
    audio: Vec<f32>,
    fast: bool,
}

#[cfg(feature = "desktop")]
async fn transcribe_handler(
    State(state): State<SharedState>,
    Json(body): Json<TranscribeBody>,
) -> Result<Json<String>, ApiError> {
    let voice = require_voice(&state)?;
    Ok(Json(voice::transcribe_impl(voice, body.audio, body.fast)?))
}

#[cfg(feature = "desktop")]
#[derive(Deserialize)]
struct SpeakBody {
    text: String,
}

#[cfg(feature = "desktop")]
async fn speak_handler(
    State(state): State<SharedState>,
    Json(body): Json<SpeakBody>,
) -> Result<Response, ApiError> {
    let voice = require_voice(&state)?;
    let bytes = voice::synthesize_speech_bytes_impl(voice, body.text).await?;
    Ok(([(header::CONTENT_TYPE, "audio/wav")], bytes).into_response())
}

async fn system_stats_handler(
    State(state): State<SharedState>,
) -> Result<Json<SystemStats>, ApiError> {
    Ok(Json(system::get_system_stats_impl(&state.system)?))
}

#[derive(Deserialize)]
struct HomeCommandBody {
    utterance: String,
}

async fn home_command_handler(
    Json(body): Json<HomeCommandBody>,
) -> Result<Json<Option<String>>, ApiError> {
    Ok(Json(home::home_command_impl(&body.utterance).await?))
}

async fn notion_pages_handler() -> Result<Json<Vec<NotionPage>>, ApiError> {
    Ok(Json(notion::list_pages_impl().await?))
}

#[derive(Deserialize)]
struct NotionPageQuery {
    id: String,
}

async fn notion_read_handler(
    Query(q): Query<NotionPageQuery>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(notion::read_page_impl(q.id).await?))
}

async fn apple_events_handler() -> Result<Json<Vec<AppleEvent>>, ApiError> {
    Ok(Json(apple::list_events_impl().await?))
}

async fn apple_reminders_handler() -> Result<Json<Vec<AppleReminder>>, ApiError> {
    Ok(Json(apple::list_reminders_impl().await?))
}

async fn google_events_handler() -> Result<Json<Vec<AppleEvent>>, ApiError> {
    Ok(Json(google::list_events_impl().await?))
}

#[derive(Deserialize)]
struct NewsQuery {
    category: String,
}

async fn news_handler(
    State(state): State<SharedState>,
    Query(q): Query<NewsQuery>,
) -> Result<Json<Vec<NewsItem>>, ApiError> {
    Ok(Json(news::get_news_impl(&state.news, q.category).await?))
}

#[tokio::main]
async fn main() {
    let app_data_dir = shared_app_data_dir();
    let conn = db::init_db(app_data_dir.clone()).expect("failed to initialize reminders database");
    let tls_config = tls::load_or_create(&tls::cert_dir_from_app_data(&app_data_dir)).await;

    #[cfg(feature = "desktop")]
    let voice = {
        let small = resolve_manifest_resource_path("resources/whisper/ggml-small.en.bin");
        let whisper_model_path = if small.exists() {
            small
        } else {
            resolve_manifest_resource_path("resources/whisper/ggml-base.en.bin")
        };
        let whisper_fast_model_path =
            resolve_manifest_resource_path("resources/whisper/ggml-tiny.en.bin");
        let piper_bin = resolve_manifest_resource_path("resources/piper/piper");
        let piper_lib_dir = resolve_manifest_resource_path("resources/piper");
        let piper_voice =
            resolve_manifest_resource_path("resources/piper/voices/en_GB-jenny_dioco-medium.onnx");
        match VoiceState::init(
            whisper_model_path,
            whisper_fast_model_path,
            piper_bin,
            piper_lib_dir,
            piper_voice,
            None,
        ) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("Voice features disabled: {e}");
                None
            }
        }
    };

    let state: SharedState = Arc::new(ServerState {
        app: AppState {
            db: std::sync::Mutex::new(conn),
        },
        #[cfg(feature = "desktop")]
        voice,
        system: SystemState::new(),
        news: NewsCache::new(),
    });

    let dist_dir = resolve_manifest_resource_path("../dist");

    let api = Router::new()
        .route("/models", get(list_models_handler))
        .route("/chat", post(chat_handler))
        .route("/chat-stream", post(chat_stream_handler))
        .route("/weather", get(weather_handler))
        .route("/geocode", get(geocode_handler))
        .route(
            "/reminders",
            get(list_reminders_handler).post(create_reminder_handler),
        )
        .route("/reminders/{id}", delete(delete_reminder_handler))
        .route("/reminders/parse", post(parse_reminder_handler))
        .route("/system-stats", get(system_stats_handler))
        .route("/news", get(news_handler))
        .route("/home/command", post(home_command_handler))
        .route("/notion/pages", get(notion_pages_handler))
        .route("/notion/page", get(notion_read_handler))
        .route("/apple/events", get(apple_events_handler))
        .route("/apple/reminders", get(apple_reminders_handler))
        .route("/google/events", get(google_events_handler));
    // Voice endpoints only exist when built with the `voice` feature (the
    // headless phone-node build omits them).
    #[cfg(feature = "desktop")]
    let api = api
        .route("/transcribe", post(transcribe_handler))
        .route("/speak", post(speak_handler));
    let api = api.with_state(state);

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(dist_dir))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], PORT));
    let lan_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "<this machine's LAN IP>".to_string());
    println!("Jarvis web server listening on https://{lan_ip}:{PORT} (also reachable at https://localhost:{PORT} on this machine)");
    println!("First visit on each device will show a browser warning for the self-signed certificate — that's expected, click through / accept it.");
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .expect("server error");
}
