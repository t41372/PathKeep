# Release preparation and publication

Status: accepted on 2026-11-13 with the maintainer's release approval and PR #22.
Immutable releases were enabled and v0.5.0 was published from its reviewed frozen
source. [Publication receipt](../plan/releases/v0.5.0/release-publication.json).

## Problem

The old workflow grouped runs by ref. Two refs could build the same release tag
at once. A publication check at the start was only a snapshot: Tauri's combined
build/upload action could later find a published release and replace its assets.
The first guard did not close that gap.

## Contract

- A manual run names its release tag explicitly. Tag pushes use their tag name.
  Both enter the same concurrency group for that resolved target. Matrix builds
  within one preparation run remain parallel.
- Preparation uses only a private draft. A draft belongs to one source
  commit; another commit cannot reuse it. API failures stop preparation.
- The requested prerelease channel is checked on creation, reuse, verification
  and publication. A retry cannot silently change stable/prerelease intent.
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
- A source with workflow changes relative to the default branch needs a
  workflow-authorized credential for release creation/update under the
  [release API authorization rule](https://docs.github.com/en/rest/releases/releases).
  During proposal rehearsal, the owner reserves the private draft locally;
  the contents-only Actions token claims it. Publication requires the complete
  workflow subtree to match the default branch, so the approved proposal is
  merged first. Git tree IDs avoid the compare API's file-list limit.
- Every metadata PATCH explicitly repeats the approved tag; publication also
  repeats the approved channel. Omitting
  `tag_name` removes the association; GitHub's
  [release-edit client preserves it explicitly](https://github.com/cli/cli/blob/v2.103.3/pkg/cmd/release/edit/edit.go).
  The publication response is checked for the same tag, source and channel.
- GitHub's server locks published assets and the tag. Client-side checks are not
  atomic and do not replace that protection. Publication checks the immutable
  result and performs no repair upload on failure.

GitHub documents the draft → attach all assets → publish sequence and the
server-side protection in [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases).
The administrator can inspect or enable the setting using the
[repository API](https://docs.github.com/en/rest/repos/repos#check-if-immutable-releases-are-enabled-for-a-repository).
The initial preparation check on 2026-11-13 returned `enabled=false`; HTTP 200
alone does not mean the feature is enabled. Before v0.5.0 publication, the
administrator enabled it and confirmed `enabled=true`; the published release
then confirmed `immutable=true`.

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
The macOS signed-primary policy remains in place. The maintainer approved the
unsigned-preview exception for v0.5.0, which uses manual installation without
in-app automatic updates. The exception does not apply to future versions.
