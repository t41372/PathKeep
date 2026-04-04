# Contributing

## Ground Rules

- Keep the app local-first and audit-friendly.
- Prefer additive, reviewable changes over hidden automation.
- Do not remove provenance or revert data unless the feature explicitly requires a reversible user action.
- Keep UX changes aligned with the Stitch-approved direction instead of ad-hoc restyling.

## Development Setup

```bash
bun install
cargo install prek --locked
prek install
prek install --hook-type pre-push
```

## Daily Commands

```bash
bun run check
bun run build
bun run desktop:build:debug
```

For deeper validation:

```bash
bun run coverage
bun run test:e2e
bun run mutation:js
bun run mutation:rust
```

## Commit Style

Use Conventional Commits.

Examples:

- `feat(app): add takeout batch revert flow`
- `test(rust): add worker cli integration coverage`
- `docs(repo): add contribution guide and ci overview`

## Pull Request Checklist

- Run `bun run check`
- Run `bun run build`
- Run `bun run desktop:build:debug`
- Update docs if user-facing behavior changed
- Keep commit history reviewable and logically grouped

## Testing Strategy

- Frontend core logic under `src/lib` is expected to keep 100% unit-test coverage.
- UI shell behavior should be covered with React integration tests plus Playwright e2e smoke tests.
- Rust changes should include unit tests where logic lives and integration-style coverage through `vault-worker` when multiple crates interact.
- Mutation testing is configured for both JS and Rust. Use it for quality sweeps and when touching critical parsing, migration, or audit logic.

## Linting and Checkers

- JavaScript / TypeScript
  - Prettier
  - ESLint
  - `tsc`
  - Vitest
  - Playwright
  - Stryker
- Rust
  - rustfmt
  - clippy
  - cargo test
  - cargo llvm-cov
  - cargo mutants
  - cargo audit
  - cargo deny

## Pre-commit Hooks

The repo uses `.pre-commit-config.yaml` with `prek` as the preferred runner. `pre-commit` should also understand the same config, but `prek` is the intended fast path.

Useful commands:

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
