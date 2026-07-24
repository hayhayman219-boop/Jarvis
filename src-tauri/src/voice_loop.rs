//! Always-on wake-word listener for the desktop app.
//!
//! Replaces the old frontend-driven loop that opened and closed the
//! microphone every ~2.5 seconds (constant stream churn, missed words at
//! window boundaries, and a suspect in earlier session instability). This
//! version holds ONE persistent cpal input stream into a rolling ring
//! buffer, and only runs Whisper when a cheap RMS energy check says there
//! was actually sound — so idle CPU cost is near zero.
//!
//! Detected interactions are pushed to the frontend as window events:
//! - `voice-status`  { status: "listening" | "idle" }  (reactor display)
//! - `voice-command` { text }                          (send to the LLM)
//!
//! The web app can't use this (its microphone lives in the visitor's
//! browser), so the JS loop remains for that platform.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::voice::{self, VoiceState};

const SAMPLE_RATE: usize = 16_000;
/// Rolling buffer capacity — 30s is far more than any single interaction.
const RING_CAPACITY: usize = SAMPLE_RATE * 30;
/// Absolute lower bound for the speech gate, below which even a very quiet
/// room won't trigger checks off electrical noise.
const MIN_SPEECH_RMS: f32 = 0.003;
/// Hard ceilings so a hot mic (input gain cranked, constant loud noise)
/// can't drive the adaptive floor — and thus the gate — above the maximum
/// possible RMS (~1.0), which would make the wake word permanently
/// untriggerable. Observed in the wild: gain at 85% pushed the floor to
/// 0.45 and the gate to 1.13, silently deafening wake detection.
const MAX_NOISE_FLOOR: f32 = 0.15;
const MAX_SPEECH_GATE: f32 = 0.35;
/// Speech must exceed the tracked ambient noise floor by this factor. A
/// fixed threshold doesn't work on this hardware: the laptop fan sits right
/// next to the built-in mic, and its noise level moves with CPU load —
/// including load this loop itself creates (whisper heats CPU → fan spins
/// up → noise passes a fixed gate → more whisper → hotter). Tracking the
/// floor adaptively breaks that feedback loop.
const SPEECH_ABOVE_FLOOR: f32 = 1.45;
/// "Silence" for end-of-command detection: below this multiple of the floor.
const SILENCE_BELOW_FLOOR: f32 = 1.7;
/// How often the loop wakes to look at the buffer.
const TICK: Duration = Duration::from_millis(400);
/// Wake-word checks look at this much trailing audio.
const WAKE_WINDOW_SECS: f32 = 3.0;
/// Only re-run a wake check once this much new audio has arrived since the
/// last one, so overlapping windows aren't transcribed over and over.
const MIN_NEW_AUDIO_SECS: f32 = 1.8;
/// Command capture: stop after this much silence following speech...
const COMMAND_SILENCE: Duration = Duration::from_millis(1100);
/// ...or after this long no matter what.
const COMMAND_MAX: Duration = Duration::from_secs(10);

struct Ring {
    buf: VecDeque<f32>,
    /// Total samples ever pushed — lets the loop measure "new audio since
    /// last check" even as old samples fall off the front.
    total_pushed: u64,
}

impl Ring {
    fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            if self.buf.len() == RING_CAPACITY {
                self.buf.pop_front();
            }
            self.buf.push_back(s);
        }
        self.total_pushed += samples.len() as u64;
    }

    fn last_secs(&self, secs: f32) -> Vec<f32> {
        let n = ((secs * SAMPLE_RATE as f32) as usize).min(self.buf.len());
        self.buf.iter().skip(self.buf.len() - n).copied().collect()
    }

    fn clear(&mut self) {
        self.buf.clear();
    }
}

/// DC-removed RMS: digital MEMS mics often sit on a constant DC bias, which
/// would inflate a naive RMS well above any silence threshold and defeat
/// the energy gate entirely. Measuring deviation from the mean captures
/// only the actual audio energy.
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    (samples.iter().map(|s| (s - mean) * (s - mean)).sum::<f32>() / samples.len() as f32).sqrt()
}

