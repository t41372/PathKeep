# Release preparation and publication

Status: proposed in the v0.5.0 preparation PR. Changing the existing tag-push
publication contract requires the user's approval before this PR is merged.
No public release or repository immutability setting has been changed.

## Problem

The old workflow grouped runs by ref. Two refs could build the same release tag
at once. A publication check at the start was only a snapshot: Tauri's combined
build/upload action could later find a published release and replace its assets.
The first guard did not close that gap.

## Proposed contract

- A manual run names its release tag explicitly. Tag pushes use their tag name.
  Both enter the same concurrency group for that resolved target. Matrix builds
  within one preparation run remain parallel.
- Preparation always creates a private draft. A draft belongs to one source
  commit; another commit cannot reuse it. API failures stop preparation.
- The Tauri action receives both the reserved release ID and its explicit tag.
  The ID bypasses release lookup/creation; the tag pins updater URLs to this
  version instead of the mutable latest alias. This follows the action’s
  [release-ID routing](https://github.com/tauri-apps/tauri-action/blob/v0.6.2/src/index.ts)
  and [updater URL construction](https://github.com/tauri-apps/tauri-action/blob/v0.6.2/src/upload-version-json.ts).
- `prepare`, `verify` and `publish` are separate operations in the same workflow
  and share its tag lock. Publication performs no build. It requires the reviewed
  source SHA, release ID and manifest digest, then checks the current assets again.
- Stable publication sets the repository's latest release. Prereleases retain
  the current stable latest release, as required by the
  [release update API](https://docs.github.com/en/rest/releases/releases#update-a-release).
- Publishing requires explicit user approval. Repository immutable releases must
  be enabled and checked by an administrator before dispatching publication.
  The workflow token does not need administration permissions or a new secret.
- GitHub's server locks published assets and the tag. Client-side checks are not
  atomic and do not replace that protection. Publication checks the immutable
  result and performs no repair upload on failure.

GitHub documents the draft → attach all assets → publish sequence and the
server-side protection in [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases).
The administrator can inspect or enable the setting using the
[repository API](https://docs.github.com/en/rest/repos/repos#check-if-immutable-releases-are-enabled-for-a-repository).
The observed setting on 2026-11-13 was `enabled=false`; HTTP 200 alone does not
mean the feature is enabled.

## Trade-offs

The optional inferred tag and `draft=false` build path are retired. An explicit
target avoids ref-dependent locks, and separating publication keeps approved
binaries unchanged. Existing callers must use the new operation inputs.

A repository-wide lock would also stop concurrent builds, but would unnecessarily
couple different versions. Required tags let the workflow serialize precisely the
target whose draft it can change.

Published bytes require a new version to change. Titles, release notes and the
latest/prerelease designation can still be edited. Existing older releases and
historical workflow definitions are not retroactively protected; do not rerun
those definitions to upload assets.

This changes release operations, not archive formats or application behavior.
The macOS signed-primary policy remains in place; publishing this unsigned
v0.5.0 preview still needs its separate, explicit policy exception.
