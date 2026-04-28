# Testing

PathKeep has multiple quality surfaces. This file exists so we do not confuse a focused triage helper with the signed-off checker.

## Mainline Blocking Path

Run this before merging normal changes:

```bash
bun run check
```

`bun run check` is the authoritative per-commit checker. It runs:

- `bun run check:base`: formatting, linting, i18n checks, type checking, unit tests, desktop-contract checks, Rust checks, supply-chain audit, and host-matched platform checks.
- `bun run coverage:js`: 100% statement / branch / function / line coverage for active frontend runtime source under `src/**/*.{ts,tsx}`.
- `bun run coverage:rust`: 100% line + function coverage for full `src-tauri/**/src/*.rs` workspace source.
- `bun run build`: TypeScript compile + Vite browser bundle.
- `bun run test:e2e`: browser-preview Playwright smoke.
- `bun run test:e2e:desktop-bridge:truth`: Chrome + Playwright smoke against the feature-gated Rust desktop command bridge.
- `bun run check:mutation`: 100% desktop-contract JS mutation score for `src/main.tsx` and `src/lib/ipc/bridge.ts`.

## Deep Checks And Release Checks

Use these for release rehearsal and focused triage:

```bash
bun run verify
bun run check:base
bun run coverage:js
bun run coverage:rust
bun run mutation:js
bun run mutation:js:full
bun run mutation:rust
bun run check:deep
bun run test:desktop-bridge:rust
bun run test:e2e:desktop-bridge
```

`bun run verify` runs the release-style local sweep:

- `bun run check`
- `bun run desktop:build:debug`

What they mean:

- `bun run check:base`: fast static/unit/native triage path; it is not a signed-off merge gate by itself.
- `bun run mutation:js`: desktop-contract Stryker gate used by `bun run check`.
- `bun run mutation:js:full`: active frontend runtime Stryker sweep for manual / scheduled deep checks.
- `bun run mutation:rust`: whole-workspace cargo-mutants deep sweep. Surviving mutants are failures unless a narrow equivalent/inapplicable exclusion is documented with evidence.
- `bun run check:deep`: `bun run check` plus full JS/Rust mutation sweeps. It is intentionally long-running and not required before every commit.
- `bun run test:desktop-bridge:rust`: targeted Rust unit coverage for the feature-gated dev desktop bridge command dispatcher.
- `bun run test:e2e:desktop-bridge`: Chrome + Playwright smoke that drives the frontend through the dev-only desktop command bridge and proves browser automation can reach real Rust responses.

## Full Mutation Deep-Sweep Recipe

Use this only when `WORK-QA-GATE-B` is explicitly scheduled:

1. Start from a green `bun run check`.
2. Run `bun run mutation:js:full`; fix survivors with tests or product-code repairs, and only annotate narrow equivalent/inapplicable mutants with a reason.
3. Before Rust mutation, fix the cargo-mutants copy-sandbox fixture contract if the Safari reference database path still fails in the copied tree.
4. Run `bun run mutation:rust:full`, or shard the same command with `cargo mutants --shard n/m` and merge the survivor list.
5. Update [docs/plan/program/quality-matrix.md](./docs/plan/program/quality-matrix.md), this file, and [docs/plan/CHANGELOG.md](./docs/plan/CHANGELOG.md) with actual runtime, survivor closeout, and any narrow equivalent evidence.

## Focused Commands

```bash
bun run test:unit
bun run test:unit:desktop-contract
bun run coverage:js:desktop-contract
bun run check:js
bun run check:rust
bun run mutation:js:desktop-contract
bun run mutation:js:full
bun run mutation:rust:quality
```

## Honest Boundaries

- Focused helpers do not replace `bun run check`.
- The desktop-contract slice only protects `src/main.tsx` and `src/lib/ipc/bridge.ts`.
- Browser-preview e2e does not verify native scheduler install, keyring integration, signing, notarization, or filesystem side effects.
- Chrome desktop-bridge smoke verifies the typed desktop command facade from a real browser, but it still does not magically grant every Tauri guest API to Chrome. Treat it as an agent/dev-loop surface, not the final WebView plugin truth.
- Platform validation for macOS / Windows / Linux lives in [RELEASE.md](./RELEASE.md) and [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).
- User-facing support diagnostics and redaction rules live in [SUPPORT.md](./SUPPORT.md).

## Browser Support Truth

- Public browser support claims must follow [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md), not just the broadest code path currently implemented in the repo.
- `Validated now`: Google Chrome; ChatGPT Atlas on macOS; Perplexity Comet on macOS; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Microsoft Edge, Microsoft Edge Dev, Brave, Vivaldi, Arc, Opera, Opera GX, Firefox, LibreWolf, Floorp, Waterfox.

## Local Browser Validation Recipe

Use this recipe before promoting any browser into README or onboarding promise copy:

1. Verify one successful Google Chrome backup / recall path on the current local host.
2. Verify `/import` Browser Direct against one local ChatGPT Atlas macOS `History` profile under `com.openai.atlas/browser-data/host`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only schema coverage, aggregate counts, and time ranges; never paste private URLs or titles into docs, logs, or chat.
3. Verify `/import` Browser Direct against one local Perplexity Comet macOS `History` profile under `~/Library/Application Support/Comet/<profile>`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only schema coverage, aggregate counts, and time ranges; never paste private URLs or titles into docs, logs, or chat.
4. Verify Safari remains visible but unreadable when `History.db` cannot be accessed, and Browser Direct reports Full Disk Access guidance instead of a generic parse failure.
5. Verify Safari baseline backup succeeds after Full Disk Access is granted.
6. Verify `/import` Browser Direct against Safari `History.db`: preview, execute, re-import dedupe, import batch preview, revert, and restore. Record only aggregate counts and time ranges; never paste private URLs into docs, logs, or chat.

Focused Atlas / Comet / Safari Browser Direct gates:

- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core atlas -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core comet -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p browser-history-parser safari -- --nocapture`
- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-core browser_history -- --nocapture`
- `bun run test:unit -- src/pages/trust-flows/import-flows.test.tsx src/lib/browser-icons.test.tsx src/lib/i18n.test.ts`

Additional adapters may keep shipping as implementation coverage, but they stay out of public promise copy until the same recipe is documented for them.

## Release Closeout Command Order

Use this order for release rehearsal:

1. `bun run check`
2. `bun run verify`

If the change touches packaging, release workflow, platform guidance, or troubleshooting copy, also perform the traceability sweep in [RELEASE.md](./RELEASE.md).