fn contains_word(text: &str, word: &str) -> bool {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == word)
}

/// Whisper regularly garbles the proper noun "Jarvis", especially the tiny
/// model — real transcripts from this machine's mic include "nervous" and
/// "George" for a clearly spoken "Jarvis". Matching a set of observed/likely
/// mishearings trades a little precision (a sentence containing "nervous"
/// can false-wake) for a lot of recall, which is the right trade for a
/// personal assistant whose owner keeps saying its name to no effect.
const WAKE_ALIASES: &[&str] = &[
    "jarvis", "jarvas", "jarvus", "jervis", "jarves", "gervis", "gervais", "jarbis", "harvis",
];
/// Extra aliases accepted only in the *accurate-model* pass, after the fast
/// model already heard something jarvis-like — looser matching is safe here
/// because it can't cause a wake on its own, only rescue command extraction.
const EXTRACT_ONLY_ALIASES: &[&str] = &["george", "jarvis's", "travis", "nervous"];
/// The name-word for Hacks and its base.en mishearings. Never wakes on its own
/// (that's the whole point of tightening) — must be preceded by an address word.
const HACKS_NAME_ALIASES: &[&str] = &["hacks", "hax", "hack", "hacki", "acks", "hex", "axe"];
/// Words that count as "addressing" someone before the name. "Hey Hacks" is
/// the canonical form; "hay" is the common base.en mishearing of "hey". Kept
/// deliberately tight — "a"/"ok" were removed because "a hack(s)" and the like
/// occur in ordinary speech and would false-wake.
const ADDRESS_WORDS: &[&str] = &["hey", "hay", "yo", "hi"];

/// Splits `s` into (byte_start, byte_end, word) for each maximal alphanumeric
/// run — so wake matching is punctuation-tolerant ("hey, hacks" still matches).
fn words_with_pos(s: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c.is_alphanumeric() {
            start.get_or_insert(i);
        } else if let Some(st) = start.take() {
            out.push((st, i, &s[st..i]));
        }
    }
    if let Some(st) = start {
        out.push((st, s.len(), &s[st..]));
    }
    out
}

/// Bounded span of the first single-word alias in `aliases` within `lower`.
fn first_span(lower: &str, aliases: &[&str]) -> Option<(usize, usize)> {
    words_with_pos(lower)
        .into_iter()
        .find(|(_, _, w)| aliases.contains(w))
        .map(|(s, e, _)| (s, e))
}

/// Span of an "address + Hacks-name" pair (e.g. "hey hacks"), requiring the two
/// as consecutive words so the everyday word "hacks" can't wake on its own.
fn hacks_wake_span(lower: &str) -> Option<(usize, usize)> {
    let words = words_with_pos(lower);
    words.windows(2).find_map(|pair| {
        let (s0, _, w0) = pair[0];
        let (_, e1, w1) = pair[1];
        if ADDRESS_WORDS.contains(&w0) && HACKS_NAME_ALIASES.contains(&w1) {
            Some((s0, e1))
        } else {
            None
        }
    })
}

/// True when the utterance addressed Hacks ("hey hacks …"). If both a Hacks
/// phrase and a Jarvis alias are present, the one spoken first wins.
fn woke_as_hacks(text: &str) -> bool {
    let lower = text.to_lowercase();
    match (hacks_wake_span(&lower), first_span(&lower, WAKE_ALIASES)) {
        (Some((h, _)), Some((j, _))) => h <= j,
        (Some(_), None) => true,
        _ => false,
    }
}

