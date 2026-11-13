# Release

This is the contributor-facing release runbook. The implementation-level source of truth for `WORK-M4-B` lives in [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md).

## Release Channels

| Platform | Channel | Policy                                                                                                                         |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| macOS    | Primary | External releases should be signed and notarized before public distribution.                                                   |
| Windows  | Preview | CI builds unsigned MSI / NSIS installers. Code signing is optional future hardening, not a release blocker.                    |
| Linux    | Preview | CI builds packages when host tooling is present; checksums are part of the release contract, signatures are not yet universal. |

## Browser Support Promise

- `Validated now`: Google Chrome; Microsoft Edge / Edge Dev; Firefox history-only baseline; ChatGPT Atlas on macOS; Perplexity Comet on macOS; Safari baseline on macOS after Full Disk Access is granted.
- `Implemented, not yet publicly promised`: Chromium, Brave, Vivaldi, Arc, Opera, Opera GX, LibreWolf, Floorp, Waterfox.
- Promotion into README / onboarding / release claims requires the gate in [docs/architecture/browser-support-and-adapter-playbook.md](./docs/architecture/browser-support-and-adapter-playbook.md).

## Artifact Matrix

| Artifact                            | Produced By                           | Audience              | Notes                                                                                                                                                                         |
| ----------------------------------- | ------------------------------------- | --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser bundle                      | `bun run build`                       | CI / local validation | Confirms the frontend bundle still builds.                                                                                                                                    |
| Debug desktop binary                | `bun run desktop:build:debug`         | Maintainers           | Used for pre-release smoke and packaging rehearsal.                                                                                                                           |
| macOS `.app` / `.dmg`               | GitHub `Release` workflow             | Users                 | Signed / notarized only when Apple secrets are configured.                                                                                                                    |
| Windows installers                  | GitHub `Release` workflow             | Users                 | Unsigned MSI / NSIS outputs stay small and run the WebView2 download bootstrapper only when the runtime is missing; `Unknown Publisher` and SmartScreen prompts are expected. |
| Windows test binary / installers    | GitHub `Windows Test Binary` workflow | Maintainers / QA      | Manual workflow artifact only; builds unsigned Windows release binary plus MSI / NSIS installers without updating GitHub Release assets.                                      |
| Linux `.AppImage` / `.deb` / `.rpm` | GitHub `Release` workflow             | Users                 | Requires Linux packaging dependencies on the runner.                                                                                                                          |
| `SHA256SUMS.txt`                    | GitHub `Release` workflow             | Users / operators     | Attached to every release.                                                                                                                                                    |
| `RELEASE-MANIFEST.json`             | GitHub `Release` workflow             | Operators / support   | Lists released files, sizes, and checksums for traceability.                                                                                                                  |

## Versioning Rules

Keep these three files aligned before tagging or dispatching a release:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Also update the `pathkeep-desktop` version in `src-tauri/Cargo.lock` and review that the lockfile diff changes only that version. `release:bump` updates the three declarations, not the lockfile; run Cargo metadata to refresh it and format the changed JSON before the full gate.

The GitHub `Release` workflow now fails fast if those versions drift or if the requested release tag does not match them.

To bump them together locally, run:

```bash
bun run release:bump -- 0.2.0
```

The canonical desktop namespace is now `com.yi-ting.pathkeep`.
This is a clean break: if you still need data from an older `dev.codex.pathkeep` install, move it manually before release validation.

## Required Validation Before Release

Run:

```bash
bun run verify
```

`bun run verify` runs `bun run check` first (lint, browser build, Rust checks, release-config check, and the desktop-bridge E2E suite), then adds the debug desktop build rehearsal.

Before a release candidate, also run the slower checks (supply-chain audit and host-matched platform tests):

```bash
bun run check:slow
```

Then perform the platform and traceability review from:

