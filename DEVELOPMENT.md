# Development

This document is the maintainer-facing guide for running PathKeep locally without confusing the browser preview shell, the Tauri desktop shell, and the release workflow.

## Setup

Install:

- Bun
- Rust `1.94.1` with `clippy`, `rustfmt`, and `llvm-tools-preview`
- Git
- Tauri 2 host dependencies for your platform

Install dependencies:

```bash
bun install
```

Linux development packages used by CI:

```bash
sudo apt-get update
sudo apt-get install -y \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  rpm
```

## Daily Commands

```bash
bun run dev
bun run desktop:dev
bun run desktop:dev:bridge
bun run check
bun run build
```

Use [TESTING.md](./TESTING.md) when you need deeper validation or release signoff.

## Surface Boundaries

- `bun run dev` is the browser preview shell. It is useful for route / styling iteration but cannot validate real filesystem, keyring, scheduler, or installer behavior.
- `bun run desktop:dev` is the real Tauri desktop surface. Use it for trust-critical flows, local data-path validation, and platform guidance review.
- `bun run desktop:dev:bridge` starts the dev-only `devtools-bridge` feature and mirrors the typed desktop command surface to localhost so Chrome / Playwright / CDP tooling can drive the frontend against real Rust responses.
- `bun run desktop:build:debug` is the fast local debug build used for release rehearsal and packaging smoke.

## Chrome And Agent Loop

For AI coding agent validation on macOS:

```bash
bun run desktop:dev:bridge
bun run test:e2e:desktop-bridge
```

What this does:

- Runs the frontend on a local port that Chrome can open directly.
- Starts a feature-gated localhost bridge from the Tauri desktop process to the existing typed command surface.
- Lets Playwright or Chrome DevTools exercise real Rust / worker / filesystem-aware read models without pretending that browser preview fixtures are desktop truth.

Honest boundary:

- The desktop bridge is dev-only and localhost-only.
- It mirrors desktop commands, not every Tauri guest API. Stronghold guest bindings, updater progress events, and other WebView-only plugin surfaces still require the actual Tauri window for final signoff.
- The runtime is intentionally tri-state: `browser-preview`, `browser-desktop-bridge`, or `tauri`.

## Repo Map

- `src/main.tsx`: desktop entrypoint covered by the desktop-contract sub-gate.
- `src/app/`: shell provider, router, route chrome, and preview orchestration.
- `src/pages/`: route-scoped UI surfaces.
- `src/lib/backend.ts`: browser-preview and mock backend reference surface.
- `src/lib/ipc/bridge.ts`: typed IPC wrapper for the desktop shell.
- `src-tauri/src/`: Tauri facade, session state, and desktop bridge.
- `src-tauri/crates/vault-core/`: archive engine, remote backup, doctor, AI, insights.
- `src-tauri/crates/vault-platform/`: scheduler and keyring/platform adapters.
- `src-tauri/crates/vault-worker/`: orchestration shared by GUI, CLI, and MCP mode.

## Local Data And Diagnostics

The Settings page exposes:

- app data root
- archive database path
- audit repository path
- app version
- git short SHA

That same metadata is what support and release docs rely on. If you change those paths or labels, update [SUPPORT.md](./SUPPORT.md), [TROUBLESHOOTING.md](./TROUBLESHOOTING.md), and the source docs under `docs/`.

## When To Update Docs

Update docs in the same branch when you change:

- platform support stance
- installer or packaging behavior
- scheduler / keyring / permission troubleshooting
- release commands or workflow inputs
- support diagnostics expectations
- user-visible settings, onboarding, or audit navigation
