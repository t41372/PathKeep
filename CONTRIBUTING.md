# Contributing

## Ground Rules

- Keep the app local-first and audit-friendly.
- Prefer additive, reviewable changes over hidden automation.
- Do not remove provenance or delete user data unless the feature explicitly requires a reversible user action.
- Keep UX changes aligned with the Stitch-approved direction instead of ad-hoc restyling.
- If a change affects user-facing behavior, update the docs in the same branch.

## Local Setup

### Required Tools

- Bun
- Rust 1.94.1
- Git
- Tauri 2 platform prerequisites:
  [Tauri distribute docs](https://v2.tauri.app/distribute/)

Recommended Rust toolchain setup:

```bash
rustup toolchain install 1.94.1 \
  --component clippy \
  --component rustfmt \
  --component llvm-tools-preview
rustup override set 1.94.1
```

Install project dependencies:

```bash
bun install
```

Install pre-commit hooks with `prek`:

```bash
cargo install prek --locked
prek install
prek install --hook-type pre-push
```

For Linux development on Debian or Ubuntu, use:

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

## Running From Source

Browser-only frontend preview:

```bash
bun run dev
```

Full desktop app in development mode:

```bash
bun run desktop:dev
```

## Building the App

Release bundle for the current OS:

```bash
bun run desktop:build
```

Debug desktop binary:

```bash
bun run desktop:build:debug
```

Build outputs:

- Debug binary:
  `src-tauri/target/debug/pathkeep-desktop`
- Release bundle directory:
  `src-tauri/target/release/bundle/`

## Daily Commands

```bash
bun run check
bun run build
bun run desktop:build:debug
```

For deeper validation:

```bash
bun run verify
bun run coverage
bun run mutation:js
bun run mutation:rust
```

Useful focused commands:

```bash
bun run format
bun run format:check
bun run lint
bun run typecheck
bun run test:unit
bun run test:e2e
bun run check:js
bun run check:rust
bun run coverage:rust
```

## CI and Release Workflows

### CI

- `.github/workflows/ci.yml`
  Runs frontend formatting, linting, type checking, unit tests, build, Playwright e2e smoke coverage, Rust checks, Rust supply-chain checks (`cargo audit` and `cargo deny`), coverage artifact upload, and a macOS debug build.

### Mutation

- `.github/workflows/mutation.yml`
  Runs JavaScript and Rust mutation sweeps on demand and on schedule.

### Release

- `.github/workflows/release.yml`
  Builds and publishes desktop release assets for:
  - macOS Apple Silicon
  - macOS Intel
  - Windows
  - Linux

The release workflow has two entry points:

1. Push a version tag such as `v0.1.0`.
2. Run the `Release` workflow manually from GitHub Actions.

Manual runs derive the release tag from the current app version, so bump the version first if you want a new release instead of updating the assets for an existing one.

The release job uses `tauri-apps/tauri-action` to upload installers directly to the GitHub Release, then downloads the release assets again and uploads a generated `SHA256SUMS.txt` manifest.

### Release Preparation Checklist

Before cutting a release:

1. Keep versions aligned in:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Run:

```bash
bun run verify
```

3. Choose one release path:

Tag-driven release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Manual release:

- Push the versioned commit.
- Open GitHub Actions.
- Run the `Release` workflow.
- Set `draft` and `prerelease` flags as needed.

4. Verify the GitHub Release contains:
   - macOS installers
   - Windows installers
   - Linux packages
   - `SHA256SUMS.txt`

### Signing Secrets

The release workflow is prepared for optional signing and notarization through GitHub secrets:

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

If those secrets are absent, the workflow still publishes unsigned installers.

## Commit Style

Use Conventional Commits.

Examples:

- `feat(app): add takeout batch revert flow`
- `test(rust): add worker cli integration coverage`
- `docs(repo): document release workflow`
- `chore(ci): install linux tauri dependencies`

## Pull Request Checklist

- Run `bun run check`
- Run `bun run build`
- Run `bun run desktop:build:debug`
- Update docs if user-facing behavior changed
- Keep commit history reviewable and logically grouped
- Mention any gaps you intentionally did not address

## Testing Strategy

### JavaScript and TypeScript

- Prettier
- ESLint
- `tsc`
- Vitest
- React integration tests
- Playwright
- Stryker

### Rust

- rustfmt
- clippy
- cargo test
- cargo llvm-cov
- cargo mutants
- cargo audit
- cargo deny

Current reality:

- Frontend core logic under `src/lib` is expected to keep 100% unit-test coverage.
- Rust coverage tooling is wired and should stay green, but the workspace is not yet at 100% line coverage.

## Pre-commit Hooks

Run the configured hooks locally with:

```bash
prek run --all-files
prek run --hook-stage pre-push --all-files
```

## Reporting Design Drift

If UI implementation drifts from the approved design direction:

- stop restyling in code
- capture the mismatch with screenshots
- realign the design source first
- only then continue implementation
