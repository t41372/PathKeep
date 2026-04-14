[![CI](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/ci.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/ci.yml)
[![Release](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/release.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/release.yml)
[![Mutation](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/mutation.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/mutation.yml)
[![Latest release](https://img.shields.io/github/v/release/t41372/BrowserHistoryBackup?display_name=tag)](https://github.com/t41372/BrowserHistoryBackup/releases)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-1f6feb.svg)](./LICENSE)

# PathKeep

PathKeep is a local-first desktop app for long-term, auditable browser history archiving. It is built with Tauri 2, Rust, React 19, TypeScript, Vite, and Bun, and it treats every high-risk action as a reviewable flow instead of a background black box.

The app keeps canonical history facts, rollback state, audit artifacts, scheduler previews, remote backup bundles, and optional AI-derived state separate. That separation is what makes Preview / Manual / Execute possible across backup, import, scheduling, security, and remote backup.

## Product Boundaries

- Local-first by default. Archive data stays on the machine unless the user explicitly configures remote backup.
- Intelligence is optional. Search, backup, import, rollback, export, audit, and recovery still work without any AI provider.
- Recoverability beats convenience. Remote backup, re-key, rollback, and derived-state rebuild all keep the user-facing boundary honest.
- Hidden telemetry is a non-goal. Any frontend analytics must stay explicit opt-in, coarse, metadata-only, and first-party.

## Release Status

- `WORK-M4-B` release-readiness closeout is complete: release docs, troubleshooting docs, platform validation runbooks, support diagnostics guidance, and release workflow preflight are now in repo.
- macOS is the primary release target and has a documented signing / notarization path in CI.
- Windows and Linux release artifacts are built in CI and covered by the validation runbook, but both remain explicit preview channels until maintainers wire platform signing choices for their own credentials and distribution policy.
- App Lock remains a session-only boundary. macOS now ships truthful Touch ID unlock for the current session, while Windows / Linux still show honest unsupported states instead of fake parity.
- Settings also includes a manual update-check surface with release availability, release notes, install progress, and restart controls backed by Tauri's updater contract.

## What Ships Today

### Archive, Audit, And Trust

- Incremental browser-history backup through staged database copies rather than live reads.
- Append-only archive ledger with rollback-aware visibility and audit artifacts kept outside the archive DB.
- Google Takeout preview, import, revert, restore, and doctor / repair flows.
- Plaintext and encrypted archive modes, plus re-key preview / execute review surfaces.
- Native scheduler preview for macOS `launchd`, Windows Task Scheduler XML, and Linux `systemd --user`.

### Recall And Intelligence

- Keyword, regex, semantic, and hybrid recall surfaces with honest fallback when AI is disabled or unavailable.
- Optional provider configuration for LLM and embedding backends.
- Insight cards, topic / thread views, storage analytics, and evidence deep-links back into Explorer.
- Remote backup PME with bundle preview, upload, and checksum / restore-readiness verification.

### Platform And Support Surfaces

- Settings page exposes the app data root, archive database path, audit repository path, app version, and git short SHA.
- Platform troubleshooting callouts exist for Safari Full Disk Access, scheduler mismatch / manual review, and keyring degradation.
- User-facing support docs, troubleshooting guidance, and issue templates are bundled in-repo instead of living only in chat or tribal knowledge.

## Browser Support Today

PathKeep separates implemented adapters from publicly validated support so the README only promises the browser paths that are currently backed by local validation evidence.

- `Validated now`: Google Chrome; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Microsoft Edge, Microsoft Edge Dev, Brave, Vivaldi, Arc, Opera, Opera GX, Firefox, LibreWolf, Floorp, Waterfox.
- `Adapter candidates`: browsers that are not yet wired into discovery, parser selection, and canonical archive ingest.

## Platform Support

| Platform | Channel | Notes                                                                                                                           |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | Primary | Signed / notarized path is documented for release CI; Safari baseline support still depends on Full Disk Access.                |
| Windows  | Preview | MSI / NSIS artifacts build in CI; SmartScreen reputation and code signing remain maintainer-operated.                           |
| Linux    | Preview | AppImage / `.deb` / `.rpm` build when packaging dependencies are present; keyring behavior still varies by desktop environment. |

## Build From Source

### Prerequisites

- Bun
- Rust `1.94.1` with `clippy`, `rustfmt`, and `llvm-tools-preview`
- Git
- Tauri 2 platform prerequisites: [Tauri distribute docs](https://v2.tauri.app/distribute/)

Linux development on Debian or Ubuntu currently uses:

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

Install dependencies:

```bash
bun install
```

Run the browser preview shell:

```bash
bun run dev
```

Run the full desktop app:

```bash
bun run desktop:dev
```

Build the current host bundle:

```bash
bun run desktop:build
```

Build the debug desktop binary:

```bash
bun run desktop:build:debug
```

## Quality Gates

Mainline checks:

- `bun run check`
- `bun run coverage:js`
- `bun run coverage:rust`
- `bun run build`
- `bun run test:e2e`

Release / deep checks:

- `bun run verify`
- `bun run mutation:js`
- `bun run mutation:rust`
- `bun run mutation:rust:full` (exploratory whole-workspace Rust sweep)

Current recovery-mode note: mutation scripts remain available, but they are temporarily out of the default `check` / `verify` path until the current product-recovery wave is complete.

`bun run mutation:rust` currently protects the honest Rust mutation contract: `browser-history-parser` plus the `vault-core/src/ai.rs` status/helper slice. The desktop-contract slice inside `bun run check` only protects `src/main.tsx` and `src/lib/ipc/bridge.ts`; it is not a blanket signoff for every route or component. See [TESTING.md](./TESTING.md) for the honest boundary.

## Docs Map

| Need                                        | Read                                                                                                                     |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Contributor workflow                        | [CONTRIBUTING.md](./CONTRIBUTING.md)                                                                                     |
| Local environment and repo layout           | [DEVELOPMENT.md](./DEVELOPMENT.md)                                                                                       |
| Test surfaces and command matrix            | [TESTING.md](./TESTING.md)                                                                                               |
| Release runbook and artifact matrix         | [RELEASE.md](./RELEASE.md)                                                                                               |
| User troubleshooting and diagnostics        | [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)                                                                               |
| Support and bug-report expectations         | [SUPPORT.md](./SUPPORT.md)                                                                                               |
| Browser support truth and adapter promotion | [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md) |
| Product, feature, and milestone source docs | [docs/](./docs/)                                                                                                         |

## Repository Layout

- [`src`](./src): React + TypeScript desktop UI, preview fixtures, and UI tests.
- [`src-tauri`](./src-tauri): Tauri shell plus the Rust workspace.
- [`src-tauri/crates/vault-core`](./src-tauri/crates/vault-core): archive engine, remote backup, indexing, insights, and audit behavior.
- [`src-tauri/crates/vault-platform`](./src-tauri/crates/vault-platform): scheduler artifacts and platform adapters.
- [`src-tauri/crates/vault-worker`](./src-tauri/crates/vault-worker): shared orchestration for GUI, worker CLI, and MCP mode.
- [`docs/plan/m4-full-polish/release-readiness-runbook.md`](./docs/plan/m4-full-polish/release-readiness-runbook.md): internal release-readiness and platform validation source of truth.

## Contributing

PathKeep uses Conventional Commits, colocated tests, and doc-first updates for product or platform changes. Start with [CONTRIBUTING.md](./CONTRIBUTING.md); release and support changes must also keep [RELEASE.md](./RELEASE.md), [TROUBLESHOOTING.md](./TROUBLESHOOTING.md), and the matching `docs/` source docs in sync.

## License

PathKeep is licensed under the [GNU General Public License v3.0](./LICENSE).
