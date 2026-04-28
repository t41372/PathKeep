# Release

This is the contributor-facing release runbook. The implementation-level source of truth for `WORK-M4-B` lives in [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).

## Release Channels

| Platform | Channel | Policy                                                                                                                         |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| macOS    | Primary | External releases should be signed and notarized before public distribution.                                                   |
| Windows  | Preview | CI builds installers, but maintainers must explicitly own the code-signing path they want to use.                              |
| Linux    | Preview | CI builds packages when host tooling is present; checksums are part of the release contract, signatures are not yet universal. |

## Browser Support Promise

- `Validated now`: Google Chrome; Microsoft Edge / Edge Dev; Firefox history-only baseline; ChatGPT Atlas on macOS; Perplexity Comet on macOS; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Brave, Vivaldi, Arc, Opera, Opera GX, LibreWolf, Floorp, Waterfox.
- Promotion into README / onboarding / release claims requires the gate in [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md).

## Artifact Matrix

| Artifact                            | Produced By                   | Audience              | Notes                                                                                                           |
| ----------------------------------- | ----------------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------- |
| Browser bundle                      | `bun run build`               | CI / local validation | Confirms the frontend bundle still builds.                                                                      |
| Debug desktop binary                | `bun run desktop:build:debug` | Maintainers           | Used for pre-release smoke and packaging rehearsal.                                                             |
| macOS `.app` / `.dmg`               | GitHub `Release` workflow     | Users                 | Signed / notarized only when Apple secrets are configured.                                                      |
| Windows installers                  | GitHub `Release` workflow     | Users                 | MSI / NSIS outputs depend on the host bundle result; SmartScreen reputation depends on operator signing policy. |
| Linux `.AppImage` / `.deb` / `.rpm` | GitHub `Release` workflow     | Users                 | Requires Linux packaging dependencies on the runner.                                                            |
| `SHA256SUMS.txt`                    | GitHub `Release` workflow     | Users / operators     | Attached to every release.                                                                                      |
| `RELEASE-MANIFEST.json`             | GitHub `Release` workflow     | Operators / support   | Lists released files, sizes, and checksums for traceability.                                                    |

## Versioning Rules

Keep these three files aligned before tagging or dispatching a release:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

The GitHub `Release` workflow now fails fast if those versions drift or if the requested release tag does not match them.

To bump them together locally, run:

```bash
bun run release:bump -- 0.1.0
```

The canonical desktop namespace is now `com.yi-ting.pathkeep`.
This is a clean break: if you still need data from an older `dev.codex.pathkeep` install, move it manually before release validation.

## Required Validation Before Release

Run:

```bash
bun run verify
```

`bun run verify` runs the strict per-commit checker first, including coverage, browser build, browser-preview e2e, desktop-bridge truth, and desktop-contract JS mutation, then adds the debug desktop build rehearsal.

For long-running mutation investigation before a high-risk release candidate, use:

```bash
bun run check:deep
bun run mutation:js:full
bun run mutation:rust:full
```

Then perform the platform and traceability review from:

- [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md)
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- [SUPPORT.md](./SUPPORT.md)

## GitHub Release Workflow

Entry points:

1. Push a tag such as `v0.1.0`
2. Run the `Release` workflow manually from GitHub Actions

Manual workflow inputs:

- `draft`
- `prerelease`
- `release_tag` (optional explicit tag; defaults to `v<package.json version>`)

Workflow behavior:

- bump versions locally first with `bun run release:bump -- <semver>`
- verify the repo with `bun run verify`
- generate the local size attribution bundle with `bun run release:size-audit`
- resolves the tag and version up front
- verifies version sync across the repo
- builds release bundles on macOS, Windows, and Linux
- builds updater artifacts and publishes `latest.json`
- uploads assets to the GitHub Release
- downloads the assets again
- publishes `SHA256SUMS.txt`
- publishes `RELEASE-MANIFEST.json`

## Secrets

### Always Used

- `GITHUB_TOKEN`

### macOS Signing / Notarization

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`

### Optional Updater Signing

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

### Windows Signing

PathKeep does not hardcode a single Windows signing provider in repo config. If you want signed Windows releases, choose and wire one operator-owned path before GA:

- certificate thumbprint in Tauri config
- custom `signCommand`
- Azure Trusted Signing / Azure Key Vault

Until that is configured, Windows stays an explicit preview channel.

## Platform Validation

Every release rehearsal should cover:

- fresh install
- first-run onboarding
- first local backup on Google Chrome, Microsoft Edge, and Firefox
- Browser Direct preview / execute / re-import / revert / restore on Chrome, Edge, and Firefox, with Edge metadata preserved and Firefox kept history-only
- Safari visible-but-unreadable guidance before Full Disk Access
- Safari baseline backup after Full Disk Access is granted
- schedule preview / install / verify / remove
- Windows Task Scheduler apply / status / mismatch or not-installed / remove on a real Windows host or VM
- encrypted archive unlock and re-open
- remote backup preview / execute / verify
- upgrade or reinstall over existing data
- uninstall expectations

Use the per-platform checklist in [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).

## Size Audit

For release closeout, also generate the size attribution bundle:

```bash
bun run release:size-audit
```

## Rollback Plan

If a release is bad:

- bad migration or archive compatibility issue:
  stop distribution, mark the release draft or prerelease as withdrawn, and keep users on the previous build until a fixed binary is available
- bad scheduler artifact:
  publish a patched build and direct users to remove the generated scheduler artifact via the in-app Schedule page or the documented manual removal path
- AI or derived-state regression:
  disable the provider or derived-state toggle in Settings, then rebuild or clear derived state; do not ask users to delete canonical archive data
- remote backup regression:
  stop telling users to rely on new bundles, keep existing local archive data as the source of truth, and use bundle verification results plus checksums to scope impact

## Known Limitations

- Safari access on macOS still depends on Full Disk Access outside the app.
- Firefox support is a history-only baseline in this release; Firefox favicons, downloads, keyword-search sidecars, and richer `moz_*` evidence remain future work.
- ChatGPT Atlas / Perplexity Comet support remains scoped to the validated macOS browser-history profile layouts; Windows / Linux locations are not public release promises.
- Windows SmartScreen reputation depends on maintainer signing policy and reputation, not just a successful CI build.
- Linux keyring behavior varies by desktop environment; encrypted mode remains supported, but unattended unlock can degrade.
- App Lock remains a session-only boundary; only macOS currently ships a real Touch ID unlock path.

## External References

- [Tauri GitHub pipeline docs](https://v2.tauri.app/distribute/pipelines/github/)
- [Tauri macOS signing docs](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri Windows signing docs](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft SignTool docs](https://learn.microsoft.com/windows/win32/seccrypto/signtool)
