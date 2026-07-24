# Emulated aarch64 build of the headless Jarvis server for a postmarketOS
# (Alpine / musl) phone node. Built via `docker buildx --platform linux/arm64`.
#
# Since `tauri` was decoupled from the server, this build no longer needs GTK /
# WebKit / whisper / audio — just a Rust toolchain, SQLite (bundled) and OpenSSL
# (for reqwest). So it's small and fast even under emulation.
#
# Build (from repo root, after `npm run build` so ./dist exists):
#   docker buildx build --platform linux/arm64 \
#     -f docker/phone-node.Dockerfile --target artifact \
#     -o type=local,dest=dist-phone .
# → produces ./dist-phone/server (aarch64/musl)

FROM alpine:edge AS build

RUN apk add --no-cache \
      rust cargo \
      build-base pkgconf git \
      openssl-dev

WORKDIR /build
COPY . /build

# Frontend is built on the host and copied in (dist/). Fallback: build it here.
RUN if [ ! -d dist ]; then \
      apk add --no-cache nodejs npm && npm ci && npm run build; \
    fi

# Headless build: --no-default-features drops tauri, webkit, whisper and audio.
RUN cargo build --release --bin server --no-default-features \
      --manifest-path src-tauri/Cargo.toml \
 && strip src-tauri/target/release/server

# Export stage: just the binary, so `-o type=local` yields ./dist-phone/server.
FROM scratch AS artifact
COPY --from=build /build/src-tauri/target/release/server /server
