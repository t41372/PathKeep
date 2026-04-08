# Release

This is the contributor-facing release runbook. The implementation-level source of truth for `WORK-M4-B` lives in [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).

## Release Channels

| Platform | Channel | Policy                                                                                                                         |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| macOS    | Primary | External releases should be signed and notarized before public distribution.                                                   |
| Windows  | Preview | CI builds installers, but maintainers must explicitly own the code-signing path they want to use.                              |
| Linux    | Preview | CI builds packages when host tooling is present; checksums are part of the release contract, signatures are not yet universal. |

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

## Required Validation Before Release

Run:

```bash
bun run verify
```

For release closeout or milestone handoff, also run:

```bash
bun run mutation:js
bun run mutation:rust
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

- resolves the tag and version up front
- verifies version sync across the repo
- builds release bundles on macOS, Windows, and Linux
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
- first local backup
- schedule preview / install / verify / remove
- encrypted archive unlock and re-open
- remote backup preview / execute / verify
- upgrade or reinstall over existing data
- uninstall expectations

Use the per-platform checklist in [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).

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
- Windows SmartScreen reputation depends on maintainer signing policy and reputation, not just a successful CI build.
- Linux keyring behavior varies by desktop environment; encrypted mode remains supported, but unattended unlock can degrade.
- App lock / biometric / passcode work is still blocked on `PG-RD-PLAT-006` and is intentionally not shipped as a fake front-end-only feature.

## External References

- [Tauri GitHub pipeline docs](https://v2.tauri.app/distribute/pipelines/github/)
- [Tauri macOS signing docs](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri Windows signing docs](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft SignTool docs](https://learn.microsoft.com/windows/win32/seccrypto/signtool)
