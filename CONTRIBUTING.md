# Contributing

PathKeep is mid-rewrite, local-first, and audit-first. We optimize for reviewable changes, honest product boundaries, and docs that stay in sync with the code instead of drifting into aspiration.

## Before You Start

- Read [README.md](./README.md) for product scope and current support stance.
- Use [DEVELOPMENT.md](./DEVELOPMENT.md) for environment setup and repo layout.
- Use [TESTING.md](./TESTING.md) for the real gate matrix.
- If your work touches release, packaging, platform validation, or support diagnostics, also read [RELEASE.md](./RELEASE.md), [TROUBLESHOOTING.md](./TROUBLESHOOTING.md), and [SUPPORT.md](./SUPPORT.md).

## Ground Rules

- Keep the app local-first and inspectable.
- Do not hide destructive or high-risk actions behind opaque automation.
- Do not silently weaken the trust model to simplify implementation.
- Update source docs in the same branch when product behavior, platform stance, or operator workflow changes.
- Treat browser preview, desktop shell, and release packaging as different surfaces with different acceptance criteria.

## Environment Setup

Required tools:

- Bun
- Rust `1.94.1`
- Git
- Tauri 2 host dependencies for your platform

Recommended Rust setup:

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

Optional pre-commit hooks:

```bash
cargo install prek --locked
prek install
prek install --hook-type pre-push
```

## Daily Workflow

Run the browser preview shell:

```bash
bun run dev
```

Run the desktop shell:

```bash
bun run desktop:dev
```

Mainline validation:

```bash
bun run check
bun run build
```

Release-style validation:

```bash
bun run verify
```

## Code And Docs Expectations

- Keep commits reviewable and logically scoped.
- Prefer colocated tests for new or substantially rewritten modules.
- Keep the docs truthful. If the UI or workflow no longer matches `README`, `RELEASE`, `TROUBLESHOOTING`, or the `docs/` source tree, update them in the same branch.
- Do not claim the desktop-contract gate covers the entire UI.
- Do not ship fake security affordances while platform research is still unresolved.

## Commit Style

Use Conventional Commits.

Examples:

- `feat(settings): expose build and archive diagnostics`
- `build(release): add release manifest preflight`
- `docs(release): add platform validation runbook`
- `test(app): cover settings diagnostics metadata`

## Pull Request Checklist

- `bun run check`
- `bun run build`
- Run any additional commands required by the surface you changed:
  - [TESTING.md](./TESTING.md)
  - [RELEASE.md](./RELEASE.md)
- Update docs if user-facing behavior or operator workflow changed.
- Mention explicit gaps, preview-only paths, or deferred risks.

## Release And Support Changes

Changes to packaging, installer behavior, troubleshooting copy, support diagnostics, or platform stance are not docs-only polish. They change the operator contract and must keep these files aligned:

- [RELEASE.md](./RELEASE.md)
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- [SUPPORT.md](./SUPPORT.md)
- [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md)
- Matching source docs under [`docs/`](./docs/)
