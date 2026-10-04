# Testing

E2E tests are how we prove features. They run against the real app: a Rust backend, the real frontend, and Playwright driving it. The authoritative gate is defined in [docs/plan/program/quality-matrix.md](./docs/plan/program/quality-matrix.md); this file is the short version.

## The Gate

Run this before merging:

```bash
bun run check
```

It runs, in order:

1. `bun run format:check`, `lint`, `check:i18n`, `typecheck`
2. `bun run build`
3. `bun run check:rust`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`
4. `bun run release:check`: updater URLs, Windows bundle config, WebView2 mode, support links
5. `bun run test:e2e`: the E2E suite

There is no coverage threshold and no mutation testing. A green run means the features the E2E suite exercises work, so that suite has to exercise real scenarios.

## E2E Suite

`playwright.config.ts` starts `scripts/pathkeep-dev-desktop-bridge.mjs`, which runs the debug Rust backend with the `devtools-bridge` feature next to the Vite dev server. The tests open the frontend in Chrome and talk to real Rust command handlers.

Each run writes seeded synthetic browser profiles (two Chrome, one Firefox, real SQLite history files, about 21,000 visits over 14 months) to a temp folder, with the archive, keyring and project root next to them. `tests/e2e/support/fixture.ts` reads the same data, so expected numbers are computed from it rather than typed into the specs. The keyring is a file in that folder: debug builds honor `CHB_TEST_KEYRING_DIR`, release builds ignore it.

The specs run in order on one archive (see `projects` in the config): `first-run` (onboarding, first backup) → `read` (search, insights) → `change` (new visits, paused source, custom-interval schedule install / verify / remove, Browser Direct import / undo / restore, app lock) → `wipe` (delete all data).

Writing E2E tests:

- Pick a medium-to-hard scenario, not the simplest one that passes. A half-built feature usually passes the happy path.
- Go through the same entry point a user would. Do not mock the backend; if a seam is unavoidable, put it at the outermost transport and keep the real compute and I/O under test.
- Assert the negative before the positive (not found before the change, found after).
- Write the test from the intended behavior before or alongside the code, not from what the code happens to do afterwards.

### Artifacts

Every run, pass or fail, ends with an artifact in `artifacts/e2e/`:

- `fixture.json`: the data the run used (seed, `now`, every visit per profile)
- `report/index.html`: HTML report; each check attaches the `{ expected, actual }` pair it compared
- `results.json`: results for every test
- `test-results/`: traces, screenshots, videos

To repeat a run with the same data, pass its `now`: `PATHKEEP_E2E_NOW=<now> bun run test:e2e`. The folder is not committed; CI uploads it as `e2e-artifacts`. See [artifacts/e2e/README.md](./artifacts/e2e/README.md) for how to open reports and traces.

## Other Commands

```bash
bun run check:base           # format, lint, i18n, typecheck, Rust checks (no build, no E2E)
bun run check:slow           # supply-chain audit + host-matched platform tests
bun run verify               # check + debug desktop build, for release rehearsal
bun run test:e2e:headed      # watch the suite drive the app
bun run dev:demo             # the real app on the same synthetic data, for trying things by hand
bun run dev:demo -- --first-run --fresh   # ...starting at onboarding
bun run test:unit            # Vitest; passes when there are no tests
bun run test:desktop-bridge:rust
```

Unit tests are the exception. Use one only when something has to be tested in isolation, and write down every way it could fail before writing the code.

## Honest Boundaries

- A focused command does not replace `bun run check`.
- The E2E suite does not install a real schedule, write the real keychain, sign, notarize, or check the native window. Onboarding picks "Manual"; `schedule.spec` and `wipe.spec` install schedules into the debug sandbox (`PATHKEEP_PLATFORM_TEST_SANDBOX_DIR`), never into launchd or Task Scheduler. A debug run that sets `CHB_PROJECT_ROOT` without the sandbox is refused by the scheduler rather than touching your own LaunchAgent. Windows Task Scheduler apply/status/remove must still be accepted on a real Windows host or VM even though the Rust unit slice uses a stubbed `schtasks` runner.
- `bun run release:check` proves the release config still permits unsigned Windows installers and keeps WebView2 in download-bootstrapper mode; it does not prove a specific Windows host can launch the installer.
- The GitHub `Windows Test Binary` workflow builds an unsigned Windows app and uploads a short-lived workflow artifact for QA handoff without updating public release assets. It still needs real Windows test-machine validation for install, first launch, scheduler apply/status/remove, and upgrade behavior.
- GitHub-hosted Windows runners currently validate the desktop surface with `desktop:build:debug`, `vault-platform` native-host tests, and the updater E2E path. The `pathkeep-desktop` Rust test binary for updater/file-manager facades is skipped on Windows CI because the hosted runner fails before the test harness starts with a loader-level `STATUS_ENTRYPOINT_NOT_FOUND`; macOS/Linux still run those Rust facade tests.
- The desktop-bridge E2E suite verifies the typed desktop command facade from a real browser, but it still does not magically grant every Tauri guest API to Chrome. Treat it as an agent/dev-loop surface, not the final WebView plugin truth.
- Platform validation for macOS / Windows / Linux lives in [RELEASE.md](./RELEASE.md) and [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).
- User-facing support diagnostics and redaction rules live in [SUPPORT.md](./SUPPORT.md).

## Browser Support Truth

- Public browser support claims must follow [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md), not just the broadest code path currently implemented in the repo.
- `Validated now`: Google Chrome; Microsoft Edge / Edge Dev; Firefox history-only baseline; ChatGPT Atlas on macOS; Perplexity Comet on macOS; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Brave, Vivaldi, Arc, Opera, Opera GX, LibreWolf, Floorp, Waterfox.

## Local Browser Validation Recipe

Use this recipe before promoting any browser into README or onboarding promise copy:

1. Verify one successful Google Chrome backup / recall path on the current local host.
2. Verify one successful Microsoft Edge backup / recall path, then verify `/import` Browser Direct against one Edge `History` profile: preview, execute, re-import dedupe, import batch preview, revert, and restore. Confirm source profile metadata preserves `Microsoft Edge` rather than generic Chrome.
3. Verify one successful Firefox backup / recall path, then verify `/import` Browser Direct against one Firefox profile directory or `places.sqlite`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record that Firefox is history-only in this release slice.
4. Verify `/import` Browser Direct against one local ChatGPT Atlas macOS `History` profile under `com.openai.atlas/browser-data/host`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only schema coverage, aggregate counts, and time ranges; never paste private URLs or titles into docs, logs, or chat.
5. Verify `/import` Browser Direct against one local Perplexity Comet macOS `History` profile under `~/Library/Application Support/Comet/<profile>`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only schema coverage, aggregate counts, and time ranges; never paste private URLs or titles into docs, logs, or chat.
6. Verify Safari remains visible but unreadable when `History.db` cannot be accessed, and Browser Direct reports Full Disk Access guidance instead of a generic parse failure.
7. Verify Safari baseline backup succeeds after Full Disk Access is granted.
8. Verify `/import` Browser Direct against Safari `History.db`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only aggregate counts and time ranges; never paste private URLs into docs, logs, or chat.

Focused browser / scheduler release-blocker gates:

- `cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser firefox -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core takeout::tests::browser_history -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core backup_ -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform scheduler -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core atlas -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core comet -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser safari -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core browser_history -- --nocapture`

Additional adapters may keep shipping as implementation coverage, but they stay out of public promise copy until the same recipe is documented for them.

## Release Closeout Command Order

Use this order for release rehearsal:

1. `bun run check`
2. `bun run check:slow`
3. `bun run verify`

If the change touches packaging, release workflow, platform guidance, or troubleshooting copy, also perform the traceability sweep in [RELEASE.md](./RELEASE.md).
