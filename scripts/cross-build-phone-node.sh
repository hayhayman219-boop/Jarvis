#!/usr/bin/env bash
# Cross-build the headless Jarvis server for an aarch64 postmarketOS phone node,
# so you never have to compile on the phone. Uses Docker buildx + QEMU emulation
# to build a real arm64/musl binary on this x86 machine.
#
# PREREQS (one-time, need your privileges — Docker isn't runnable by the agent):
#   1. Start Docker (Docker Desktop, or `sudo systemctl start docker`).
#   2. Add yourself to the docker group to skip sudo:
#        sudo usermod -aG docker "$USER"   # then log out/in
#   3. That's it — this script registers QEMU + a buildx builder itself.
#
# Output: ./dist-phone/server  (copy it to the phone, see docs/phone-server-setup.md)
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> [1/4] Building web frontend on host (fast, arch-independent)…"
npm install
npm run build

echo "==> [2/4] Registering QEMU arm64 emulation…"
docker run --privileged --rm tonistiigi/binfmt --install arm64

echo "==> [3/4] Ensuring an arm64-capable buildx builder…"
docker buildx create --name jarvis-arm --use 2>/dev/null || docker buildx use jarvis-arm
docker buildx inspect --bootstrap >/dev/null

echo "==> [4/4] Building aarch64 server image (emulated — this takes a while)…"
docker buildx build --platform linux/arm64 \
  -f docker/phone-node.Dockerfile --target artifact \
  -o type=local,dest=dist-phone .

echo ""
echo "==> Done. aarch64 binary: ./dist-phone/server"
echo "    scp it to the phone and run per docs/phone-server-setup.md (section 4/6)."
echo "    Runtime libs on the phone: apk add openssl   (tauri/webkit no longer needed)"