// --- Double-clap "summon the window" detection ---
/// A clap is a short, LOUD transient — well above speech energy.
const CLAP_ABS_THRESHOLD: f32 = 0.09;
/// 30ms analysis frames (@16kHz).
const CLAP_FRAME: usize = 480;
/// A natural double clap lands in this inter-clap gap.
const CLAP_MIN_GAP_MS: f32 = 90.0;
const CLAP_MAX_GAP_MS: f32 = 700.0;
/// Trailing audio inspected for the two claps.
const CLAP_WINDOW_SECS: f32 = 1.7;

/// True when `samples` (16kHz mono) contain exactly two sharp, loud transients
/// spaced like a double clap. The "exactly two" + sharp-attack rules reject
/// single claps, sustained noise, applause, and speech.
fn is_double_clap(samples: &[f32]) -> bool {
    if samples.len() < CLAP_FRAME * 4 {
        return false;
    }
    let frames: Vec<f32> = samples.chunks(CLAP_FRAME).map(rms).collect();
    let frame_ms = CLAP_FRAME as f32 / SAMPLE_RATE as f32 * 1000.0;
    let mut peaks: Vec<usize> = Vec::new();
    for i in 1..frames.len() {
        let prev = frames[i - 1].max(1e-6);
        let next = *frames.get(i + 1).unwrap_or(&0.0);
        // Loud, a sharp rise from the previous frame, and a local peak.
        if frames[i] > CLAP_ABS_THRESHOLD && frames[i] > prev * 3.0 && frames[i] >= next {
            // Don't double-count a clap that straddles two frames.
            if peaks.last().map_or(true, |&p| i - p > 2) {
                peaks.push(i);
            }
        }
    }
    if peaks.len() != 2 {
        return false;
    }
    let gap_ms = (peaks[1] - peaks[0]) as f32 * frame_ms;
    (CLAP_MIN_GAP_MS..=CLAP_MAX_GAP_MS).contains(&gap_ms)
}

/// If `text` contains a wake alias, returns the remainder of the text after
/// that alias (the command spoken in the same breath), trimmed of leading
/// punctuation. Returns None if no alias is present.
fn text_after_wake_word(text: &str, include_extract_aliases: bool) -> Option<String> {
    let lower = text.to_lowercase();
    // Jarvis single-word aliases (+ the accurate-pass extras) AND the Hacks
    // "hey hacks" pair — strip whichever wake ends earliest so what's left is
    // the command spoken in the same breath.
    let mut singles: Vec<&str> = WAKE_ALIASES.to_vec();
    if include_extract_aliases {
        singles.extend_from_slice(EXTRACT_ONLY_ALIASES);
    }
    let mut best: Option<usize> = None;
    if let Some((_, end)) = first_span(&lower, &singles) {
        best = Some(end);
    }
    if let Some((_, end)) = hacks_wake_span(&lower) {
        best = Some(best.map_or(end, |b: usize| b.min(end)));
    }
    best.map(|end| {
        text[end..]
            .trim_start_matches([',', ':', '.', '!', '?', ' ', '-'])
            .trim()
            .to_string()
    })
}

fn heard_wake_word(text: &str) -> bool {
    let lower = text.to_lowercase();
    let jarvis = lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| WAKE_ALIASES.contains(&w));
    // Hacks needs the full "hey hacks" pair, not a bare "hacks".
    jarvis || hacks_wake_span(&lower).is_some()
}

pub struct VoiceLoopControl {
    pub enabled: AtomicBool,
    /// Set while push-to-talk holds the mic so the loop doesn't also react
    /// to the same speech.
    pub paused: AtomicBool,
}

impl VoiceLoopControl {
    pub fn new() -> Self {
        VoiceLoopControl {
            enabled: AtomicBool::new(true),
            paused: AtomicBool::new(false),
        }
    }
}

