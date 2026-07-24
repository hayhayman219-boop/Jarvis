#!/usr/bin/env bash
# Refresh the deep-security baselines after intentional system changes
# (OS updates, new software) so rkhunter/AIDE don't flag your own updates
# as tampering. Run with sudo.
set -euo pipefail
if [ "$(id -u)" -ne 0 ]; then echo "Run me with sudo: sudo $0" >&2; exit 1; fi
echo "==> Updating rkhunter file-properties baseline..."
rkhunter --propupd --nocolors
echo "==> Rebuilding AIDE database (several minutes)..."
aideinit --yes || aideinit
[ -f /var/lib/aide/aide.db.new ] && cp /var/lib/aide/aide.db.new /var/lib/aide/aide.db
echo "==> Baselines refreshed."
