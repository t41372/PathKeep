#!/usr/bin/env bash
# Responsibilities: prepare the pinned Bun, desktop services and worker in a
# Modal Linux sandbox, stopping at the first failed installation or build.
# Not responsible for: creating or terminating the sandbox, or running gates.
# A separate Bash process preserves errexit even when offload checks the setup
# command with `|| exit`, where Bash otherwise ignores errexit inside a subshell.
set -euo pipefail

pathkeep_bun_version="$(node -p 'require("./package.json").packageManager.slice(4)')"
if [ "$(bun --version)" != "$pathkeep_bun_version" ]; then
  curl -fsSL https://bun.sh/install | BUN_INSTALL=/state/home/bun bash -s -- "bun-v$pathkeep_bun_version"
fi

(pgrep -x Xvfb >/dev/null || (setsid Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &))
(test -S /tmp/offload-dbus || dbus-daemon --session --fork --address=unix:path=/tmp/offload-dbus >/dev/null)
bun install --frozen-lockfile >/dev/null
bunx playwright install chromium >/dev/null
bun run prepare:worker-sidecar --profile debug
