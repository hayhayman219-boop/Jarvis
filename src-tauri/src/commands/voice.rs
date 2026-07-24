use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{Emitter, State};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const TARGET_SAMPLE_RATE: u32 = 16_000;
// This machine is a 12th Gen Intel i5-12450H: 8 cores / 12 logical threads.
// whisper.cpp sees diminishing (or negative) returns past physical-core
// count on clips this short, and pegging every thread previously starved
// audio playback and other on-demand work. 4 threads is plenty for a one-off
// 5s accurate transcription.
const WHISPER_THREADS: i32 = 4;
// The wake-word/stop-word pass runs continuously in the background, roughly
// every ~2.5s, for as long as the app is open, on a tiny model and a short
// clip — 2 threads is enough, and keeping it low leaves headroom for
// anything else (including this app's own audio playback) running at the
// same time.
const WHISPER_THREADS_BACKGROUND: i32 = 2;

pub struct RecordingHandle {
    stop_tx: Sender<()>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

pub struct VoiceState {
    /// Larger, more accurate model used for real transcribed commands.
    pub whisper_ctx: WhisperContext,
    /// Much smaller/faster model used for the continuous background
    /// wake-word ("Jarvis") / stop-word ("stop") listening loop, so that
    /// always-on listening doesn't peg the CPU running full-size inference
    /// every couple of seconds.
    pub whisper_ctx_fast: WhisperContext,
    pub piper_bin: PathBuf,
    pub piper_lib_dir: PathBuf,
    pub piper_voice: PathBuf,
    pub current_playback: Arc<Mutex<Option<Child>>>,
    pub recording: Mutex<Option<RecordingHandle>>,
    pub tts_queue: TtsQueue,
}

fn load_whisper_ctx(model_path: &PathBuf) -> Result<WhisperContext, String> {
    WhisperContext::new_with_params(
        model_path
            .to_str()
            .ok_or_else(|| "Invalid whisper model path".to_string())?,
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load whisper model at {model_path:?}: {e}"))
}

impl VoiceState {
    pub fn init(
        whisper_model_path: PathBuf,
        whisper_fast_model_path: PathBuf,
        piper_bin: PathBuf,
        piper_lib_dir: PathBuf,
        piper_voice: PathBuf,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Self, String> {
        let whisper_ctx = load_whisper_ctx(&whisper_model_path)?;
        let whisper_ctx_fast = load_whisper_ctx(&whisper_fast_model_path)?;
        let current_playback = Arc::new(Mutex::new(None));
        let tts_queue = TtsQueue::start(
            piper_bin.clone(),
            piper_lib_dir.clone(),
            piper_voice.clone(),
            current_playback.clone(),
            app_handle,
        );

        Ok(VoiceState {
            whisper_ctx,
            whisper_ctx_fast,
            piper_bin,
            piper_lib_dir,
            piper_voice,
            current_playback,
            recording: Mutex::new(None),
            tts_queue,
        })
    }

    /// Resolves a per-persona Piper voice filename (e.g. `en_US-ryan-high.onnx`)
    /// to a full path under the bundled voices directory. Returns `None` when
    /// no override is given or the file isn't present, so callers fall back to
    /// the app's default voice.
    pub fn resolve_voice(&self, filename: Option<&str>) -> Option<PathBuf> {
        let name = filename?.trim();
        if name.is_empty() {
            return None;
        }
        let candidate = self.piper_voice.parent()?.join(name);
        candidate.exists().then_some(candidate)
    }

    /// Whether Jarvis is audibly speaking right now (either playing a clip
    /// or with more queued sentences waiting).
    pub fn is_speaking(&self) -> bool {
        let playing = self
            .current_playback
            .lock()
            .map(|mut guard| match guard.as_mut() {
                Some(child) => matches!(child.try_wait(), Ok(None)),
                None => false,
            })
            .unwrap_or(false);
        playing || !self.tts_queue.is_empty()
    }
}

/// Rewrites a Markdown link `[label](url)` (and image `![alt](url)`) to just
/// its label, so the voice says "Watch the Mark I escape" instead of reading
/// the bracket, the words, and the whole URL aloud. Sub AIs like Hacks emit
/// these constantly, which otherwise turned speech into a stream of URLs.
fn strip_markdown_links(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if bytes[i] == b'[' {
            // Find "](" then the closing ")". If the shape doesn't match, fall
            // through and treat '[' as an ordinary character.
            if let Some(rel_close) = text[i + 1..].find(']') {
                let label_end = i + 1 + rel_close;
                let after = label_end + 1;
                if after < text.len() && bytes[after] == b'(' {
                    if let Some(rel_paren) = text[after + 1..].find(')') {
                        let label = &text[i + 1..label_end];
                        // Drop a preceding '!' so images vanish label-and-all.
                        if out.ends_with('!') {
                            out.pop();
                        }
                        out.push_str(label);
                        i = after + 1 + rel_paren + 1;
                        continue;
                    }
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Strips the markdown decorations LLMs love to emit so the TTS voice reads
/// words, not formatting: link labels only (no URLs), no bullets, no bare
/// URLs, no `*#_>~` clutter. (The web frontend has its own richer TS version;
/// this covers the desktop streaming path where sentences go straight from
/// the Ollama stream to Piper without passing through the frontend.)
pub fn sanitize_for_speech(text: &str) -> String {
    let delinked = strip_markdown_links(text);
    let mut out = String::with_capacity(delinked.len());
    for token in delinked.split_whitespace() {
        let trimmed = token.trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '<' | '>'));
        // Never speak a raw URL.
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("www.")
        {
            continue;
        }
        let cleaned: String = trimmed
            .chars()
            .filter(|c| !matches!(c, '*' | '`' | '#' | '_' | '>' | '~' | '[' | ']'))
            .collect();
        // Drop standalone list bullets ("- ", "* ").
        if cleaned.is_empty() || cleaned == "-" || cleaned == "*" {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&cleaned);
    }
    out
}

/// Blocking synthesis to a temp WAV file via the local Piper voice. Jarvis is
/// fully local — no cloud TTS — so this is the only synthesis path.
fn synthesize_to_wav_blocking(
    piper_bin: &PathBuf,
    piper_lib_dir: &PathBuf,
    piper_voice: &PathBuf,
    text: &str,
) -> Result<PathBuf, String> {
    run_piper(piper_bin, piper_lib_dir, piper_voice, text)
}

struct TtsQueueInner {
    /// Each item is the sentence plus an optional Piper voice override
    /// (a resolved `.onnx` path). `None` = use the app's default voice.
    /// This is how per-persona voices work: Jarvis (default Jenny) and a
    /// Sub AI like Hacks (Ryan) can have sentences interleaved in one queue.
    queue: Mutex<VecDeque<(String, Option<PathBuf>)>>,
    cv: Condvar,
    /// Set by `stop_speaking` to interrupt an in-progress reply. While true,
    /// `enqueue` drops incoming sentences — critical because the chat stream
    /// keeps generating and enqueuing sentences after Stop is pressed, which
    /// would otherwise immediately refill the just-cleared queue. Reset when a
    /// new speak session begins.
    cancelled: AtomicBool,
}

/// Sequential text-to-speech queue: sentences enqueued during a streaming
/// chat response are synthesized and played one after another (a bare
/// `speak_impl` per sentence would have each clip killing the previous one
/// mid-word). A single worker thread owns playback order for the app's
/// lifetime.
pub struct TtsQueue {
    inner: Arc<TtsQueueInner>,
}

impl TtsQueue {
    fn start(
        piper_bin: PathBuf,
        piper_lib_dir: PathBuf,
        piper_voice: PathBuf,
        current_playback: Arc<Mutex<Option<Child>>>,
        app_handle: Option<tauri::AppHandle>,
    ) -> Self {
        let inner = Arc::new(TtsQueueInner {
            queue: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            cancelled: AtomicBool::new(false),
        });
        let worker_inner = inner.clone();

        thread::spawn(move || {
            let emit_speaking = |speaking: bool| {
                if let Some(app) = &app_handle {
                    let _ = app.emit("tts-state", serde_json::json!({ "speaking": speaking }));
                }
            };
            loop {
                let (sentence, voice_override) = {
                    let mut queue = worker_inner.queue.lock().unwrap();
                    while queue.is_empty() {
                        queue = worker_inner.cv.wait(queue).unwrap();
                    }
                    queue.pop_front().unwrap()
                };

                emit_speaking(true);

                let voice_ref = voice_override.as_ref().unwrap_or(&piper_voice);
                let wav = match synthesize_to_wav_blocking(
                    &piper_bin,
                    &piper_lib_dir,
                    voice_ref,
                    &sentence,
                ) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("TTS synthesis failed: {e}");
                        continue;
                    }
                };

                match spawn_player(&wav) {
                    Ok(child) => {
                        eprintln!(
                            "[tts] playing ({} chars): {:?}",
                            sentence.len(),
                            &sentence[..sentence.len().min(48)]
                        );
                        // Watchdog deadline: a playback child that outlives
                        // its clip by several seconds is hung (observed with
                        // `aplay` fighting PipeWire for the ALSA device: it
                        // sat silent for minutes on a 4s clip, which both
                        // muted Jarvis and pinned is_speaking() true — which
                        // in turn disabled wake-word listening entirely).
                        let deadline = wav_duration_secs(&wav) + Duration::from_secs(4);
                        let started = std::time::Instant::now();
                        if let Ok(mut guard) = current_playback.lock() {
                            *guard = Some(child);
                        }
                        // Poll rather than wait() so stop_speaking can kill the
                        // child (it holds the same Arc) without us blocking the
                        // lock for the clip's whole duration.
                        loop {
                            thread::sleep(Duration::from_millis(100));
                            let mut guard = match current_playback.lock() {
                                Ok(g) => g,
                                Err(_) => break,
                            };
                            match guard.as_mut() {
                                Some(c) => match c.try_wait() {
                                    Ok(Some(status)) => {
                                        if !status.success() {
                                            eprintln!("[tts] player exited with {status}");
                                        }
                                        *guard = None;
                                        break;
                                    }
                                    Err(_) => {
                                        *guard = None;
                                        break;
                                    }
                                    Ok(None) => {
                                        if started.elapsed() > deadline {
                                            eprintln!(
                                                "[tts] playback hung (>{deadline:?}); killing player"
                                            );
                                            let _ = c.kill();
                                            *guard = None;
                                            break;
                                        }
                                    }
                                },
                                None => {
                                    eprintln!("[tts] playback interrupted by stop");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to launch audio player: {e}"),
                }
                let _ = std::fs::remove_file(&wav);

                if worker_inner.queue.lock().unwrap().is_empty() {
                    emit_speaking(false);
                }
            }
        });

        TtsQueue { inner }
    }

    pub fn enqueue(&self, sentence: String, voice: Option<PathBuf>) {
        // Dropped while a Stop is in effect, so the still-running chat stream
        // can't refill the queue we just cleared.
        if self.inner.cancelled.load(Ordering::SeqCst) {
            return;
        }
        let sanitized = sanitize_for_speech(&sentence);
        if sanitized.is_empty() {
            return;
        }
        self.inner.queue.lock().unwrap().push_back((sanitized, voice));
        self.inner.cv.notify_one();
    }

    pub fn clear(&self) {
        self.inner.queue.lock().unwrap().clear();
    }

    /// Interrupt current speech: raise the cancel flag and drop everything
    /// pending. Subsequent `enqueue`s are ignored until `begin_session`.
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::SeqCst);
        self.inner.queue.lock().unwrap().clear();
    }

    /// Start a fresh speak session: clear the cancel flag so new sentences are
    /// accepted again. Called at the top of each chat reply / one-off speak.
    pub fn begin_session(&self) {
        self.inner.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn is_empty(&self) -> bool {
        self.inner.queue.lock().unwrap().is_empty()
    }
}

pub(crate) fn push_downmixed_resampled(
    out: &mut Vec<f32>,
    data: &[f32],
    channels: usize,
    sample_rate: u32,
) {
    let mono: Vec<f32> = data
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();

    if sample_rate == TARGET_SAMPLE_RATE {
        out.extend_from_slice(&mono);
        return;
    }

    // Box-filter downsampling: average every source sample that falls in
    // each output sample's window, rather than picking one nearest sample.
    // Nearest-neighbor decimation (the previous approach) aliases the
    // high frequencies down into the speech band, which measurably hurts
    // Whisper accuracy — a crude low-pass via averaging is cheap and much
    // kinder to the signal (typical case here: 48kHz -> 16kHz, so each
    // output sample averages ~3 inputs).
    let ratio = sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let out_len = (mono.len() as f64 / ratio) as usize;
    for i in 0..out_len {
        let start = (i as f64 * ratio) as usize;
        let end = (((i + 1) as f64 * ratio) as usize)
            .min(mono.len())
            .max(start + 1);
        let sum: f32 = mono[start..end].iter().sum();
        out.push(sum / (end - start) as f32);
    }
}

// Only used by the Tauri desktop app (native mic capture via cpal); the web
// app records audio in the browser instead, so these have no server
// equivalent and stay as plain Tauri commands.
#[tauri::command]
pub fn start_recording(state: State<VoiceState>) -> Result<(), String> {
    let mut recording = state.recording.lock().map_err(|e| e.to_string())?;
    if recording.is_some() {
        return Err("Already recording".to_string());
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No input (microphone) device found".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {e}"))?;

    let sample_rate = config.sample_rate();
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let buffer_for_thread = buffer.clone();
    let (stop_tx, stop_rx) = channel::<()>();
    let (ready_tx, ready_rx) = channel::<Result<(), String>>();

    thread::spawn(move || {
        let err_fn = |err| eprintln!("cpal input stream error: {err}");
        let stream_config: cpal::StreamConfig = config.into();

        let stream = if sample_format == cpal::SampleFormat::F32 {
            device.build_input_stream(
                stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut buf = buffer_for_thread.lock().unwrap();
                    push_downmixed_resampled(&mut buf, data, channels, sample_rate);
                },
                err_fn,
                None,
            )
        } else {
            let _ = ready_tx.send(Err(format!(
                "Unsupported input sample format: {sample_format:?}"
            )));
            return;
        };

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed to build input stream: {e}")));
                return;
            }
        };

        if let Err(e) = stream.play() {
            let _ = ready_tx.send(Err(format!("Failed to start input stream: {e}")));
            return;
        }

        let _ = ready_tx.send(Ok(()));
        let _ = stop_rx.recv();
        drop(stream);
    });

    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "Timed out starting microphone stream".to_string())??;

    *recording = Some(RecordingHandle { stop_tx, buffer });
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: State<VoiceState>) -> Result<Vec<f32>, String> {
    let handle = state
        .recording
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| "Not currently recording".to_string())?;