// try_state, not State extraction: if VoiceState failed to load (missing
// models), VoiceLoopControl is never managed, and these should no-op rather
// than panic when the frontend toggles the setting.
#[tauri::command]
pub fn set_voice_loop_enabled(app: AppHandle, enabled: bool) {
    if let Some(control) = app.try_state::<VoiceLoopControl>() {
        control.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn set_voice_loop_paused(app: AppHandle, paused: bool) {
    if let Some(control) = app.try_state::<VoiceLoopControl>() {
        control.paused.store(paused, Ordering::Relaxed);
    }
}

/// Opens the default input device as a fresh capture stream feeding `ring`.
/// Re-queries the device each call so a changed default (e.g. plugging in a
/// USB mic) is picked up on reopen.
fn open_mic_stream(ring: &Arc<Mutex<Ring>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no input device".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("no input config: {e}"))?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!(
            "unsupported sample format {:?}",
            config.sample_format()
        ));
    }
    let in_rate = config.sample_rate();
    let channels = config.channels() as usize;
    let ring_for_cb = ring.clone();
    let stream = device
        .build_input_stream(
            config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mut mono = Vec::new();
                voice::push_downmixed_resampled(&mut mono, data, channels, in_rate);
                if let Ok(mut r) = ring_for_cb.lock() {
                    r.push(&mono);
                }
            },
            |err| eprintln!("[voice-loop] input stream error: {err}"),
            None,
        )
        .map_err(|e| format!("failed to open input stream: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("failed to start input stream: {e}"))?;
    Ok(stream)
}

/// If no new samples arrive for this long, the capture stream is considered
/// dead and gets reopened. Observed cause: restarting the PipeWire stack
/// (e.g. the login audio fix) silently severs existing streams — the old
/// behavior left Jarvis permanently deaf until the app was restarted.
const MIC_STALL_TIMEOUT: Duration = Duration::from_secs(5);

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        let ring = Arc::new(Mutex::new(Ring {
            buf: VecDeque::with_capacity(RING_CAPACITY),
            total_pushed: 0,
        }));

        // --- detection state (survives stream reopens) ---
        let mut last_checked_total: u64 = 0;
        let mut tick_count: u64 = 0;
        // Debounce for the double-clap window summon.
        let mut last_clap: Option<Instant> = None;
        // Adaptive ambient noise floor: drops quickly to the quietest recent
        // level, rises only slowly, so brief speech can't drag it up.
        let mut noise_floor: f32 = 0.02;

        'stream: loop {
            let _stream = match open_mic_stream(&ring) {
                Ok(s) => {
                    eprintln!("[voice-loop] microphone stream ready");
                    s
                }
                Err(e) => {
                    eprintln!("[voice-loop] {e}; retrying in 10s");
                    thread::sleep(Duration::from_secs(10));
                    continue 'stream;
                }
            };
            let mut stall_baseline_total = ring.lock().unwrap().total_pushed;
            let mut last_progress = Instant::now();

            loop {
                thread::sleep(TICK);
                tick_count += 1;

                // Mic liveness: samples must keep flowing, or the stream got
                // severed (audio stack restart, device unplugged, suspend).
                {
                    let total = ring.lock().unwrap().total_pushed;
                    if total != stall_baseline_total {
                        stall_baseline_total = total;
                        last_progress = Instant::now();
                    } else if last_progress.elapsed() > MIC_STALL_TIMEOUT {
                        eprintln!(
                            "[voice-loop] mic stream stalled (audio stack restarted?); reopening"
                        );
                        continue 'stream;
                    }
                }

                let recent_rms = {
                    let r = ring.lock().unwrap();
                    rms(&r.last_secs(0.9))
                };
                if recent_rms < noise_floor {
                    noise_floor = noise_floor * 0.7 + recent_rms * 0.3;
                } else {
                    noise_floor += (recent_rms - noise_floor) * 0.02;
                }
                noise_floor = noise_floor.clamp(0.001, MAX_NOISE_FLOOR);
                let speech_threshold = (noise_floor * SPEECH_ABOVE_FLOOR)
                    .clamp(MIN_SPEECH_RMS, MAX_SPEECH_GATE);

                // Periodic level readout (~every 10s) — cheap, and invaluable
                // for diagnosing "why isn't the wake word triggering" against
                // real mic/fan conditions.
                if tick_count % 25 == 0 {
                    eprintln!(
                    "[voice-loop] rms {recent_rms:.5} / floor {noise_floor:.5} / gate {speech_threshold:.5}"
                );
                }

                let control = app.state::<VoiceLoopControl>();
                if !control.enabled.load(Ordering::Relaxed)
                    || control.paused.load(Ordering::Relaxed)
                {
                    continue;
                }
                let Some(voice_state) = app.try_state::<VoiceState>() else {
                    continue;
                };

                // Double-clap to summon the window. Cheap energy pre-gate (claps
                // are loud), then transient analysis. Not while Jarvis is
                // speaking (playback would confuse it).
                if !voice_state.is_speaking() && recent_rms > 0.04 {
                    let clip = {
                        let r = ring.lock().unwrap();
                        r.last_secs(CLAP_WINDOW_SECS)
                    };
                    let debounced = last_clap.map_or(true, |t| t.elapsed() > Duration::from_secs(3));
                    if debounced && is_double_clap(&clip) {
                        last_clap = Some(Instant::now());
                        eprintln!("[voice-loop] double-clap → summoning window");
                        // Window ops MUST run on the main thread — calling
                        // show/focus from this background thread corrupts the
                        // GTK/Wayland connection and crashes the app.
                        let app_main = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if let Some(win) = app_main.get_webview_window("main") {
                                let _ = win.unminimize();
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        });
                        // Consume the claps so they don't also read as a command.
                        let mut r = ring.lock().unwrap();
                        r.clear();
                        last_checked_total = r.total_pushed;
                        continue;
                    }
                }

                if voice_state.is_speaking() {
                    // Listen for the "stop" interrupt word while Jarvis talks.
                    // The mic picks up Jarvis's own playback, so the energy gate
                    // passes constantly here — require fresh audio between
                    // checks (same rule as wake checks) or this branch runs
                    // whisper nearly back-to-back for the whole reply, on top of
                    // generation + synthesis, and the UI visibly lags.
                    let total = ring.lock().unwrap().total_pushed;
                    let new_samples = total.saturating_sub(last_checked_total);
                    if recent_rms > speech_threshold
                        && (new_samples as f32) >= MIN_NEW_AUDIO_SECS * SAMPLE_RATE as f32
                    {
                        last_checked_total = total;
                        let clip = {
                            let r = ring.lock().unwrap();
                            r.last_secs(1.6)
                        };
                        if let Ok(text) = voice::transcribe_impl(&voice_state, clip, true) {
                            // The mic hears Jarvis's OWN voice here, and this
                            // check once matched a "stop" inside her reply and
                            // silenced her mid-sentence. A human interrupting
                            // says a short burst ("stop", "jarvis stop"); her
                            // own speech transcribes as long sentences — so
                            // require a short utterance, not just the word.
                            let word_count = text
                                .split(|c: char| !c.is_alphanumeric())
                                .filter(|w| !w.is_empty())
                                .count();
                            if contains_word(&text, "stop") && word_count <= 3 {
                                eprintln!("[voice-loop] stop word heard: {text:?}");
                                voice::stop_speaking_impl(&voice_state);
                                ring.lock().unwrap().clear();
                            }
                        }
                    }
                    continue;
                }

                // Idle: wake-word check, gated on energy + enough new audio.
                let (clip, total) = {
                    let r = ring.lock().unwrap();
                    (r.last_secs(WAKE_WINDOW_SECS), r.total_pushed)
                };
                let new_samples = total.saturating_sub(last_checked_total);
                if recent_rms < speech_threshold
                    || (new_samples as f32) < MIN_NEW_AUDIO_SECS * SAMPLE_RATE as f32
                {
                    continue;
                }
                last_checked_total = total;

                // Wake check: try the fast (tiny) model first — it's cheap and
                // usually enough. But tiny frequently mangles the proper noun
                // "Jarvis" (heard "nervous"/"George" in practice), so if it
                // doesn't catch a wake alias, fall back to the accurate model
                // before giving up. That accurate pass is the difference
                // between "she never wakes" and reliable triggering.
                let Ok(fast_heard) = voice::transcribe_impl(&voice_state, clip.clone(), true) else {
                    continue;
                };
                // base.en (the wake model) is reliable at "Jarvis", so we no
                // longer fall back to the large turbo+beam command model here —
                // that fallback ran on every noisy clip and pinned the CPU.
                if !heard_wake_word(&fast_heard) {
                    continue;
                }
                let heard = fast_heard;
                eprintln!("[voice-loop] wake word detected in: {heard:?}");

                let _ = app.emit("voice-status", serde_json::json!({ "status": "listening" }));

                // Was the command spoken in the same breath as the wake word?
                // Prefer extracting it from the accurate model's transcription,
                // but fall back to the fast model's text: the accurate model
                // sometimes garbles the name differently (real example: tiny
                // heard "Hey Jarvis, hows the weather?" while base heard
                // "Hey, George.") and losing the wake word there used to
                // silently discard a perfectly good command.
                let accurate =
                    voice::transcribe_impl(&voice_state, clip, false).unwrap_or_default();
                let inline = text_after_wake_word(&accurate, true)
                    .filter(|cmd| cmd.len() >= 3)
                    .or_else(|| text_after_wake_word(&heard, false).filter(|cmd| cmd.len() >= 3))
                    .unwrap_or_default();

                let command = if inline.len() >= 3 {
                    inline
                } else {
                    // User paused after "Jarvis" — capture until they finish.
                    ring.lock().unwrap().clear();
                    let started = Instant::now();
                    let mut heard_speech = false;
                    let mut quiet_since: Option<Instant> = None;
                    loop {
                        thread::sleep(Duration::from_millis(200));
                        if started.elapsed() > COMMAND_MAX {
                            break;
                        }
                        let tail = {
                            let r = ring.lock().unwrap();
                            r.last_secs(0.7)
                        };
                        let tail_rms = rms(&tail);
                        if tail_rms > speech_threshold {
                            heard_speech = true;
                            quiet_since = None;
                        } else if heard_speech && tail_rms < noise_floor * SILENCE_BELOW_FLOOR {
                            let since = quiet_since.get_or_insert_with(Instant::now);
                            if since.elapsed() >= COMMAND_SILENCE {
                                break;
                            }
                        } else {
                            // In-between zone (louder than silence, quieter than
                            // speech): neither counts toward the silence timer
                            // nor resets it — just wait for a clearer signal.
                            quiet_since = None;
                        }
                    }
                    let audio: Vec<f32> = {
                        let r = ring.lock().unwrap();
                        r.buf.iter().copied().collect()
                    };
                    voice::transcribe_impl(&voice_state, audio, false).unwrap_or_default()
                };

                let _ = app.emit("voice-status", serde_json::json!({ "status": "idle" }));
                let command = command.trim().to_string();
                // Whisper renders non-speech as bracketed annotations ("[BLANK_AUDIO]",
                // "(upbeat music)") or stray punctuation — only forward something
                // that actually contains words.
                let has_substance = command.len() >= 3
                    && command.chars().any(|c| c.is_alphanumeric())
                    && !command.starts_with('[')
                    && !command.starts_with('(');
                if has_substance {
                    // Route to whichever persona was addressed by the wake word.
                    let ai = if woke_as_hacks(&accurate) || woke_as_hacks(&heard) {
                        "hacks"
                    } else {
                        "jarvis"
                    };
                    eprintln!("[voice-loop] command ({ai}): {command:?}");
                    let _ = app.emit("voice-command", serde_json::json!({ "text": command, "ai": ai }));
                }
                {
                    let mut r = ring.lock().unwrap();
                    r.clear();
                    last_checked_total = r.total_pushed;
                }
            }
        }
    });
}
