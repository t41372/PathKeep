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
bun run verify
```

`bun run verify` runs the release-style local sweep:

- `bun run check:full`
- `bun run build`
- `bun run desktop:build:debug`

## Focused Commands

```bash
bun run test:unit
bun run test:unit:desktop-contract
bun run coverage:js:desktop-contract
bun run mutation:js:desktop-contract
bun run check:js
bun run check:rust
```

## Honest Boundaries

- The desktop-contract slice only protects `src/main.tsx` and `src/lib/ipc/bridge.ts`.
- Browser-preview e2e does not verify native scheduler install, keyring integration, signing, notarization, or filesystem side effects.
- Platform validation for macOS / Windows / Linux lives in [RELEASE.md](./RELEASE.md) and [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).
- User-facing support diagnostics and redaction rules live in [SUPPORT.md](./SUPPORT.md).

## Release Closeout Command Order

Use this order for release rehearsal:

1. `bun run check`
2. `bun run coverage:js`
3. `bun run coverage:rust`
4. `bun run test:e2e`
5. `bun run mutation:js`
6. `bun run mutation:rust`
7. `bun run build`
8. `bun run desktop:build:debug`

If the change touches packaging, release workflow, platform guidance, or troubleshooting copy, also perform the traceability sweep in [RELEASE.md](./RELEASE.md).