    let _ = handle.stop_tx.send(());
    thread::sleep(Duration::from_millis(150));

    let buf = handle.buffer.lock().map_err(|e| e.to_string())?;
    Ok(buf.clone())
}

// `*_impl` functions take a plain `&VoiceState` so they're reusable from
// both the Tauri app and the standalone Axum server (`bin/server.rs`).

pub fn transcribe_impl(state: &VoiceState, audio: Vec<f32>, fast: bool) -> Result<String, String> {
    // Level-normalize before Whisper. This laptop's digital mic captures very
    // quiet (~0.002 RMS even when the user speaks loudly), and Whisper
    // hallucinates repeated filler ("I don't know. I don't know…") on
    // near-silent input. Scale the clip toward Whisper's preferred loudness
    // (~0.12 RMS), capped so a genuinely silent window isn't blown up into
    // amplified noise.
    let audio = {
        let n = audio.len().max(1) as f32;
        let rms = (audio.iter().map(|s| s * s).sum::<f32>() / n).sqrt();
        let peak = audio.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        if rms > 1e-4 && peak > 1e-4 {
            // Aim for ~0.1 RMS, but cap the gain so the loudest sample stays
            // under 0.95 — quiet input gets boosted, already-loud input isn't
            // clipped into distortion.
            let gain = (0.1 / rms).min(0.95 / peak).min(50.0);
            audio.into_iter().map(|s| s * gain).collect()
        } else {
            audio
        }
    };

    let ctx = if fast {
        &state.whisper_ctx_fast
    } else {
        &state.whisper_ctx
    };
    let mut whisper_state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create whisper state: {e}"))?;

    // Command transcription (non-fast) is tuned for MAX accuracy: beam search
    // instead of greedy, English forced, and a rich vocabulary prompt. The
    // background wake/stop check stays greedy for speed.
    let strategy = if fast {
        SamplingStrategy::Greedy { best_of: 1 }
    } else {
        SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 }
    };
    let mut params = FullParams::new(strategy);
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // Force English so the multilingual turbo model never misdetects the
    // language (a real source of garbled transcripts).
    params.set_language(Some("en"));
    // Drop blank/no-speech artifacts rather than emitting them, and reject
    // high-uncertainty / non-speech output — this is what cuts the repeated
    // filler hallucinations ("I don't know…") Whisper produces on this quiet
    // mic's audio.
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_no_speech_thold(0.6);
    params.set_entropy_thold(2.4);
    // Vocabulary bias: Whisper conditions on this as if it preceded the
    // audio, sharply improving recognition of the assistant's proper nouns and
    // command words (real mishearings on this mic: "Jarvis" -> "nervous").
    params.set_initial_prompt(
        "Hey Jarvis. What's the weather? Open the news. Search YouTube. Open Notion. \
         Take a screenshot. Lock the screen. Turn up the volume. Set the brightness. \
         What's on my calendar? Any Pokémon news or leaks? Add a reminder. Show my camera. Stop.",
    );
    params.set_n_threads(if fast {
        WHISPER_THREADS_BACKGROUND
    } else {
        WHISPER_THREADS
    });
    if fast {
        // Skip the temperature-fallback retry passes for the lightweight
        // wake-word/stop-word check — a rough transcription is enough to
        // spot "jarvis"/"stop", and this is the main cost of running
        // continuous background inference every couple of seconds.
        params.set_temperature_inc(0.0);
    }

    whisper_state
        .full(params, &audio)
        .map_err(|e| format!("Whisper transcription failed: {e}"))?;

    let num_segments = whisper_state.full_n_segments();
    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = whisper_state.get_segment(i) {
            if let Ok(segment_text) = segment.to_str_lossy() {
                text.push_str(&segment_text);
            }
        }
    }
    let text = text.trim().to_string();
    // Final guard: if Whisper still returned a short phrase repeated over and
    // over (its signature hallucination on unclear audio), treat it as nothing
    // heard rather than acting on garbage.
    if looks_like_hallucination(&text) {
        return Ok(String::new());
    }
    Ok(text)
}

