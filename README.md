[![CI](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/ci.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/ci.yml)
[![Release](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/release.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/release.yml)
[![Mutation](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/mutation.yml/badge.svg)](https://github.com/t41372/BrowserHistoryBackup/actions/workflows/mutation.yml)
[![Latest release](https://img.shields.io/github/v/release/t41372/BrowserHistoryBackup?display_name=tag)](https://github.com/t41372/BrowserHistoryBackup/releases)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-1f6feb.svg)](./LICENSE)

# PathKeep

Browser History Backup is a local-first desktop app for long-term, auditable browser history archiving. It is built with Tauri 2, Rust, Bun, React, and Vite, and is designed around one rule: every meaningful system action should stay inspectable.

The app keeps raw provenance, normalized query data, audit manifests, scheduler artifacts, import batches, and export outputs separate so the user can preview what is about to happen, inspect what already happened, and roll back dirty imports without erasing the audit trail.

## Current Status

- The desktop app, worker mode, and local-first archive pipeline are implemented.
- Browser discovery, incremental backup, Takeout import, export, S3 backup, and optional AI analysis all exist in the codebase today.
- The GitHub release pipeline now builds platform installers for macOS, Windows, and Linux and publishes them as GitHub release assets.
- The project is still in active development. Some areas are intentionally honest about their current state:
  - Scheduler `Apply` is implemented on macOS. Windows and Linux currently ship `Preview` and `Manual` flows only.
  - Installer signing and notarization are supported by the release workflow only if the relevant GitHub secrets are configured.
  - Rust workspace coverage is not yet at the originally requested 100%; the latest `cargo llvm-cov` run reports 82.68% line coverage.

## Feature Inventory

### Archive and Provenance

- Incremental history backup using staged copies of browser history databases instead of live reads.
- Raw row retention plus normalized archive tables for visits, URLs, downloads, search terms, and favicons.
- Append-only backup manifests with chained hashes.
- Local git-backed audit artifacts for manifests, imports, scheduler changes, and related reports.
- Plaintext and encrypted archive modes.
- Session unlock flow plus system keyring support for unattended operations.

### Browser and OS Support

- Chromium-family support:
  Google Chrome, Chromium, Microsoft Edge, Microsoft Edge Dev, Brave, Vivaldi, Arc, Opera, and Opera GX.
- Firefox-family support:
  Firefox, LibreWolf, Floorp, and Waterfox.
- Safari support on macOS.
- Scheduler preview artifacts for macOS `launchd`, Windows Task Scheduler XML, and Linux `systemd --user`.
- Platform keyring adapters for macOS, Windows, Linux, and a file-backed test keyring.

### Import and Export

- Google Takeout dry-run inspection before import.
- Recognized-file reporting, quarantine reporting, preview rows, duplicate counting, and notes.
- Batch-level import review and revert controls.
- Structured exports to HTML, Markdown, plain text, and JSONL.
- S3-compatible remote backup bundle preview and upload.

### Desktop UX

- Tauri desktop shell with React audit-first UI.
- English, Simplified Chinese, and Traditional Chinese UI with system-language detection and user override.
- Setup flow for source selection, archive creation, scheduler preview, and review.
- Settings UI for language, security, key management, remote backup, AI provider configuration, app build metadata, and local data paths.
- In-app display of the running app version and short git commit SHA.
- Buttons in the UI to open the app data root, archive database location, and audit repository in the system file manager.

### Optional AI and Integration Features

- Optional LLM and embedding provider configuration, disabled by default.
- Multiple providers per request format, each with configurable base URL, models, and keyring-backed API key storage.
- Semantic indexing, semantic search, and grounded assistant Q&A using `rig-core`.
- MCP server preview artifacts and worker-mode MCP server support.
- Skill integration preview artifacts.

## Architecture

- [`src`](./src)
  React + TypeScript desktop UI, browser-preview mocks, and frontend tests.
- [`src-tauri`](./src-tauri)
  Tauri shell plus the Rust workspace.
- [`src-tauri/crates/vault-core`](./src-tauri/crates/vault-core)
  Archive schema, migrations, browser snapshotting, exports, Takeout ingestion, AI indexing, audit manifests, and health checks.
- [`src-tauri/crates/vault-worker`](./src-tauri/crates/vault-worker)
  Shared orchestration used by the GUI, worker CLI, MCP server entrypoint, and tests.
- [`src-tauri/crates/vault-platform`](./src-tauri/crates/vault-platform)
  Scheduler artifact generation plus platform keyring integration.
- [`docs/reference-review.md`](./docs/reference-review.md)
  Notes from comparing support coverage against `1History` and `browserexport`.

## Build and Run From Source

### Prerequisites

- Bun
- Rust 1.94.1 with `clippy`, `rustfmt`, and `llvm-tools-preview`
- Git
- Platform prerequisites for Tauri 2:
  [Tauri distribute docs](https://v2.tauri.app/distribute/)

For Linux development on Debian or Ubuntu, the current repo and CI use:

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

### Install Dependencies

```bash
bun install
```

### Run the Web Preview Shell

This is useful for quick frontend iteration with mock data.

```bash
bun run dev
```

### Run the Full Desktop App From Source

```bash
bun run desktop:dev
```

### Run Checks

```bash
bun run check
```

### Run the Full Verification Sweep

```bash
bun run verify
```

### Build the Desktop App Locally

Debug desktop binary:

```bash
bun run desktop:build:debug
```

Release bundle for the current host OS:

```bash
bun run desktop:build
```

Important local output locations:

- Debug desktop binary:
  `src-tauri/target/debug/browser-history-backup-desktop`
- Release bundle directory:
  `src-tauri/target/release/bundle/`

The exact installer files depend on the host OS:

- macOS:
  `.app` and `.dmg`
- Windows:
  installer assets such as `.msi` and/or NSIS `.exe`
- Linux:
  `.AppImage`, `.deb`, and `.rpm` when the required packaging tools are present

## Quality Gates

### JavaScript and TypeScript

- Prettier formatting
- ESLint
- `tsc -b`
- Vitest unit tests
- React integration tests
- Playwright e2e smoke coverage
- Stryker mutation testing

### Rust

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo llvm-cov`
- `cargo mutants`
- `cargo audit`
- `cargo deny`

## GitHub Actions and Release Pipeline

The repository currently ships three GitHub Actions workflows:

- [`ci.yml`](./.github/workflows/ci.yml)
  Frontend checks, Playwright smoke coverage, Rust formatting/lint/test gates, Rust supply-chain checks (`cargo audit` and `cargo deny`), coverage artifact generation, and a macOS debug desktop build.
- [`mutation.yml`](./.github/workflows/mutation.yml)
  Manual or scheduled JavaScript and Rust mutation-test sweeps.
- [`release.yml`](./.github/workflows/release.yml)
  Cross-platform desktop release workflow.

### Release Workflow Behavior

- Trigger automatically by pushing a tag like `v0.1.0`.
- Or run manually from the GitHub Actions UI with the `Release` workflow.
- Manual runs resolve the release tag from the current app version in `package.json` and `src-tauri/Cargo.toml`, so bump the version first if you intend to publish a new release instead of updating an existing one.
- The workflow builds:
  - macOS Apple Silicon bundles
  - macOS Intel bundles
  - Windows bundles
  - Linux bundles
- Release assets are uploaded directly to the GitHub Release using the official Tauri action.
- After all matrix builds finish, the workflow downloads the release assets again, generates `SHA256SUMS.txt`, and uploads that checksum manifest back to the same release.

If you want signed or notarized installers, configure the signing secrets referenced in [`release.yml`](./.github/workflows/release.yml). Without those secrets, the workflow still builds unsigned installers.

## Security Model

- The archive can run in `Encrypted` or `Plaintext` mode.
- Encrypted mode is built around a user-managed master password and local secret storage.
- If the user forgets the archive password and has no other valid unlock path, the encrypted archive should be treated as unrecoverable.
- Archive contents and raw snapshots do not live inside the local audit git repository.
- Remote backups package the archive plus audit material and upload to S3-compatible storage using credentials stored in the native keyring.

## Development Notes

- The repo uses [`prek`](https://prek.j178.dev/) with `.pre-commit-config.yaml`.
- Install hooks with:

```bash
cargo install prek --locked
prek install
prek install --hook-type pre-push
```

- Run all configured hooks locally with:

```bash
prek run --all-files
prek run --hook-stage pre-push --all-files
```

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for local setup, release workflow usage, quality gates, and contribution expectations.

## License

PathKeep is licensed under the [GNU General Public License v3.0](./LICENSE).
