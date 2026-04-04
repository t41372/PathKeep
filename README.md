[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-1f6feb.svg)](./LICENSE)
![Rust 2024](https://img.shields.io/badge/rust-2024%20edition-f74c00.svg)
![React 19](https://img.shields.io/badge/frontend-React%2019%20%2B%20TypeScript-0ea5e9.svg)
![CI Configured](https://img.shields.io/badge/ci-GitHub%20Actions-2088ff.svg)
![Quality Gates](https://img.shields.io/badge/tests-unit%20%2B%20integration%20%2B%20e2e-16a34a.svg)

# Chrome History Backup

Chrome History Backup is a local-first desktop app for long-term, auditable Chrome history archiving. It is built with Tauri, Rust, Bun, React, and Vite, and is designed around one core idea: every meaningful operation should remain inspectable.

The app keeps raw provenance, normalized query data, audit manifests, and scheduler artifacts separate so the user can inspect what happened, preview system changes, and revert dirty imports without losing the audit trail.

## Current Scope

- Desktop app shell with Tauri 2 and a React audit-first UI.
- Rust workspace split into `vault-core`, `vault-worker`, and `vault-platform`.
- Incremental Chrome history backups with staged snapshotting.
- Local archive database with plaintext or encrypted modes.
- Native scheduler preview and apply flows.
- Google Takeout dry-run, import preview, batch-level revert, and audit retention.
- HTML, Markdown, text, and JSONL exports.
- Optional S3-compatible remote backup bundles.
- UI internationalization for English, 简体中文, and 繁體中文.

## Architecture

- `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src`
  React + TypeScript desktop UI, integration tests, and browser-preview mocks.
- `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri`
  Tauri shell plus the Rust workspace.
- `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-core`
  Archive schema, migrations, staged Chrome copies, exports, Takeout ingestion, audit manifests, and health checks.
- `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-worker`
  Shared orchestration layer used by the GUI, worker mode, and tests.
- `/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/src-tauri/crates/vault-platform`
  Scheduler artifact generation plus platform keyring integration.

## Security Model

- The archive can run in `Encrypted` or `Plaintext` mode.
- Encrypted mode is designed around a user-managed master password and local secret storage.
- If the user forgets the archive password and has no other valid unlock path, the encrypted archive should be treated as unrecoverable.
- Audit manifests live in a local git repository, but the archive database and raw snapshots do not.
- Remote backups package the archive plus audit material and upload to S3-compatible storage with credentials kept in the native keyring.

## Quality Gates

The repo is wired for layered checks instead of a single happy-path build:

- JavaScript/TypeScript
  - Prettier formatting
  - ESLint with type-aware rules
  - `tsc -b` type checking
  - Vitest unit tests with 100% coverage gates on core frontend logic modules under `src/lib`
  - React integration tests
  - Playwright browser e2e smoke coverage
  - Stryker mutation testing
- Rust
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-targets`
  - `cargo llvm-cov`
  - `cargo mutants`
  - `cargo audit`
  - `cargo deny`

## Quick Start

### Prerequisites

- Bun
- Rust stable toolchain with `clippy`, `rustfmt`, and `llvm-tools-preview`
- Tauri system prerequisites for your platform
- Git

### Install

```bash
bun install
```

### Local Development

```bash
bun run desktop:dev
```

### Core Checks

```bash
bun run check
```

### Full Verification

```bash
bun run verify
```

### Coverage

```bash
bun run coverage
```

### Mutation Testing

```bash
bun run mutation:js
bun run mutation:rust
```

## Pre-commit with prek

This repo uses a standard `.pre-commit-config.yaml`, but the intended runner is [`prek`](https://prek.j178.dev/), the Rust implementation of the pre-commit workflow.

```bash
cargo install prek --locked
prek install
prek install --hook-type pre-push
prek run --all-files
```

## GitHub Actions

The repo includes:

- `ci.yml`
  lint, typecheck, unit/integration tests, e2e smoke, Rust checks, coverage artifact generation, and a debug desktop build
- `mutation.yml`
  JS and Rust mutation workflows intended for manual runs or scheduled quality sweeps

If you add a GitHub remote later, you can replace the static CI badge with the live Actions badge URL for your repository.

## Project Commands

```bash
bun run format
bun run format:check
bun run lint
bun run typecheck
bun run test:unit
bun run test:e2e
bun run check:js
bun run check:rust
bun run desktop:build:debug
```

## Contributing

See [CONTRIBUTING.md](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/CONTRIBUTING.md) for branch, testing, commit, and review expectations.

## License

Chrome History Backup is licensed under the [GNU General Public License v3.0](/Users/tim/LocalData/coding/2026/Lab/8_chrome_history_backup/LICENSE).
