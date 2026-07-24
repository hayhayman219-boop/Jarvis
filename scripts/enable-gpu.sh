#!/usr/bin/env bash
# Enables the NVIDIA RTX 3050 for Ollama so Jarvis/Hacks run on the GPU
# (~10x faster than CPU). Run with sudo:  sudo bash scripts/enable-gpu.sh
# Then REBOOT.  After reboot, `nvidia-smi` should work and `ollama ps` should
# show the model with a GPU percentage.
set -e

if [ "$EUID" -ne 0 ]; then
  echo "Please run with sudo:  sudo bash scripts/enable-gpu.sh"
  exit 1
fi

echo "==> 1/3  Updating the system, then installing the NVIDIA driver..."
apt update
# Bring the kernel + packages current FIRST, so the NVIDIA DKMS module builds
# against the kernel you'll actually boot into after the reboot below.
apt full-upgrade -y
# system76-driver-nvidia pulls the right driver and wires it into Pop's power
# management. If that package is unavailable, fall back to the recommended one.
if apt-get install -y system76-driver-nvidia; then
  echo "    installed system76-driver-nvidia"
else
  echo "    falling back to ubuntu-drivers autoinstall"
  apt-get install -y ubuntu-drivers-common
  ubuntu-drivers autoinstall
fi

echo "==> 2/3  Tuning Ollama to fit the 7B model in 4GB VRAM..."
# Flash attention + an 8-bit KV cache roughly halve the memory the context
# uses, letting far more of the model's layers live on the GPU.
mkdir -p /etc/systemd/system/ollama.service.d
cat > /etc/systemd/system/ollama.service.d/gpu.conf <<'EOF'
[Service]
Environment="OLLAMA_FLASH_ATTENTION=1"
Environment="OLLAMA_KV_CACHE_TYPE=q8_0"
EOF
systemctl daemon-reload
echo "    wrote /etc/systemd/system/ollama.service.d/gpu.conf"

echo "==> 3/3  Done. REBOOT now for the driver to load:  sudo reboot"
echo "    After reboot, run:  nvidia-smi   and   ollama ps"
