#!/usr/bin/env bash
# This machine's PipeWire graph occasionally wedges (observed repeatedly,
# including right after boot): every app's playback stream connects but
# never drains, so all sound silently hangs. Symptoms in Jarvis: no voice
# plus "[tts] playback hung" lines in `journalctl --user -u jarvis-app`.
# This clears it, then restarts Jarvis so its always-on mic stream (killed
# by the audio restart) comes back.
set -euo pipefail

echo "==> Restarting PipeWire audio stack..."
systemctl --user restart pipewire pipewire-pulse wireplumber
sleep 2

if systemctl --user is-active --quiet jarvis-app; then
  echo "==> Restarting Jarvis..."
  systemctl --user stop jarvis-app
  sleep 1
  systemctl --user reset-failed jarvis-app 2>/dev/null || true
  systemd-run --user --unit=jarvis-app "$(dirname "$0")/../src-tauri/target/release/jarvis"
fi


# Force the Headphones profile so the aux-jack speakers work (SOF auto-switch is flaky here).
pactl set-default-sink "alsa_output.usb-0c76_USB_PnP_Audio_Device-00.analog-stereo" 2>/dev/null || true
pactl set-card-profile alsa_card.pci-0000_00_1f.3-platform-skl_hda_dsp_generic "HiFi (HDMI1, HDMI2, HDMI3, Headphones, Mic1, Mic2)" 2>/dev/null || true
# Keep the (quiet) internal mic boosted to 150%.
wpctl set-volume @DEFAULT_AUDIO_SINK@ 0.8 2>/dev/null || true
wpctl set-volume @DEFAULT_AUDIO_SOURCE@ 1.3 2>/dev/null || true
echo "==> Done. Audio should be flowing again."
