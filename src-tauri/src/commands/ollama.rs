use serde::{Deserialize, Serialize};
use std::time::Duration;

// 127.0.0.1, not "localhost": on this machine `localhost` resolves to IPv6
// `::1` first, but Ollama listens only on IPv4 `127.0.0.1:11434`. Connecting
// via the hostname raced IPv6 (connection refused) against IPv4 and
// intermittently failed with "error sending request". Pinning IPv4 fixes it.
const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
/// Cold-loading a model into RAM can alone take a minute or more on this
/// machine (observed: llama3.2 took ~60s, which blew a previous 60s
/// total-request timeout before the first token arrived). So: fail fast
/// only on connecting, and treat prolonged *inactivity* — not total request
/// duration — as the failure signal, so long generations and slow model
/// loads both survive while a truly hung request still errors out.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(180);
// This machine is a 12th Gen Intel i5-12450H: 8 physical cores / 12 logical
// threads (hyperthreaded). llama.cpp-based inference is compute-bound and
// typically performs best around the physical core count — hyperthreads
// share execution units and mostly just add contention for this workload.
const OLLAMA_THREADS: i32 = 8;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
    remote_host: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub name: String,
    /// True if this model is forwarded to a remote/cloud host (e.g. Ollama's
    /// hosted models) rather than actually running on this machine.
    pub is_remote: bool,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    num_thread: i32,
    /// Ollama's default context window (~4096 tokens) silently truncates the
    /// prompt FROM THE TOP when exceeded — which discards the system prompt
    /// and injected live data (weather/news) first, making the model fall
    /// back to stale training knowledge with no visible error.
    num_ctx: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
    /// Anti-repetition. Smaller models (qwen2.5:3b) tend to fall into loops —
    /// repeating a sentence or a whole section — especially on Hacks' long,
    /// structured presentations. A stronger-than-default penalty (1.1) over a
    /// wider window (default 64) breaks those loops without hurting coherence.
    repeat_penalty: f32,
    repeat_last_n: i32,
}

const OLLAMA_NUM_CTX: i32 = 8192;
const REPEAT_PENALTY: f32 = 1.2;
const REPEAT_LAST_N: i32 = 256;
/// Hard ceiling on a single reply so a runaway repetition loop can't generate
/// forever (it would otherwise stream until the 180s read timeout). Generous
/// enough for a long Hacks presentation (~1500+ words).
const MAX_REPLY_TOKENS: i32 = 2048;

/// Keep the model resident in RAM this long after each request. Ollama's
/// default (~5 min) lets the model unload between uses, so the next message
/// cold-loads it (~6s here) — and requests sent during that window
/// intermittently dropped the connection ("error sending request"). Holding
/// the model warm removes the cold-start race entirely.
const KEEP_ALIVE: &str = "30m";

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
    keep_alive: &'a str,
}

