#!/bin/bash
# Setup script for the Claude cloud environment (paste into "設定指令碼").
# Mirrors the Linux job in .github/workflows/ci.yml. Runs once per new session,
# before Claude Code starts, so everything here must be idempotent and quiet on success.
set -euo pipefail

SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"

# Tauri / WebKitGTK build deps, plus xvfb + dbus for the real-app E2E.
export DEBIAN_FRONTEND=noninteractive
$SUDO apt-get update -qq
$SUDO apt-get install -y -qq --no-install-recommends \
  build-essential curl file git pkg-config perl \
  libdbus-1-dev libglib2.0-dev libgtk-3-dev libayatana-appindicator3-dev \
  libssl-dev libwebkit2gtk-4.1-dev libxdo-dev librsvg2-dev patchelf \
  dbus-x11 gnome-keyring xvfb protobuf-compiler

# Bun
if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
fi
export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
export PATH="$BUN_INSTALL/bin:$HOME/.cargo/bin:$PATH"

# Rust: the version comes from rust-toolchain.toml (1.96.0 + clippy + rustfmt).
if ! command -v rustup >/dev/null 2>&1; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
cd "${CLAUDE_PROJECT_DIR:-$(pwd)}"
rustup toolchain install --profile minimal --component clippy,rustfmt >/dev/null

# Make bun/cargo visible to later shells in the session. A session may write about 40 GB, and
# the gate's test binaries with line tables and incremental caches filled it (2026-10-12), so
# builds here keep no debug info or incremental state.
{
  echo "export BUN_INSTALL=\"$BUN_INSTALL\""
  echo "export PATH=\"$BUN_INSTALL/bin:$HOME/.cargo/bin:\$PATH\""
  echo "export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0"
  # The image's Chromium is not the build Playwright pins; E2E launches it by path.
  if [ -x /opt/pw-browsers/chromium ]; then
    echo "export PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/opt/pw-browsers/chromium"
  fi
} >> "$HOME/.bashrc"

# There is no display: run the gate as CI does, `dbus-run-session -- xvfb-run -a bun run check`.
# Not as root: tests that take a file's permissions away to make a read or unlink fail cannot
# fail for root (`durable_io`, `chrome::tests`).

# JS deps and the Playwright browser used by bun run test:e2e.
bun install --frozen-lockfile
bunx playwright install --with-deps chromium

# Warm the Rust dependency cache so the first `bun run check` doesn't pay for it.
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