/// True when `text` is one short phrase repeated ≥3 times (e.g. "I don't know.
/// I don't know. I don't know."), which is a Whisper hallucination, not speech.
fn looks_like_hallucination(text: &str) -> bool {
    let parts: Vec<String> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    parts.len() >= 3 && parts[0].len() <= 24 && parts.iter().all(|p| *p == parts[0])
}

fn kill_current_playback(current_playback: &Mutex<Option<Child>>) {
    if let Ok(mut guard) = current_playback.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            // Reap it: kill() without wait() leaves a zombie for the app's
            // whole lifetime (observed: defunct pw-play from a mid-reply stop).
            let _ = child.wait();
        }
    }
}

/// Picks the audio player once: prefer PipeWire-native `pw-play` — this
/// system (Pop!_OS) runs PipeWire, and raw ALSA `aplay` was observed hanging
/// indefinitely fighting it for the device — then `paplay`, then `aplay`.
fn player_binary() -> &'static str {
    use std::sync::OnceLock;
    static PLAYER: OnceLock<&'static str> = OnceLock::new();
    PLAYER.get_or_init(|| {
        for candidate in ["pw-play", "paplay", "aplay"] {
            let found = Command::new("which")
                .arg(candidate)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if found {
                eprintln!("[tts] using audio player: {candidate}");
                return candidate;
            }
        }
        "aplay"
    })
}

