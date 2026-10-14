# Development

This document is the maintainer-facing guide for running PathKeep locally: the Tauri desktop app, the same app driven from a browser through the dev bridge, and the release workflow.

## Setup

Install:

- Bun
- Rust `1.94.1` with `clippy` and `rustfmt`
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
bun run desktop:dev                    # the Tauri desktop app
bun run dev:demo                       # real backend + synthetic browser data, in a browser
bun run dev:demo -- --first-run --fresh # same, starting on onboarding
bun run desktop:dev:bridge             # real backend on your own data, in a browser
bun run check
bun run build
```

`bun run check` ends with the E2E suite and leaves its report in `artifacts/e2e/`. See [TESTING.md](./TESTING.md) for details and release signoff.

## Surface Boundaries

There is no browser-preview backend. The frontend reaches Rust either through Tauri or through the dev IPC bridge; with neither, every command fails with "PathKeep cannot reach its backend". `bun run dev` on its own only serves the Vite bundle.

- `bun run desktop:dev` is the real Tauri desktop surface. Use it for anything native: the overlay title bar, the menu bar / tray icon and its `pathkeep://` events, open at login, keychain prompts, Finder reveal, the updater, window sizing.
- `bun run dev:demo` (`scripts/dev-demo.mjs`) writes synthetic Chrome ×2 and Firefox profiles under `var/demo/`, starts the debug backend with the dev bridge, initializes the archive and runs a first backup, then serves the frontend at http://127.0.0.1:1420. It keeps the archive, keychain and scheduler inside `var/demo/` (`CHB_PROJECT_ROOT`, `CHB_TEST_KEYRING_DIR`, `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`, its own schedule label), so it never touches your real history, keychain or LaunchAgents. `--fresh` wipes `var/demo/`; `--first-run` stops before initialization so the app opens on onboarding. This is the default way to look at UI changes.
- `bun run desktop:dev:bridge` starts the same bridge against your normal app data folder. Use it only when you need your own archive in a browser.
- `bun run desktop:build:debug` is the fast local debug build used for release rehearsal and packaging smoke.

## Chrome And Agent Loop

For AI coding agent validation:

```bash
bun run dev:demo
# open http://127.0.0.1:1420 in Chrome, or:
bun run test:e2e
```

What this does:

- Runs the frontend on a local port that Chrome can open directly.
- Starts a feature-gated localhost bridge from the Tauri desktop process to the existing typed command surface.
- Lets Playwright or Chrome DevTools exercise real Rust / worker / filesystem-aware read models.

Honest boundary:

- The desktop bridge is dev-only and localhost-only.
- It mirrors desktop commands, not every Tauri guest API. There is no menu bar icon, no `pathkeep://` desktop events, no native file dialogs and no updater in the browser; those need the actual Tauri window for final signoff.
- `src/lib/runtime.ts` reports `tauri`, `browser-desktop-bridge`, or `browser-preview`; the last one now only means "no backend".

## Repo Map

- `src/main.tsx`: frontend entrypoint (theme and language before first paint).
- `src/app/`: session state machine, backup runner, router, and the window shell (nav rail, ⌘K palette, lock and status screens).
- `src/features/`: one folder per screen — home, history, insights, ask, backup, settings, onboarding, plus `ai-setup` shared by Settings and onboarding.
- `src/components/ui/`: shadcn/ui components, kept as installed. `src/components/evilcharts/`: EvilCharts charts, kept as installed. `src/components/app/`: our shared components (cards, setting rows, heatmap, favicon, backup button).
- `src/lib/backend-client/`: typed clients, one per domain. `src/lib/queries/` and `src/lib/query.ts`: TanStack Query setup and shared queries. `src/lib/i18n/`: translator and per-feature catalogs in `messages/`.
- `src/lib/ipc/bridge.ts`: typed IPC wrapper (Tauri `invoke` or the dev bridge).
- `src-tauri/src/`: Tauri facade, session state, menu bar icon, and the dev bridge.
- `src-tauri/crates/vault-core/`: archive engine, migration bundles, doctor, AI, insights.
- `src-tauri/crates/vault-platform/`: scheduler, keyring, login items and platform adapters.
- `src-tauri/crates/vault-worker/`: orchestration shared by GUI, CLI, and MCP mode.

Boundaries and import direction: [docs/architecture/module-boundary-map.md](./docs/architecture/module-boundary-map.md).

## Local Data And Diagnostics

The app shows:

- Settings → Storage: the app data folder (Show in Finder / folder), disk usage
- Settings → About: product name, version and short commit (marked when built from a dirty tree), a button that opens the logs folder, and the archive health check

That same metadata is what support and release docs rely on. If you change those paths or labels, update [SUPPORT.md](./SUPPORT.md), [TROUBLESHOOTING.md](./TROUBLESHOOTING.md), and the source docs under `docs/`.

## When To Update Docs

Update docs in the same branch when you change:

- platform support stance
- installer or packaging behavior
- scheduler / keyring / permission troubleshooting
- release commands or workflow inputs
- support diagnostics expectations
- user-visible settings, onboarding, or screens (`docs/design/screens-and-nav.md`)