/// POSTs `body` to Ollama, retrying a few times when the send itself fails
/// (connection refused/reset/dropped, or a slow connect) — which is exactly
/// what a cold-loading or briefly-busy Ollama does. Safe to retry because no
/// response has been consumed yet.
async fn send_with_retry(
    url: &str,
    body: &ChatRequest<'_>,
) -> Result<reqwest::Response, String> {
    let mut attempt = 0u32;
    loop {
        match ollama_client().post(url).json(body).send().await {
            Ok(resp) => return Ok(resp),
            Err(e) if attempt < 3 && (e.is_connect() || e.is_request() || e.is_timeout()) => {
                attempt += 1;
                eprintln!("[ollama] send failed ({e}); retry {attempt}/3");
                tokio::time::sleep(Duration::from_millis(700 * attempt as u64)).await;
            }
            Err(e) => return Err(format!("Failed to reach Ollama at {url}: {e}")),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

fn ollama_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .expect("failed to build Ollama HTTP client")
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn list_models() -> Result<Vec<ModelInfo>, String> {
    let url = format!("{OLLAMA_BASE_URL}/api/tags");
    let resp = ollama_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama at {url}: {e}"))?
        .json::<TagsResponse>()
        .await
        .map_err(|e| format!("Failed to parse Ollama /api/tags response: {e}"))?;
    Ok(resp
        .models
        .into_iter()
        .map(|m| ModelInfo {
            name: m.name,
            is_remote: m.remote_host.is_some(),
        })
        .collect())
}

async fn chat_with_options(
    model: String,
    messages: Vec<ChatMessage>,
    num_predict: Option<i32>,
    json_mode: bool,
) -> Result<String, String> {
    let url = format!("{OLLAMA_BASE_URL}/api/chat");
    let body = ChatRequest {
        model: &model,
        messages: &messages,
        stream: false,
        options: Some(ChatOptions {
            num_thread: OLLAMA_THREADS,
            num_ctx: OLLAMA_NUM_CTX,
            num_predict,
            repeat_penalty: REPEAT_PENALTY,
            repeat_last_n: REPEAT_LAST_N,
        }),
        format: if json_mode { Some("json") } else { None },
        keep_alive: KEEP_ALIVE,
    };
    let resp = send_with_retry(&url, &body)
        .await?
        .json::<ChatResponse>()
        .await
        .map_err(|e| format!("Failed to parse Ollama /api/chat response: {e}"))?;
    Ok(resp.message.content)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn chat(model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    chat_with_options(model, messages, None, false).await
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    message: Option<ChatMessage>,
    #[serde(default)]
    done: bool,
}

/// Streams a chat completion from Ollama (NDJSON), invoking `on_token` for
/// each content fragment as it arrives, and returns the full assembled
/// reply. Used by both the Tauri `chat_stream` command (forwarding tokens as
/// window events) and the web server's `/api/chat-stream` endpoint.
pub async fn chat_stream_tokens(
    model: String,
    messages: Vec<ChatMessage>,
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    let url = format!("{OLLAMA_BASE_URL}/api/chat");
    let body = ChatRequest {
        model: &model,
        messages: &messages,
        stream: true,
        options: Some(ChatOptions {
            num_thread: OLLAMA_THREADS,
            num_ctx: OLLAMA_NUM_CTX,
            num_predict: Some(MAX_REPLY_TOKENS),
            repeat_penalty: REPEAT_PENALTY,
            repeat_last_n: REPEAT_LAST_N,
        }),
        format: None,
        keep_alive: KEEP_ALIVE,
    };
    let mut resp = send_with_retry(&url, &body).await?;

    let mut line_buf = String::new();
    let mut full = String::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Ollama stream error: {e}"))?
    {
        line_buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(newline_pos) = line_buf.find('\n') {
            let line: String = line_buf.drain(..=newline_pos).collect();
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parsed: StreamChunk = serde_json::from_str(line)
                .map_err(|e| format!("Failed to parse Ollama stream line '{line}': {e}"))?;
            if let Some(message) = parsed.message {
                if !message.content.is_empty() {
                    full.push_str(&message.content);
                    on_token(&message.content);
                }
            }
            if parsed.done {
                return Ok(full);
            }
        }
    }
    Ok(full)
}

/// Used for structured-output extraction (e.g. reminder parsing). Forces
/// Ollama's grammar-constrained JSON mode so the reply is always valid JSON
/// (rather than hoping the model follows a "respond with only JSON"
/// instruction), and caps generation so a model that falls into a
/// repetition loop fails fast instead of hanging the UI.
pub async fn chat_json(model: String, messages: Vec<ChatMessage>) -> Result<String, String> {
    chat_with_options(model, messages, Some(200), true).await
}

/// Minimum bytes a sentence must reach before a terminator splits it off,
/// so abbreviations ("Dr.", "e.g.") and terse fragments don't produce
/// choppy one-word TTS clips.
const MIN_SENTENCE_LEN: usize = 24;

/// Drains any complete sentences off the front of `pending`, leaving the
/// unfinished tail in place. A sentence is complete at '.', '!' or '?'
/// followed by whitespace (which must have already arrived in the stream),
/// or at a newline.
fn drain_complete_sentences(pending: &mut String) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        let mut split_at = None;
        {
            let chars: Vec<(usize, char)> = pending.char_indices().collect();
            for (i, (byte_idx, c)) in chars.iter().enumerate() {
                let end = byte_idx + c.len_utf8();
                if end < MIN_SENTENCE_LEN {
                    continue;
                }
                match c {
                    '\n' => {
                        split_at = Some(end);
                        break;
                    }
                    '.' | '!' | '?' => {
                        let next_is_space = chars
                            .get(i + 1)
                            .map(|(_, nc)| nc.is_whitespace())
                            .unwrap_or(false);
                        if next_is_space {
                            split_at = Some(end);
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        match split_at {
            Some(pos) => {
                let sentence: String = pending.drain(..pos).collect();
                let trimmed = sentence.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
            None => break,
        }
    }
    out
}

/// Streaming chat for the desktop app: forwards tokens to the window as
/// `chat-token` events for live display, and (when `speak` is on) feeds
/// completed sentences into the TTS queue as they arrive, so Jarvis starts
/// talking a sentence or two into the reply instead of after the whole
/// generation finishes.
#[cfg(feature = "desktop")]
#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn chat_stream(
    app: tauri::AppHandle,
    model: String,
    messages: Vec<ChatMessage>,
    speak: bool,
    // Optional per-persona Piper voice filename (e.g. Hacks -> Ryan). When
    // absent/unresolvable, the TTS queue uses the app's default voice (Jenny).
    voice: Option<String>,
) -> Result<String, String> {
    use crate::commands::voice::VoiceState;
    use tauri::{Emitter, Manager};

    let mut pending = String::new();
    let mut enqueued_any = false;

    // Resolve the persona's voice override once, up front, and clear any
    // lingering Stop from a previous reply so this one is allowed to speak.
    let voice_override: Option<std::path::PathBuf> = app
        .try_state::<VoiceState>()
        .map(|v| {
            v.tts_queue.begin_session();
            v.resolve_voice(voice.as_deref())
        })
        .flatten();

    let result = chat_stream_tokens(model, messages, |token| {
        let _ = app.emit("chat-token", serde_json::json!({ "content": token }));
        if speak {
            pending.push_str(token);
            for sentence in drain_complete_sentences(&mut pending) {
                if let Some(vs) = app.try_state::<VoiceState>() {
                    vs.tts_queue.enqueue(sentence, voice_override.clone());
                    enqueued_any = true;
                }
            }
        }
    })
    .await;

    if speak {
        let tail = pending.trim().to_string();
        if !tail.is_empty() {
            if let Some(vs) = app.try_state::<VoiceState>() {
                vs.tts_queue.enqueue(tail, voice_override.clone());
                enqueued_any = true;
            }
        }
        if !enqueued_any {
            // Nothing was ever spoken, so the TTS worker won't emit the
            // final "not speaking" event the frontend uses to leave the
            // speaking state — send it here.
            let _ = app.emit("tts-state", serde_json::json!({ "speaking": false }));
        }
    }

    let full = result?;
    let _ = app.emit("chat-done", serde_json::json!({ "content": full }));
    Ok(full)
}
