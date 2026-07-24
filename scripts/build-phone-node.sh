#!/usr/bin/env bash
# Builds the lean, headless Jarvis server for a phone node (postmarketOS/ARM).
# Drops whisper (C++) + cpal (ALSA) via --no-default-features, so it compiles
# fast and doesn't need audio/voice libraries. Run this ON the phone, or use it
# as the cargo invocation inside an aarch64 cross-build container.
#
# The desktop app is unaffected: it builds normally with `voice` on by default.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR"

echo "==> Building web frontend (served by the node)…"
npm install
npm run build

echo "==> Building headless server (no voice / whisper / audio)…"
cargo build --release --bin server --no-default-features \
  --manifest-path src-tauri/Cargo.toml

echo "==> Done: src-tauri/target/release/server"
echo "    Run it with your settings.json in ~/.local/share/com.jarvis.assistant/"
