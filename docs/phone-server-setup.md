# Jarvis Phone Node — OnePlus 6T Setup

Turn a **OnePlus 6T** (codename `fajita`, Snapdragon 845, 8GB RAM) into an
always-on Linux server that runs Jarvis's **HTTP service layer** — news,
weather, calendar (Google/Apple), reminders, Notion, Home Assistant — reachable
from any device on your network even when the PC is off.

The heavy code (Ollama LLM, whisper voice) stays on the desktop; the phone runs
the lean **headless server** built with the `voice` feature turned off.

---

## 0. What runs where

| Piece | Where | Why |
|---|---|---|
| HTTP API + web UI (news, weather, calendar, reminders, Notion, HA) | **Phone** | Light, always-on |
| Ollama LLM ("the brain") | **PC / cloud** | Too heavy for a phone |
| Voice (whisper turbo, piper) | **PC** | Too heavy; not needed headless |

Chat endpoints on the phone need an Ollama reachable at `OLLAMA_HOST` — point
them at the PC's Ollama (`http://<pc-ip>:11434`) or a cloud endpoint. Everything
non-LLM works standalone on the phone.

---

## 1. Buy the phone (~$130–180 used)

- Model: **OnePlus 6T, 8GB/128GB**, codename **fajita**.
- **Must be bootloader-unlockable** — avoid **carrier-locked** units (T-Mobile
  OnePlus 6T especially can be locked). "Unlocked / international" is safest.
- Sources: Swappa, Back Market, eBay (check seller notes for "bootloader
  unlockable / unlocked").
- Shortcut: some vendors sell it **pre-flashed with postmarketOS** if you want
  to skip section 2.

## 2. Flash postmarketOS

Follow the official wiki page for `oneplus-fajita`:
<https://wiki.postmarketos.org/wiki/OnePlus_6T_(oneplus-fajita)>

Summary:
1. On the PC: `pip install pmbootstrap`, then `pmbootstrap init`
   - Vendor `oneplus`, device `fajita`, UI `none` (headless server — no desktop
     needed), enable SSH.
2. Unlock the bootloader: boot to fastboot (Vol-Up + power), `fastboot oem unlock`.
3. `pmbootstrap install`, then `pmbootstrap flasher flash_rootfs` and
   `flash_kernel`.
4. Boot; it comes up with SSH. `ssh user@<phone-ip>`.

> Tip: with UI `none` the phone is a pure server. You can run it "batteryless"
> on USB power for a permanent node.

## 3. Install build/runtime deps (on the phone, Alpine/apk)

```sh
sudo apk add rust cargo nodejs npm git openssl-dev pkgconf build-base
```

Since `tauri` is decoupled from the server, **no GTK / WebKit / ALSA / whisper
deps are needed** — the headless build is pure Rust + SQLite (bundled) + OpenSSL.
(If you cross-build instead of building here, the only runtime lib is `openssl`.)

## 4. Get the code + build the headless server

```sh
git clone <your Jarvis2 repo>   # or scp the Jarvis2 folder over
cd Jarvis2

# Build the web frontend (served by the node)
npm install && npm run build

# Build the LEAN server — no voice, no whisper C++, no audio:
cargo build --release --bin server --no-default-features \
  --manifest-path src-tauri/Cargo.toml
```

The binary lands at `src-tauri/target/release/server`.

> The `--no-default-features` flag is the whole point: it drops `whisper-rs`
> (C++) and `cpal` (ALSA), which are the slow/painful parts to build on ARM.
> The desktop app builds normally (voice on by default) — this only changes the
> phone build.

## 5. Credentials on the phone

The server reads the same `settings.json` the desktop app uses:

```
~/.local/share/com.jarvis.assistant/settings.json
```

Copy yours over (or recreate it) with the keys you use: `notionToken`,
`appleId` / `appleAppPassword`, `googleCalUrls`, Home Assistant token, and — for
chat — set the LLM target (Ollama) to a reachable host.

## 6. Run it as a service (OpenRC)

Create `/etc/init.d/jarvis-node`:

```sh
#!/sbin/openrc-run
name="jarvis-node"
command="/home/user/Jarvis2/src-tauri/target/release/server"
command_background=true
pidfile="/run/jarvis-node.pid"
directory="/home/user/Jarvis2"
```

```sh
sudo chmod +x /etc/init.d/jarvis-node
sudo rc-update add jarvis-node default
sudo rc-service jarvis-node start
```

## 7. Reach it

The server listens on its TLS port (self-signed). From another device on the
network, browse to `https://<phone-ip>:<port>/` for the web UI, or hit
`https://<phone-ip>:<port>/api/...` for the JSON endpoints (accept the
self-signed cert once).

Quick check from the PC:

```sh
curl -k https://<phone-ip>:<port>/api/weather?...   # etc.
```

---

## Node #2 (later)

Repeat with a second OnePlus 6T (or a cheaper Poco F1 — same SDM845). Put a
different service set on each, or run one as a hot spare. Budget left from the
first purchase covers it.

## Cross-build (skip on-device compiling)

Build the aarch64 binary on the **PC** instead of the phone, then just copy it
over. Uses Docker buildx + QEMU emulation (Alpine base = matches postmarketOS).

```sh
./scripts/cross-build-phone-node.sh      # → ./dist-phone/server (aarch64)
scp dist-phone/server user@<phone-ip>:~/jarvis-server
```

One-time prereqs (need your privileges): Docker running + your user in the
`docker` group. The script registers QEMU and the buildx builder itself. See
`docker/phone-node.Dockerfile` for the image.

**Runtime lib on the phone** — just OpenSSL (for HTTPS); `tauri`/webkit are gone:

```sh
sudo apk add openssl
```

Notes / caveats:
- With `tauri` decoupled, the emulated build is now **fast** (~5–15 min — only
  axum/reqwest/rusqlite compile, no tauri/webkit/whisper).
- Untested against a real device until the phone's in hand, but the same
  `--no-default-features` build is verified compiling on x86 with **zero**
  tauri/webkit/whisper/gtk in its dependency tree.
