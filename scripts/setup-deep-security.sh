#!/usr/bin/env bash
# One-time deep-security setup — run with sudo:
#   sudo ~/Jarvis/scripts/setup-deep-security.sh
#
# Installs and baselines:
#   - rkhunter (rootkit scanner) — scans ~3 minutes after every boot
#   - AIDE (file-integrity monitor) — full check weekly
# Reports land in /var/lib/jarvis-security/ (world-readable), where the
# login-time sweep (security-check.sh) folds them into its notification and
# spoken alert.
#
# NOTE: the AIDE baseline build scans the whole filesystem — expect this
# script to take 5-15 minutes on first run.
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "Run me with sudo: sudo $0" >&2
  exit 1
fi

REPO="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="/var/lib/jarvis-security"
mkdir -p "$OUT_DIR"
chmod 755 "$OUT_DIR"

# Lock timeout: the desktop's package service (packagekitd / COSMIC Store)
# periodically holds the apt lock; without a timeout apt fails instantly.
# ForceIPv4: this machine's IPv6 routing comes and goes, and the Pop!_OS
# mirror advertises IPv6 addresses — apt burned through eight unreachable
# v6 endpoints and hard-failed instead of falling back to v4.
APT="apt-get -o DPkg::Lock::Timeout=180 -o Acquire::ForceIPv4=true"

echo "==> Refreshing package lists (waits up to 3min if another updater holds the lock)..."
# Non-fatal: rkhunter/aide are in Ubuntu's base repos and resolve from the
# cached lists even if a mirror refresh fails.
$APT update || echo "WARN: some repositories failed to refresh; continuing with cached lists"

echo "==> Installing rkhunter and AIDE..."
DEBIAN_FRONTEND=noninteractive $APT install -y rkhunter aide aide-common

echo "==> Baselining rkhunter file properties..."
rkhunter --propupd --nocolors || true

echo "==> Building AIDE database (this is the slow part)..."
# -y: overwrite an existing db; -f: overwrite existing db.new.
aideinit -y -f
# aideinit leaves the fresh db as aide.db.new; promote it.
if [ -f /var/lib/aide/aide.db.new ]; then
  cp /var/lib/aide/aide.db.new /var/lib/aide/aide.db
fi

echo "==> Installing scan services and timers..."
install -m 644 "$REPO/scripts/system-units/jarvis-rkhunter.service" /etc/systemd/system/
install -m 644 "$REPO/scripts/system-units/jarvis-rkhunter.timer" /etc/systemd/system/
install -m 644 "$REPO/scripts/system-units/jarvis-aide.service" /etc/systemd/system/
install -m 644 "$REPO/scripts/system-units/jarvis-aide.timer" /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now jarvis-rkhunter.timer jarvis-aide.timer

echo "==> Running the first rootkit scan now (1-3 minutes)..."
systemctl start jarvis-rkhunter.service || true

echo
echo "==> Done. Reports: $OUT_DIR/{rkhunter-last.txt,aide-last.txt}"
echo "    The login security sweep now includes these automatically."
echo "    After big system updates, refresh baselines with:"
echo "      sudo $REPO/scripts/update-security-baselines.sh"