fn spawn_player(wav: &PathBuf) -> Result<Child, String> {
    Command::new(player_binary())
        .arg(wav)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{} failed to start: {e}", player_binary()))
}

/// Rough clip length from file size, assuming Piper's output format
/// (22050 Hz, mono, 16-bit). Only used to size the playback watchdog, so
/// precision doesn't matter.
fn wav_duration_secs(wav: &PathBuf) -> Duration {
    let bytes = std::fs::metadata(wav).map(|m| m.len()).unwrap_or(0);
    let payload = bytes.saturating_sub(44) as f64;
    Duration::from_secs_f64(payload / (22_050.0 * 2.0))
}

static SPEECH_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Runs the bundled Piper neural-TTS voice (far more natural than the
/// default espeak-ng/speech-dispatcher voice) and returns the path to the
/// rendered WAV file. The filename includes a per-process counter, not just
/// the PID, so concurrent calls (e.g. several devices talking to the web
/// server at once) never collide on the same temp file.
fn run_piper(
    piper_bin: &PathBuf,
    piper_lib_dir: &PathBuf,
    piper_voice: &PathBuf,
    text: &str,
) -> Result<PathBuf, String> {
    let unique = SPEECH_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_wav =
        std::env::temp_dir().join(format!("jarvis-speech-{}-{unique}.wav", std::process::id()));

    let mut piper = Command::new(piper_bin)
        .arg("--model")
        .arg(piper_voice)
        .arg("--output_file")
        .arg(&tmp_wav)
        .env("LD_LIBRARY_PATH", piper_lib_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch piper: {e}"))?;

    piper
        .stdin
        .take()
        .ok_or_else(|| "Failed to open piper stdin".to_string())?
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write text to piper: {e}"))?;

    piper
        .wait()
        .map_err(|e| format!("Piper synthesis failed: {e}"))?;

    Ok(tmp_wav)
}

/// Desktop-app path: play the synthesized speech through this machine's own
/// speakers via `aplay`, tracking the child so `stop_speaking` can kill it
/// mid-sentence.
pub fn speak_impl(state: &VoiceState, text: String) -> Result<(), String> {
    // Fresh utterance clears any lingering Stop from a previous reply.
    state.tts_queue.begin_session();
    kill_current_playback(&state.current_playback);
    let tmp_wav = synthesize_to_wav_blocking(
        &state.piper_bin,
        &state.piper_lib_dir,
        &state.piper_voice,
        &text,
    )?;

    let player = spawn_player(&tmp_wav)?;

    *state.current_playback.lock().map_err(|e| e.to_string())? = Some(player);

    // Reap the child once playback finishes so it doesn't linger as a zombie,
    // without holding the lock for the whole duration (that would block
    // stop_speaking from interrupting mid-sentence).
    let playback_arc = state.current_playback.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(200));
        let mut guard = match playback_arc.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) | Err(_) => {
                    *guard = None;
                    let _ = std::fs::remove_file(&tmp_wav);
                    return;
                }
                Ok(None) => continue,
            },
            None => return,
        }
    });

    Ok(())
}

