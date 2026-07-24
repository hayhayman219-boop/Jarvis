#!/usr/bin/env python3
"""Always-on double-clap listener that LAUNCHES Jarvis when he's fully closed.

Jarvis's own double-clap handler only works while he's running (it just brings
the window forward). This tiny daemon runs independently — so a double clap can
start the app from a fully-closed state. It's deliberately lightweight: raw-PCM
transient detection, no ML, ~1% CPU.

On a double clap it runs `systemctl --user start jarvis2-app`, which launches
Jarvis if he's closed and is a harmless no-op if he's already running (in which
case Jarvis's own handler shows the window).
"""
import array
import os
import subprocess
import time

RATE = 16000
FRAME = 480              # 30 ms @ 16 kHz
FRAME_BYTES = FRAME * 2  # s16le
CLAP_THRESH = float(os.environ.get("CLAP_THRESH", "0.08"))  # claps are loud
ATTACK = 3.0             # a clap is a sharp rise from the previous frame
MIN_GAP = 0.09           # inter-clap spacing bounds (seconds)
MAX_GAP = 0.70
DEBOUNCE = 3.0           # ignore repeats for this long after a trigger
REFRACTORY = 0.09        # a single clap can span a couple of frames
SETTLE = 0.35            # after the 2nd clap, wait this long to confirm it's a
                         # double clap and not the start of applause (a 3rd clap)
WINDOW = 1.2             # only consider onsets this recent


def frame_rms(buf: bytes) -> float:
    a = array.array("h")
    a.frombytes(buf)
    if not a:
        return 0.0
    total = 0
    for v in a:
        total += v * v
    return (total / len(a)) ** 0.5 / 32768.0


def start_capture():
    return subprocess.Popen(
        ["parecord", "--rate=%d" % RATE, "--channels=1",
         "--format=s16le", "--raw", "--latency-msec=30"],
        stdout=subprocess.PIPE,
    )


def launch_jarvis():
    subprocess.run(
        ["systemctl", "--user", "start", "jarvis2-app"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def main():
    proc = start_capture()
    prev = 0.0
    burst = []                # onset timestamps in the current run of claps
    last_onset = 0.0
    last_trigger = 0.0
    refractory_until = 0.0
    print("[clap-daemon] listening for double-claps", flush=True)

    while True:
        buf = proc.stdout.read(FRAME_BYTES)
        if not buf or len(buf) < FRAME_BYTES:
            # Capture ended (audio stack restarted?) — reopen after a beat.
            time.sleep(1.0)
            try:
                proc.terminate()
            except Exception:
                pass
            proc = start_capture()
            prev = 0.0
            burst = []
            continue

        r = frame_rms(buf)
        now = time.monotonic()
        # Register a clap onset: loud + a sharp rise, outside the refractory of
        # the previous one.
        if now >= refractory_until and r > CLAP_THRESH and r > prev * ATTACK:
            burst.append(now)
            last_onset = now
            refractory_until = now + REFRACTORY
        prev = r

        # A burst ends once it's been quiet for SETTLE seconds. Evaluate only
        # then: fire ONLY if the whole burst was exactly two well-spaced claps.
        # Applause (>2) and single claps (1) are rejected by construction.
        if burst and (now - last_onset) >= SETTLE:
            if len(burst) == 2:
                gap = burst[1] - burst[0]
                if (MIN_GAP <= gap <= MAX_GAP
                        and now - last_trigger > DEBOUNCE):
                    print("[clap-daemon] double-clap -> launching Jarvis", flush=True)
                    launch_jarvis()
                    last_trigger = now
            burst = []


if __name__ == "__main__":
    main()
