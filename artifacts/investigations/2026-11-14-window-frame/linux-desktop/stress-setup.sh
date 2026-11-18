#!/bin/bash
# Fresh machine: CI's Linux packages plus the window tools and stress-ng, Rust 1.96, Node 24, Bun;
# then unpack the branch, build the debug app with the dev bridge, install Playwright's Chromium.
set -euo pipefail
exec > /home/boxd/stress-setup.log 2>&1
rm -f /home/boxd/stress-setup.CHECK_DONE
trap 'echo FAILED; touch /home/boxd/stress-setup.CHECK_DONE' ERR
export DEBIAN_FRONTEND=noninteractive
sudo -E apt-get update
sudo -E apt-get install -y build-essential curl file pkg-config libdbus-1-dev libglib2.0-dev libgtk-3-dev \
  libayatana-appindicator3-dev libssl-dev libwebkit2gtk-4.1-dev libxdo-dev librsvg2-dev patchelf \
  dbus-x11 gnome-keyring wget xvfb mutter wmctrl x11-utils xdotool stress-ng xz-utils
curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.96.0 --profile minimal
V=$(curl -fsSL https://nodejs.org/dist/index.json | python3 -c "import json,sys;print([r['version'] for r in json.load(sys.stdin) if r['version'].startswith('v24.')][0])")
curl -fsSL https://nodejs.org/dist/$V/node-$V-linux-x64.tar.xz -o /tmp/node.tar.xz
sudo tar -xJf /tmp/node.tar.xz -C /opt && sudo ln -sf /opt/node-$V-linux-x64/bin/* /usr/local/bin/
curl -fsSL https://bun.sh/install | bash
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
mkdir -p /home/boxd/pk && tar -xzf /home/boxd/branch.tgz -C /home/boxd/pk
cd /home/boxd/pk
bun install --frozen-lockfile
export CARGO_TARGET_DIR=/home/boxd/pk/var/playwright/desktop-bridge/cargo-target
bun run prepare:worker-sidecar --profile debug
cargo build --manifest-path src-tauri/Cargo.toml --features devtools-bridge
bunx playwright install chromium
sudo -E env PATH=$PATH bunx playwright install-deps chromium
echo OK; touch /home/boxd/stress-setup.CHECK_DONE
