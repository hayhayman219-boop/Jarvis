#!/usr/bin/env bash
# Builds the standalone release app (frontend embedded in the binary),
# installs an application-menu entry, and registers it to auto-start at
# login — so Jarvis launches like any other desktop app and comes up on its
# own after every reboot, no terminal or dev server involved.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO="$(pwd)"

echo "==> Building production app (frontend embedded)..."
# Must go through the Tauri CLI: a plain `cargo build --release` compiles in
# dev mode and produces a binary that tries to load the vite dev server
# (blank "Could not connect to localhost" window) instead of the embedded
# frontend assets.
npm run tauri build -- --no-bundle

echo "==> Installing desktop entry..."
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/jarvis.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Jarvis
Comment=Personal AI assistant (local LLM, voice, HUD dashboard)
Exec=$REPO/src-tauri/target/release/jarvis
Icon=$REPO/src-tauri/icons/128x128.png
Terminal=false
Categories=Utility;
StartupWMClass=Jarvis
EOF
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

echo "==> Installing login-time audio fix (clears this machine's recurring PipeWire wedge)..."
mkdir -p "$HOME/.config/systemd/user"
cp "$REPO/scripts/jarvis-fix-audio.service" "$HOME/.config/systemd/user/jarvis-fix-audio.service"
systemctl --user daemon-reload
systemctl --user enable jarvis-fix-audio.service

echo "==> Installing auto-start on login..."
cp "$REPO/scripts/jarvis-app.service" "$HOME/.config/systemd/user/jarvis-app.service"
systemctl --user daemon-reload
systemctl --user enable jarvis-app.service

echo "==> Installing boot-time security check..."
cp "$REPO/scripts/jarvis-security-check.service" "$HOME/.config/systemd/user/jarvis-security-check.service"
systemctl --user daemon-reload
systemctl --user enable jarvis-security-check.service

echo "==> Done. Jarvis will launch automatically at your next login, or run now with: systemctl --user restart jarvis-app.service"
