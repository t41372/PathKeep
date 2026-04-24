# M4 — Release Readiness Runbook

> `WORK-M4-B` closeout source of truth. This file turns the M4 release-polish WBS into an operator checklist that can be re-run without guessing.

## Exit State

- release workflow has version-sync preflight and produces updater `latest.json`, signatures, `SHA256SUMS.txt`, plus `RELEASE-MANIFEST.json`
- README / CONTRIBUTING / DEVELOPMENT / TESTING / RELEASE / TROUBLESHOOTING / SUPPORT are aligned with real product boundaries
- Settings and Maintenance expose release and support diagnostics needed by the docs: data root, archive DB, audit repo, version, git short SHA, and updater review state
- platform validation is explicit about what is stable, what is preview, and what is deferred
- bundle / keyring / data-root namespace is consistently `com.yi-ting.pathkeep`, with no automatic migration from `dev.codex.pathkeep`

## Artifact Matrix

| Artifact                             | Expected Source           | Expected Use                                                       |
| ------------------------------------ | ------------------------- | ------------------------------------------------------------------ |
| `bun run build` output               | local / CI                | browser bundle regression check                                    |
| `bun run desktop:build:debug` output | local / CI                | debug packaging rehearsal                                          |
| macOS release bundles                | GitHub `Release` workflow | signed / notarized public builds when Apple secrets are configured |
| Windows release bundles              | GitHub `Release` workflow | preview installers until operator-owned signing path is wired      |
| Linux release bundles                | GitHub `Release` workflow | preview packages with checksum verification                        |
| updater `latest.json`                | GitHub `Release` workflow | updater availability contract for desktop installs                 |
| updater signatures                   | GitHub `Release` workflow | updater install verification                                       |
| `SHA256SUMS.txt`                     | GitHub `Release` workflow | user-visible checksum validation                                   |
| `RELEASE-MANIFEST.json`              | GitHub `Release` workflow | operator-facing file inventory and traceability                    |

## Browser Support Promise

- `Validated now`: Google Chrome; ChatGPT Atlas on macOS, including Browser Direct local `History` import; Perplexity Comet on macOS, including Browser Direct local `History` import; Safari baseline on macOS after Full Disk Access is granted, including Browser Direct local `History.db` import.
- `Implemented, not yet publicly promised`: Chromium, Microsoft Edge, Microsoft Edge Dev, Brave, Vivaldi, Arc, Opera, Opera GX, Firefox, LibreWolf, Floorp, Waterfox.
- Promotion into README / onboarding / release docs follows [../../architecture/browser-support-and-adapter-playbook.md](../../architecture/browser-support-and-adapter-playbook.md), not just the broadest internal implementation surface.

## Platform Validation Matrix

### macOS

- install app bundle and confirm first-run onboarding succeeds
- verify first backup on Google Chrome
- verify ChatGPT Atlas Browser Direct `History` preview / execute / re-import dedupe / revert / restore against the validated macOS `com.openai.atlas/browser-data/host` layout; record schema coverage, aggregate counts, and time range only
- verify Perplexity Comet Browser Direct `History` preview / execute / re-import dedupe / revert / restore against the validated macOS `~/Library/Application Support/Comet` layout; record schema coverage, aggregate counts, and time range only
- verify Safari stays visible with needs-access guidance when Full Disk Access is missing
- verify Safari baseline backup succeeds after Full Disk Access is granted
- verify Safari Browser Direct `History.db` preview / execute / re-import dedupe / revert / restore after Full Disk Access is granted; record aggregate counts and time range only
- review LaunchAgent preview, apply, verify, and remove flow
- confirm encrypted archive unlock, restart, and re-open behavior
- verify App Lock passcode + Touch ID unlock path, including truthful Touch ID unavailable fallback
- run remote backup preview / execute / verify
- reinstall over an existing data directory and confirm archive is reused
- uninstall expectation: app removal does not silently delete the archive data root

Support stance:

- primary release channel
- external release should be signed and notarized

Known limitations:

- Safari baseline support and Browser Direct local `History.db` import still depend on Full Disk Access
- ChatGPT Atlas support is only validated for the macOS browser-history profile layout; Atlas workspace data, chats, tabs, bookmarks, suggestions, and Windows / Linux locations are not promised
- Perplexity Comet support is only validated for the macOS browser-history profile layout; Comet AI memory, Perplexity account/workspace data, chats, tabs, bookmarks, suggestions, and Windows / Linux locations are not promised

### Windows

- install packaged build and confirm onboarding, first backup, and Settings diagnostics
- review Task Scheduler XML preview and manual install path
- validate keyring availability and degradation messaging
- validate remote backup preview / execute / verify
- upgrade over an existing installation and confirm archive reuse
- uninstall expectation: generated scheduler XML / installed task must be removable through documented flow

Support stance:

- preview release channel
- internal path is supported; public code-signing strategy remains operator-owned

Known limitations:

- SmartScreen reputation and code signing are not repo-defaulted

### Linux

- install AppImage or native package available for the target environment
- confirm onboarding, first backup, and Settings diagnostics
- review `systemd --user` timer preview and manual install path
- validate keyring fallback warning path
- validate remote backup preview / execute / verify
- reinstall or replace the app binary without losing the existing data directory

Support stance:

- preview release channel
- checksummed packages are supported; desktop-environment variance is explicit

Known limitations:

- keyring behavior can differ between Secret Service / KWallet / no keyring
- packaging availability depends on runner and host package tools

## Cross-Platform Smoke

Every release rehearsal should cover:

- first run to first backup
- archive migration or reinstall reuse
- data directory move awareness
- encrypted archive unlock path
- remote backup verify and restore-readiness review
- scheduler apply / remove review
- rollback / doctor surfaces still reachable after import activity

## Final QA Sweep

Run:

```bash
bun run check
bun run coverage:js
bun run coverage:rust
bun run test:e2e
bun run mutation:js
bun run mutation:rust
bun run build
bun run desktop:build:debug
```

If you are explicitly doing broader Rust mutation triage beyond the signed-off parser + AI helper contract, also run:

```bash
bun run mutation:rust:full
```

Then perform a traceability sweep:

- root docs point to the right commands and support stance
- Settings labels match the docs
- plan docs, features docs, and design docs do not claim unsupported behavior
- browser promise copy only names adapters that have current validation evidence
- blocked work remains explicitly blocked instead of silently implied as shipped

Then generate the release size audit:

```bash
bun run release:size-audit
```

## Deferred With Rationale

- Windows code signing strategy is not hardcoded because the correct operator path depends on who owns the certificate and trust chain
- Linux signing is not treated as a universal requirement because distribution channels and desktop environments vary
- App Lock 已正式 shipping 為 session-only boundary；只有 macOS 現在有真正的 Touch ID integration，Windows / Linux native biometric 仍 deferred
- namespace rename 採 clean break；若要保留既有 `dev.codex.pathkeep` 本機資料，維運者必須手動搬移
