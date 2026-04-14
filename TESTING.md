# Testing

PathKeep has multiple quality surfaces. This file exists so we do not confuse a passing desktop-contract sub-gate with full desktop or release signoff.

## Mainline Blocking Path

Run these before merging normal changes:

```bash
bun run check
bun run coverage:js
bun run coverage:rust
bun run build
bun run test:e2e
```

What they mean:

- `bun run check`: formatting, linting, type checking, unit tests, desktop-contract checks, Rust checks, and supply-chain audit.
- `bun run coverage:js`: 100% coverage gate for the living M0-M3 JS quality surface.
- `bun run coverage:rust`: 100% coverage gate for the Tauri desktop command / bridge quality surface.
- `bun run build`: browser bundle build.
- `bun run test:e2e`: browser preview smoke, not full desktop signoff.

## Deep Checks And Release Checks

Use these for milestone closeout, risky refactors, and release rehearsal:

```bash
bun run mutation:js
bun run mutation:rust
bun run mutation:rust:full
bun run test:desktop-bridge:rust
bun run test:e2e:desktop-bridge
bun run verify
```

`bun run verify` runs the release-style local sweep:

- `bun run check:full`
- `bun run build`
- `bun run desktop:build:debug`

Current recovery-mode note:

- mutation scripts are still available, but they are temporarily out of the default `check` / `check:full` / `verify` path while product recovery work is underway.

What they mean:

- `bun run mutation:rust`: current honest Rust mutation contract for `browser-history-parser` plus the `vault-core/src/ai.rs` status/helper slice (`ai_index_status`, `ai_queue_status`, `reconcile_ai_queue_controls`, `provider_capabilities`, `provider_connection_failure_report`, `test_provider_connection`).
- `bun run mutation:rust:full`: exploratory whole-workspace cargo-mutants sweep used to discover future backlog or deferred rationale; it is not the default signed-off contract.
- `bun run test:desktop-bridge:rust`: targeted Rust unit coverage for the feature-gated dev desktop bridge command dispatcher.
- `bun run test:e2e:desktop-bridge`: Chrome + Playwright smoke that drives the frontend through the dev-only desktop command bridge and proves browser automation can reach real Rust responses.

## Focused Commands

```bash
bun run test:unit
bun run test:unit:desktop-contract
bun run coverage:js:desktop-contract
bun run check:js
bun run check:rust
```

## Honest Boundaries

- The desktop-contract slice only protects `src/main.tsx` and `src/lib/ipc/bridge.ts`.
- Browser-preview e2e does not verify native scheduler install, keyring integration, signing, notarization, or filesystem side effects.
- Chrome desktop-bridge smoke verifies the typed desktop command facade from a real browser, but it still does not magically grant every Tauri guest API to Chrome. Treat it as an agent/dev-loop surface, not the final WebView plugin truth.
- Platform validation for macOS / Windows / Linux lives in [RELEASE.md](./RELEASE.md) and [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).
- User-facing support diagnostics and redaction rules live in [SUPPORT.md](./SUPPORT.md).

## Browser Support Truth

- Public browser support claims must follow [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md), not just the broadest code path currently implemented in the repo.
- `Validated now`: Google Chrome; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Microsoft Edge, Microsoft Edge Dev, Brave, Vivaldi, Arc, Opera, Opera GX, Firefox, LibreWolf, Floorp, Waterfox.

## Local Browser Validation Recipe

Use this recipe before promoting any browser into README or onboarding promise copy:

1. Verify one successful Google Chrome backup / recall path on the current local host.
2. Verify Safari remains visible but unreadable when `History.db` cannot be accessed.
3. Verify Safari baseline backup succeeds after Full Disk Access is granted.

Additional adapters may keep shipping as implementation coverage, but they stay out of public promise copy until the same recipe is documented for them.

## Release Closeout Command Order

Use this order for release rehearsal:

1. `bun run check`
2. `bun run coverage:js`
3. `bun run coverage:rust`
4. `bun run test:e2e`
5. `bun run build`
6. `bun run desktop:build:debug`

Run `bun run mutation:js` and `bun run mutation:rust` manually before release / milestone signoff while the temporary recovery-mode policy is in effect.

If the change touches packaging, release workflow, platform guidance, or troubleshooting copy, also perform the traceability sweep in [RELEASE.md](./RELEASE.md).
