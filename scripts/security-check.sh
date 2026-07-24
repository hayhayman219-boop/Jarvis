#!/usr/bin/env bash
# Boot-time security sweep for this laptop, run at login by
# jarvis-security-check.service. Checks the handful of signals that matter
# on a personal machine, writes a report, raises a desktop notification,
# and — only when something is actually wrong — speaks the alert in
# Jarvis's local voice. Requires no root; every probe degrades gracefully.
set -uo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
DATA_DIR="$HOME/.local/share/com.jarvis.assistant"
REPORT="$DATA_DIR/security-report.txt"
BASELINE="$DATA_DIR/security-ports-baseline.txt"
mkdir -p "$DATA_DIR"

FINDINGS=()
note() { FINDINGS+=("$1"); }

{
  echo "Jarvis security check — $(date)"
  echo "======================================================"

  # 1. Remote login sessions (anything not the local seat).
  echo; echo "-- Active sessions --"
  who
  REMOTE_SESSIONS=$(who | grep -vE '\(:[0-9]\)|tty|seat' | grep -cE '\([0-9a-fA-F:.]+\)' || true)
  [ "${REMOTE_SESSIONS:-0}" -gt 0 ] && note "$REMOTE_SESSIONS remote login session(s) active"

  # 2. Failed authentication attempts this boot.
  echo; echo "-- Failed logins (this boot) --"
  FAILED=$(journalctl -b -q --no-pager 2>/dev/null | grep -icE "failed password|authentication failure" || true)
  echo "count: ${FAILED:-0}"
  [ "${FAILED:-0}" -gt 5 ] && note "${FAILED} failed login attempts since boot"

  # 3. Listening ports vs. known baseline (new ports = new attack surface,
  #    or malware phoning home with an open backdoor). Ports in the kernel's
  #    ephemeral range (>=32768) are excluded — they're randomly assigned,
  #    churn on every boot, and would false-alarm constantly.
  echo; echo "-- Listening ports --"
  CURRENT_PORTS=$(ss -tulnH 2>/dev/null | awk '{print $1, $5}' | sed 's/.*[]:]//; s/^/port /' | sort -u | awk '$2 < 32768')
  ss -tulpnH 2>/dev/null | awk '{print $1, $5, $7}' | sort -u
  if [ -f "$BASELINE" ]; then
    NEW_PORTS=$(comm -13 <(sort -u "$BASELINE") <(echo "$CURRENT_PORTS") || true)
    if [ -n "$NEW_PORTS" ]; then
      echo "NEW since baseline:"; echo "$NEW_PORTS"
      note "new listening port(s) since baseline: $(echo "$NEW_PORTS" | tr '\n' ' ')"
    else
      echo "(matches baseline)"
    fi
  else
    echo "$CURRENT_PORTS" > "$BASELINE"
    echo "(baseline recorded — future checks flag additions)"
  fi

  # 4. Processes executing from volatile/world-writable locations — a
  #    common malware pattern; legitimate software essentially never does it.
  echo; echo "-- Processes running from /tmp or /dev/shm --"
  SUSPICIOUS=$(ls -l /proc/[0-9]*/exe 2>/dev/null | grep -E ' -> (/tmp/|/dev/shm/|/var/tmp/)' || true)
  if [ -n "$SUSPICIOUS" ]; then
    echo "$SUSPICIOUS"
    note "process(es) running from a temp directory"
  else
    echo "(none)"
  fi

  # 5. Firewall.
  echo; echo "-- Firewall --"
  FW="inactive"
  systemctl is-active --quiet ufw 2>/dev/null && FW="ufw"
  systemctl is-active --quiet firewalld 2>/dev/null && FW="firewalld"
  echo "state: $FW"
  [ "$FW" = "inactive" ] && note "no firewall is active"

  # 6. Pending security updates (uses cached package lists; no root needed).
  echo; echo "-- Pending security updates --"
  SEC_UPDATES=$(apt-get -s dist-upgrade 2>/dev/null | grep -c "^Inst.*ecurity" || true)
  echo "count: ${SEC_UPDATES:-0}"
  [ "${SEC_UPDATES:-0}" -gt 0 ] && note "${SEC_UPDATES} pending security update(s)"

  # 7. Deep scans (rkhunter/AIDE via system timers, if set up with
  #    setup-deep-security.sh) — fold in their latest reports.
  DEEP_DIR="/var/lib/jarvis-security"
  if [ -f "$DEEP_DIR/rkhunter-last.txt" ]; then
    echo; echo "-- Rootkit scan (rkhunter, last run) --"
    tail -3 "$DEEP_DIR/rkhunter-last.txt"
    RK_WARN=$(grep -c "Warning" "$DEEP_DIR/rkhunter-last.txt" || true)
    [ "${RK_WARN:-0}" -gt 0 ] && note "rootkit scanner raised ${RK_WARN} warning(s) — see $DEEP_DIR/rkhunter-last.txt"
  fi
  if [ -f "$DEEP_DIR/aide-last.txt" ]; then
    echo; echo "-- File integrity (AIDE, last run) --"
    grep -E "Number of entries|Added|Removed|Changed entries|scanned:" "$DEEP_DIR/aide-last.txt" | head -6
    if ! grep -q "found no differences\|Looks okay" "$DEEP_DIR/aide-last.txt"; then
      AIDE_CHANGED=$(grep -oE "Changed entries:[[:space:]]*[0-9]+" "$DEEP_DIR/aide-last.txt" | grep -oE "[0-9]+" || echo "?")
      note "file-integrity check found changes (${AIDE_CHANGED} entries) — expected after updates; refresh with sudo scripts/update-security-baselines.sh, otherwise investigate $DEEP_DIR/aide-last.txt"
    fi
    # Staleness guard: a silent, never-running scanner looks like safety.
    AIDE_AGE_DAYS=$(( ( $(date +%s) - $(stat -c %Y "$DEEP_DIR/aide-last.txt") ) / 86400 ))
    [ "$AIDE_AGE_DAYS" -gt 9 ] && note "file-integrity report is ${AIDE_AGE_DAYS} days old (weekly timer may be off)"
  fi

  echo; echo "-- Verdict --"
  if [ ${#FINDINGS[@]} -eq 0 ]; then
    echo "All clear."
  else
    printf '%s\n' "${FINDINGS[@]}"
  fi
} > "$REPORT" 2>&1

# Surface the result.
if [ ${#FINDINGS[@]} -eq 0 ]; then
  notify-send -i security-high "Jarvis security check" "All clear. Report: $REPORT" 2>/dev/null || true
else
  SUMMARY=$(printf '%s; ' "${FINDINGS[@]}")
  notify-send -u critical -i security-low "Jarvis security check" "$SUMMARY Report: $REPORT" 2>/dev/null || true
  # Speak only when something needs attention — via the local Piper voice so
  # this works even when the Jarvis app (or the internet) isn't up yet.
  PIPER="$REPO/src-tauri/resources/piper/piper"
  VOICE="$REPO/src-tauri/resources/piper/voices/en_GB-alan-medium.onnx"
  if [ -x "$PIPER" ] && [ -f "$VOICE" ]; then
    WAV=$(mktemp --suffix=.wav)
    SPOKEN="Security check complete. ${#FINDINGS[@]} item(s) need attention: ${FINDINGS[*]}"
    LD_LIBRARY_PATH="$(dirname "$PIPER")" "$PIPER" --model "$VOICE" --output_file "$WAV" --quiet <<< "$SPOKEN" 2>/dev/null \
      && timeout 60 pw-play "$WAV" 2>/dev/null
    rm -f "$WAV"
  fi
fi

exit 0