/// Web-app path: synthesize speech (local Piper) and return the raw WAV bytes
/// so the browser making the request can play it back on whatever device is
/// actually viewing the page (not this server's own speakers).
pub async fn synthesize_speech_bytes_impl(
    state: &VoiceState,
    text: String,
) -> Result<Vec<u8>, String> {
    let (bin, lib, voice) = (
        state.piper_bin.clone(),
        state.piper_lib_dir.clone(),
        state.piper_voice.clone(),
    );
    let tmp_wav = tokio::task::spawn_blocking(move || run_piper(&bin, &lib, &voice, &text))
        .await
        .map_err(|e| e.to_string())??;
    let bytes = std::fs::read(&tmp_wav)
        .map_err(|e| format!("Failed to read synthesized audio: {e}"))?;
    let _ = std::fs::remove_file(&tmp_wav);
    Ok(bytes)
}

pub fn stop_speaking_impl(state: &VoiceState) {
    let dropped = state.tts_queue.inner.queue.lock().map(|q| q.len()).unwrap_or(0);
    eprintln!("[tts] stop requested ({dropped} queued sentence(s) dropped)");
    // cancel() raises the flag AND clears the queue, so any sentences the
    // still-running chat stream tries to enqueue after this are dropped
    // instead of resuming playback a beat later.
    state.tts_queue.cancel();
    kill_current_playback(&state.current_playback);
}

#[tauri::command]
pub fn transcribe(state: State<VoiceState>, audio: Vec<f32>, fast: bool) -> Result<String, String> {
    transcribe_impl(&state, audio, fast)
}

#[tauri::command]
pub fn speak(state: State<VoiceState>, text: String) -> Result<(), String> {
    speak_impl(&state, text)
}

#[tauri::command]
pub fn stop_speaking(state: State<VoiceState>) -> Result<(), String> {
    stop_speaking_impl(&state);
    Ok(())
}
