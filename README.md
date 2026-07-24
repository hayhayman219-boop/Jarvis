# Jarvis

A personal, **local-first** AI assistant styled after Tony Stark's JARVIS. The
"brain" is a local LLM (via [Ollama](https://ollama.com)); speech-to-text
(Whisper) and text-to-speech (Piper) run entirely on your machine; and the UI
is an ambient HUD dashboard with a movable chat overlay.

Everything runs offline except the optional integrations you choose to connect
(weather, calendar, Notion, etc.). No data is sent anywhere you don't set up
yourself, and every API key/token you enter is stored **only** in a local
settings file on your machine — never in this repo.

## Features

- **Local LLM chat** via Ollama, streamed token-by-token, spoken aloud.
- **Voice** — always-on wake words ("Hey Jarvis" / "Hey Hacks"), local Whisper
  STT, local Piper TTS with a distinct voice per assistant.
- **Double-clap** to bring the window to the front.
- **Sub AIs** — Jarvis is the main assistant; add specialist personas (ships
  with "Hacks", an Iron Man expert) with their own voice, colour, and skills.
- **HUD dashboard** — weather, schedule, reminders, live CPU/RAM/disk/temp
  gauges, and pop-up screens (webcam, Notion, checklists, comic covers…).
- **Actions Jarvis can actually do**: control Chrome, adjust volume/brightness,
  lock the screen, take screenshots, add items to shopping carts, set
  timers/alarms, read your screen (OCR), give a spoken daily briefing, and save
  any reply to a PDF.
- **Optional GPU acceleration** (NVIDIA) for much faster responses.
- **Web mode** — the same backend served over your LAN so you can reach Jarvis
  from your phone.

---

## Setup

Tested on Linux (Pop!_OS / Ubuntu). macOS and Windows are supported by Tauri;
where steps differ they're noted.

### 1. Install prerequisites

- **[Rust](https://rustup.rs/)** and **Node.js 18+ / npm**.
- **[Ollama](https://ollama.com)** (the local LLM runtime).
- **Tesseract OCR** (only if you want the "read my screen" feature):
  ```bash
  sudo apt install tesseract-ocr        # Debian/Ubuntu
  # macOS: brew install tesseract   |   Windows: install from UB-Mannheim build
  ```
- **Linux build dependencies for Tauri** (Debian/Ubuntu):
  ```bash
  sudo apt install -y libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev pkg-config libasound2-dev \
    libclang-dev cmake
  ```

### 2. Clone and install

```bash
git clone https://github.com/<your-username>/Jarvis.git
cd Jarvis
npm install
```

### 3. Download the voice models

These are large (Whisper + Piper), so they're **not** in the repo. Fetch them
with the included script:

```bash
bash scripts/download-voice-assets.sh
# For the most accurate voice recognition, also grab the large model (~1.6GB):
TURBO=1 bash scripts/download-voice-assets.sh
```

This populates `src-tauri/resources/whisper/` (STT models) and
`src-tauri/resources/piper/` (the Piper engine + Jenny and Ryan voices). On
macOS/Windows, replace the Piper Linux download in the script with the matching
build from the [Piper releases](https://github.com/rhasspy/piper/releases).

### 4. Pull an LLM

```bash
ollama pull qwen2.5:3b       # fast, great on a 4GB GPU or modern CPU
# or a bigger model if you have the VRAM/CPU for it, e.g. qwen2.5:7b
```

### 5. Run it

```bash
npm run tauri dev            # development
# or a production build:
npm run tauri build -- --no-bundle
./src-tauri/target/release/jarvis
```

On first launch, open **Settings (⚙)** and pick your model from the dropdown
(it lists what Ollama has). That's the minimum to start chatting.

---

## Optional: GPU acceleration (NVIDIA)

If you have an NVIDIA GPU, running the LLM on it is dramatically faster than
CPU. On Pop!_OS/Ubuntu:

```bash
sudo bash scripts/enable-gpu.sh   # installs the driver + tunes Ollama
sudo reboot
```

After rebooting, `ollama ps` should show your model running on the GPU. On a
4GB card, a 3B model fits entirely in VRAM; larger models partially offload.

## Optional: autostart on boot (Linux)

`scripts/install-desktop-app.sh` sets up a user systemd service so Jarvis
launches at login and installs an app-menu launcher. Review it first — it's
tailored to a systemd user session.

Closing the window **hides** Jarvis (he keeps running in the background so the
wake word and double-clap still work). To also let a **double-clap launch him
when he's fully closed**, install the tiny clap-listener daemon:

```bash
cp scripts/jarvis-clap-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now jarvis-clap-daemon.service
```

It listens for a double clap (via `parecord`) and starts the app — a harmless
no-op if Jarvis is already running.

---

## Connecting integrations

All keys/tokens go in **Settings** inside the app and are saved locally to
`~/.local/share/com.jarvis.assistant/settings.json` (never committed). Each one
is optional.

| Integration | What you need | Where to get it |
|---|---|---|
| **Weather** | Your city | Type it in Settings → Location (geocoded via free Open-Meteo). |
| **Notion** | Internal integration token (`ntn_…`) | notion.so/my-integrations, then share each page with the integration. |
| **Google Calendar** | "Secret iCal" URL(s) | Google Calendar → Settings → *Integrate calendar* → Secret address in iCal format. Paste one per line. |
| **Apple Calendar/Reminders** | Apple ID + app-specific password | appleid.apple.com → Sign-In and Security → App-Specific Passwords. |
| **Comic Vine** (Hacks) | Free API key | comicvine.gamespot.com/api — enables real comic data + cover art. |
| **News monitor** | — | Toggle categories in Settings; watches feeds while idle. |
| **Home Assistant** | Long-lived access token + URL | HA → Profile → Long-Lived Access Tokens. |

---

## Using Jarvis

- **Chat**: click the arc-reactor core (or say a wake word). Two movable panels —
  one per AI — each show only their own conversation.
- **Wake words**: say **"Hey Jarvis, …"** for the main assistant or
  **"Hey Hacks, …"** to address the Iron Man expert. Whoever you name answers.
- **Double-clap** anywhere to bring the window forward.
- **Stop speech**: the ⏹ button, or say "stop".
- **Voice/text commands** (examples):
  - "What's the weather?" · "What's on my calendar today?"
  - "Set a timer for 10 minutes" · "Set an alarm for 7:30am"
  - "Good morning" / "brief me" → spoken daily briefing
  - "Read my screen" / "what does this say?" → screen OCR
  - "Open the news" · "search YouTube for …" · "add AAA batteries to my Amazon cart"
  - "Save that as a PDF" → exports the last reply to your Downloads
  - **Hacks**: "play the Mark I escape", "show me the cover of Tales of Suspense 39",
    "open my collection"

---

## Web mode (reach Jarvis from your phone)

The same backend can be served over your LAN:

```bash
npm run web
```

Voice needs a secure context, so this serves over **HTTPS with a self-signed
certificate** — browsers show a one-time "not private" warning per device
(Advanced → Proceed). It prints the LAN URL, e.g.
`https://192.168.1.20:4488`. Reminders share the same local database as the
desktop app. Re-run just the server (no frontend rebuild) with `npm run web:server`.

---

## Project layout

- `src/` — React frontend, shared by the desktop and web builds. `src/lib/env.ts`
  detects the platform; `src/lib/apiClient.ts` branches between Tauri `invoke()`
  and browser `fetch()`.
- `src-tauri/src/commands/` — Rust logic (Ollama, weather, reminders, voice,
  system, browser, comics, vision…). Each exposes plain `*_impl` functions reused
  by both the Tauri commands and the web server.
- `src-tauri/src/voice_loop.rs` — the always-on wake-word + double-clap listener.
- `src-tauri/src/bin/server/` — the standalone Axum web server (routing/TLS only).
- `src/data/subAIs.ts` — the AI personas (add your own Sub AI here).
- `scripts/` — voice-asset download, GPU setup, install helpers.

## Privacy

The LLM, STT, and TTS all run locally. Integration keys live only in your local
settings file. This repo intentionally excludes the models, that settings file,
and the reminders/chat database — nothing personal is published.

## License

MIT — see `LICENSE`.
