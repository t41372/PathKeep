# v0.5.0 local release rehearsal

This evidence records preparation, not a published release. The source is
`89ed2040bbb0cc1a36450f7004a08a51024bf70b` plus the four version changes in
`docs/plan/releases/v0.5.0/release-version.patch`. The candidate has not passed
the complete release gate.

Read `checks.json` for command results and `EVIDENCE-MANIFEST.json` for hashes.
The text logs have ANSI colour escapes removed. `e2e/report/index.html` and
`e2e/results.json` record a startup timeout with zero tests executed.
`javascript-audit.json` is the complete dependency advisory response; it is not
a count of exploitable desktop bugs. `macos-app-inspection.json` describes the
locally built ARM64 app, whose DMG build failed. The web size audit excludes
old bundles inherited with the build cache and does not count the app under
the architecture-specific Cargo target directory.

The local unsigned app archive is retained in the ignored rehearsal directory;
its location and checksum are in `checks.json`. It has not passed installation
or launch acceptance and is not approved for public distribution. Rebuild from
the final reviewed release commit before producing public assets.

The complete review and three-language release-note drafts are in
`docs/plan/releases/v0.5.0/`. No GitHub release, tag, branch or workflow dispatch
was created by this rehearsal. No cloud compute was started.