- [docs/plan/m4-full-polish/release-readiness-runbook.md](./docs/plan/m4-full-polish/release-readiness-runbook.md)
- [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
- [SUPPORT.md](./SUPPORT.md)

## GitHub Release Workflow

The proposed v0.5.0 contract is documented in
[release-publication.md](./docs/architecture/release-publication.md). It requires
user approval before this preparation PR is merged. The old `draft=false` build
path and inferred manual tag are retired by that proposal.

The workflow has three operations sharing one lock per target tag:

- `prepare` builds a private draft. A manual run requires `release_tag`; a tag
  push uses its tag name and also prepares a draft. Matrix builds stay parallel
  within the run. The draft belongs to the checked-out source SHA.
- `verify` checks the reviewed source SHA, release ID, manifest digest and every
  current asset without building or publishing.
- `publish` performs the same checks and promotes the reviewed draft without
  rebuilding. It requires explicit user approval and the administrator preflight
  confirmation. Run it from the frozen source ref.

For preparation:

If the source changes `.github/workflows/` relative to the default branch,
an owner first reserves the private draft using a credential with Contents
write and Workflows write (`workflow` scope for a classic token). Keep this
credential on the owner's machine. For this proposal's rehearsal:

Before reserving a new candidate, confirm there is no matching tag or release.
On a retry, claim the existing draft only when its full SHA and requested
stable/prerelease channel match; do not create a second draft.

```bash
candidate_sha="$(git rev-parse HEAD)"
gh release create v0.5.0 --repo t41372/PathKeep --draft \
  --target "$candidate_sha" --title 'PathKeep v0.5.0' \
  --notes-file docs/plan/releases/v0.5.0/notes.en.md
```

This creates an unpublished draft. Then dispatch preparation, which claims
that same source-owned draft with its contents-only Actions token. Once the
reviewed workflow definitions are on the default branch, normal preparation
can reserve its draft directly. No additional repository secret is required.

```bash
gh workflow run release.yml --ref release/prep-v0.5.0 \
  -f operation=prepare -f release_tag=v0.5.0 \
  -f prerelease=false -f platforms=all -f unsigned_preview=true
```

`release_tag` names the target; it does not check out that tag. Confirm the
selected ref is the reviewed source. Before a new candidate, confirm the target
tag and all matching releases, including drafts, do not exist. A retry can reuse
only a private draft for the same SHA; a different SHA needs deliberate cleanup
of the obsolete private candidate after all its jobs stop.

Preparation validates versions and signing mode, reserves the draft, builds the
selected matrix and attaches `SHA256SUMS.txt` and `RELEASE-MANIFEST.json`. The
latter records source SHA and workflow run. `unsigned_preview=true` uses
`--no-sign`, disables updater artifacts and skips `latest.json`. Updater-enabled
preparation requires its signing key.

Before publication, an administrator must check and enable GitHub immutable
releases. The setting is currently disabled; activation and the new publication
contract remain part of final approval. Inspect the JSON boolean, not HTTP 200:

```bash
gh api repos/t41372/PathKeep/immutable-releases \
  -H 'X-GitHub-Api-Version: 2026-03-10' --jq .enabled
```

After approval, enable it with `gh api --method PUT
repos/t41372/PathKeep/immutable-releases`, then check `.enabled` again. The
workflow token deliberately has no administration permission. Set
`immutable_releases_confirmed=true` only after the administrator preflight.

Verification/publication require `approved_release_id`, `approved_source_sha`
and `approved_manifest_sha256` from the reviewed candidate. Use
`operation=verify` first. Only after user approval use `operation=publish` with
the administrator confirmation. The requested `prerelease` channel must match
the draft during preparation, verification and publication. Before publication,
the source's complete workflow subtree must match the default branch; merge
the approved proposal first. Publication holds the preparation tag lock,
rechecks all current bytes and confirms an immutable result.

Metadata PATCH requests must include the approved `tag_name`, even for a
notes-only edit: omitting it removes the draft's tag association. Use
`gh release edit` or explicitly repeat the tag in an API update. Publication
also checks that its response retains the approved tag, source and channel.

Published assets and tags are locked server-side; client reads alone are not
atomic protection. Corrected binaries require a new version. Notes and the
latest/prerelease designation remain editable. Historical workflow definitions
are not retroactively changed; do not rerun old definitions to upload.

The [v0.5.0 review](./docs/plan/releases/v0.5.0/readiness.md) records the assessment,
upgrade boundary and notes. Remote receipts stay in the preparation PR/private
draft so the candidate commit remains frozen. Publication still needs approval.

## Secrets

### Always Used

- `GITHUB_TOKEN`

### macOS Signing / Notarization

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`

### Required Updater Signing

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The current Tauri config has `bundle.createUpdaterArtifacts=true`, so the `Release` workflow fails fast when `unsigned_preview=false` and `TAURI_SIGNING_PRIVATE_KEY` is not configured. Unsigned Windows installer builds use `unsigned_preview=true`; they do not need updater signing secrets and do not publish updater artifacts.

Set the updater private key as a repository Actions secret only before dispatching an updater-enabled release:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo t41372/PathKeep
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo t41372/PathKeep
```

### Windows Signing

PathKeep's Windows release path is unsigned. The installer is expected to show `Unknown Publisher`, and SmartScreen may require the user to choose **More info -> Run anyway** until the project has publisher reputation.

The CI release config must keep Windows buildable without Windows code-signing secrets. If maintainers later want signed Windows releases, wire that as an optional hardening path without making unsigned Windows installers fail:

- certificate thumbprint in Tauri config
- custom `signCommand`
- Azure Trusted Signing / Azure Key Vault

Do not gate Windows preview support on any of those providers.

## Windows Test Binary Workflow

For ad-hoc Windows QA without touching public release assets, run the `Windows Test Binary` workflow manually from GitHub Actions. It builds on `windows-latest`, uses the same unsigned Tauri override as the release preview path, and uploads a 14-day workflow artifact containing:

- `pathkeep-desktop.exe`
- generated Windows installer bundles such as MSI / NSIS setup outputs
- `SHA256SUMS.txt`
- `WINDOWS-TEST-MANIFEST.json`

This workflow is for test-machine handoff only. Promote a build through the `Release` workflow after versioning, release notes, and the full release checklist are complete.

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
- Windows unsigned installer download, `Unknown Publisher` / SmartScreen prompt path, WebView2 already-present path, missing-WebView2 bootstrapper path, first launch, and reinstall / upgrade over an existing install
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
- Windows installers are unsigned in the preview channel. SmartScreen reputation is not proof that the binary failed to build.
- Windows installers require internet access only on the minority of machines missing Microsoft Edge WebView2 Runtime.
- Linux keyring behavior varies by desktop environment; encrypted mode remains supported, but unattended unlock can degrade.
- App Lock remains a session-only boundary; only macOS currently ships a real Touch ID unlock path.

## External References

- [Tauri GitHub pipeline docs](https://v2.tauri.app/distribute/pipelines/github/)
- [Tauri macOS signing docs](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri Windows signing docs](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft SignTool docs](https://learn.microsoft.com/windows/win32/seccrypto/signtool)
