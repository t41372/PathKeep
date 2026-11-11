# Dependency maintenance

PathKeep follows current stable releases. A major version is a migration to
validate, not a reason to keep an old dependency. Keep an older release only
for a concrete incompatibility, with its reason and reproduction recorded.

## JavaScript and TypeScript

`package.json` declares the supported ranges and the Bun version; `bun.lock`
records the exact graph. Use the Bun version in `packageManager`, which
`oven-sh/setup-bun` also reads in CI. Modal setup reads the same field, rather
than keeping a second Bun version pin.

```sh
bun run outdated:js
bun run update:js
bun run check
bun run check:supply-chain
```

`update:js` checks latest stable versions across major-version boundaries.
Review their migration guides and keep related peers compatible. Refresh
transitive dependencies too; do not add a transitive package to the app's
direct dependencies just to update it. Bun 1.4.4 supports named transitive
updates without changing that ownership:

```sh
bun update <existing-transitive-package>
```

`audit:js` runs `bun audit` over the lockfile, including development and
transitive dependencies. It is part of `check:js` and the combined
`check:supply-chain`. New advisories fail the checks; an unavailable audit
service is not treated as a clean result. Rust retains its separate advisory,
license, source and dependency checks.

## Dependabot coverage

`.github/dependabot.yml` checks the Bun project daily, including major updates,
without a cooldown. Related JavaScript peers are grouped in one PR. Changes
still require review and the normal gate; there is no automatic merge.

Security updates and scheduled version updates are separate GitHub features.
On 2026-11-10 this repository had security updates enabled and no open alerts,
but no version-update configuration. Its GitHub SBOM exposed 43 npm entries,
including manifest ranges such as `vite@^8.0.3`, rather than the complete Bun
lockfile graph. It omitted transitive packages such as `dompurify` and
`react-router`. Therefore zero GitHub alerts did not establish a clean Bun
dependency graph. Keep the lockfile audit in CI even with Dependabot enabled.

References: [Dependabot Bun support](https://docs.github.com/en/code-security/reference/supply-chain-security/supported-ecosystems-and-repositories),
[dependency graph formats](https://docs.github.com/en/code-security/reference/supply-chain-security/dependency-graph-supported-package-ecosystems),
[security updates](https://docs.github.com/en/code-security/concepts/supply-chain-security/dependabot-security-updates).

## Current compatibility choices

- **TypeScript compiler and API.** `@typescript/native` aliases the current
  `typescript` 7 package and supplies `tsc`. The package named `typescript`
  aliases Microsoft's current `@typescript/typescript6` compatibility API.
  The current typescript-eslint supports the 6 API and rejects 7. This keeps
  the compiler current without disabling typed linting or using an unsupported
  API. Follow [Microsoft's side-by-side guidance](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0)
  until typescript-eslint supports the new API.
- **minimatch override.** Current ESLint and typescript-eslint still request
  minimatch 10. `overrides.minimatch` selects current 11 because
  [GHSA-g3hv-75wg-m249](https://github.com/advisories/GHSA-g3hv-75wg-m249)
  affects versions below 11.0.1. The consumers use the supported `Minimatch`
  and matching interfaces; validate the project's real lint, typecheck and
  full gate. Remove the override when upstream ranges accept the fixed major.
- **Tauri peers.** Updating `@tauri-apps/plugin-dialog` or
  `@tauri-apps/plugin-log` also updates the corresponding Rust crate to the
  same major/minor. Preserve the desktop identifier, IPC payloads and backend
  client boundary.
- **Node support.** Current jsdom requires Node 22.22.2+, 24.15.0+ or 26+.
  `engines.node` records those supported release lines; CI uses current Node
  22 and local validation also covers Node 26.

## Validation scope

The authority remains `bun run check`, including the real Rust backend and
all E2E configurations. A dependency audit is additional evidence about known
advisories, not a substitute for behavior tests. Keep each run's fixture,
results, report and logs. When a full local gate cannot fit on disk, use the
project's authorized Modal workflow with a hard timeout and terminate the
sandbox when the run ends.
