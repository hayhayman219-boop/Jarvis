#!/usr/bin/env bash
# Downloads the local voice models/binaries Jarvis needs (Whisper STT models
# and the Piper TTS engine + voices). These are excluded from git because
# they're large — run this once after cloning.
#
#   bash scripts/download-voice-assets.sh          # base + tiny + small Whisper
#   TURBO=1 bash scripts/download-voice-assets.sh  # also fetch large-v3-turbo
#                                                    (~1.6GB, best accuracy)
set -euo pipefail

cd "$(dirname "$0")/.."
WHISPER_DIR="src-tauri/resources/whisper"
PIPER_DIR="src-tauri/resources/piper"
HF="https://huggingface.co"

mkdir -p "$WHISPER_DIR" "$PIPER_DIR/voices"

echo "==> Downloading Whisper models (wake: base.en, command: small.en)..."
curl -L --progress-bar -o "$WHISPER_DIR/ggml-base.en.bin" \
  "$HF/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin"
curl -L --progress-bar -o "$WHISPER_DIR/ggml-tiny.en.bin" \
  "$HF/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin"
curl -L --progress-bar -o "$WHISPER_DIR/ggml-small.en.bin" \
  "$HF/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin"

if [ "${TURBO:-0}" = "1" ]; then
  echo "==> Downloading large-v3-turbo (~1.6GB, best command accuracy)..."
  curl -L --progress-bar -o "$WHISPER_DIR/ggml-large-v3-turbo.bin" \
    "$HF/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
fi

echo "==> Downloading Piper TTS engine (Linux x86_64)..."
# On macOS/Windows grab the matching build from github.com/rhasspy/piper/releases
TMP_TAR="$(mktemp)"
curl -L --progress-bar -o "$TMP_TAR" \
  "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz"
tar -xzf "$TMP_TAR" -C "src-tauri/resources"
rm -f "$TMP_TAR"

# Voices: Jenny (British female) is Jarvis's voice; Ryan is the Hacks Sub AI.
echo "==> Downloading Piper voices (Jenny + Ryan)..."
dl_voice() { # $1 = lang/name/quality path, $2 = filename stem
  curl -L --progress-bar -o "$PIPER_DIR/voices/$2.onnx" \
    "$HF/rhasspy/piper-voices/resolve/main/en/$1/$2.onnx"
  curl -L --progress-bar -o "$PIPER_DIR/voices/$2.onnx.json" \
    "$HF/rhasspy/piper-voices/resolve/main/en/$1/$2.onnx.json"
}
dl_voice "en_GB/jenny_dioco/medium" "en_GB-jenny_dioco-medium"
dl_voice "en_US/ryan/medium"        "en_US-ryan-medium"

echo "==> Done. Voice assets are in $WHISPER_DIR and $PIPER_DIR"
