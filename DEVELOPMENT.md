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
bun run desktop:dev                    # the Tauri desktop app, on the development data folder
bun run dev:demo                       # real backend + synthetic browser data, in a browser
bun run dev:demo -- --first-run --fresh # same, starting on onboarding
bun run desktop:dev:bridge             # real backend on the development data folder, in a browser
bun run check
bun run build
```

`bun run check` ends with the E2E suite and leaves its report in `artifacts/e2e/`. See [TESTING.md](./TESTING.md) for details and release signoff.

## Surface Boundaries

There is no browser-preview backend. The frontend reaches Rust either through Tauri or through the dev IPC bridge; with neither, every command fails with "PathKeep cannot reach its backend". `bun run dev` on its own only serves the Vite bundle.

- `bun run desktop:dev` is the real Tauri desktop surface. Use it for anything native: the overlay title bar, the menu bar / tray icon and its `pathkeep://` events, open at login, keychain prompts, Finder reveal, the updater, window sizing.
- `bun run dev:demo` (`scripts/dev-demo.mjs`) writes synthetic Chrome ×2 and Firefox profiles under `var/demo/`, starts the debug backend with the dev bridge, initializes the archive and runs a first backup, then serves the frontend at http://127.0.0.1:1420. It keeps the archive, keychain and scheduler inside `var/demo/` (`CHB_PROJECT_ROOT`, `CHB_TEST_KEYRING_DIR`, `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`, its own schedule label), so it never touches your real history, keychain or LaunchAgents. `--fresh` wipes `var/demo/`; `--first-run` stops before initialization so the app opens on onboarding. This is the default way to look at UI changes.
- `bun run desktop:dev:bridge` starts the same bridge against the development data folder (see below). It no longer opens the installed app's archive.
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

## Development Data Folder

A build you make yourself never opens the installed app's data. Only the release packaging (`bun run release:build`) produces a build that uses the released data folder, keychain entries, schedule and installer identity. `bun run desktop:dev`, `desktop:dev:bridge`, `desktop:build`, `desktop:build:debug`, `cargo build --release` and the test binaries are development builds. The full rule and the list of what is separated are in [docs/architecture/build-identity.md](./docs/architecture/build-identity.md).

Without `CHB_PROJECT_ROOT`, a development build keeps its archive here:

| OS      | Development data folder                                         |
| ------- | --------------------------------------------------------------- |
| macOS   | `~/Library/Application Support/com.yi-ting.pathkeep-dev`        |
| Windows | `%LOCALAPPDATA%\yi-ting\pathkeep-dev\data`                      |
| Linux   | `$XDG_DATA_HOME/pathkeep-dev`, or `~/.local/share/pathkeep-dev` |

It starts empty. Nothing is copied from the installed app.

To use another folder, set `CHB_PROJECT_ROOT` to a non-empty path (an empty value is an error). A development build whose folder is moved this way refuses to touch the OS scheduler and login items unless `PATHKEEP_PLATFORM_TEST_SANDBOX_DIR` is set too:

```bash
CHB_PROJECT_ROOT="$PWD/var/manual-archive" \
PATHKEEP_PLATFORM_TEST_SANDBOX_DIR="$PWD/var/manual-os" \
PATHKEEP_PLATFORM_TEST_KEYRING_DIR="$PWD/var/manual-keyring" \
bun run desktop:dev
```

Pointing a development build at the installed app's folder is possible only by setting `CHB_PROJECT_ROOT` to it yourself. Do that on a copy, not on the archive you use: development code that writes a newer archive format leaves the installed, older app unable to open it. Even then the keychain service, schedule label and login item stay the development ones.

Other things that differ in a development build: it has its own keychain service (`com.yi-ting.pathkeep.dev`), schedule label and login item; packages are named `PathKeep Dev` with their own bundle identifier and Windows upgrade code; the updater is off. The name shown inside the app is still `PathKeep`.

Both executables answer `--build-identity` with what they were compiled as, without opening anything:

```bash
src-tauri/target/debug/pathkeep-desktop --build-identity
```

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

## Dependency updates

Use the Bun version in `package.json`'s `packageManager` field. Current JS
checks require a supported Node release: 22.22.2+, 24.15.0+ or 26+.
`bun run outdated:js` lists available updates; `bun run update:js` updates to
current stable versions, including new majors. Run `bun run check` and
`bun run check:supply-chain` after the compatibility changes.

See [dependency maintenance](docs/architecture/dependency-maintenance.md) for
the TypeScript compiler/API split, the minimatch override and Dependabot's
coverage. Do not replace the TypeScript API alias with the native compiler
package: typed ESLint still needs that compatibility API.
