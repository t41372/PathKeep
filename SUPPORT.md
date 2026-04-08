# Support

PathKeep support is metadata-first. We need enough context to debug platform, scheduler, archive, or release issues without normalizing the habit of sending private history data or secrets.

## Before Filing An Issue

1. Read [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).
2. Reproduce the issue once with the exact steps written down.
3. Capture the diagnostics listed below.
4. File the GitHub bug report template instead of opening a blank issue.

## Diagnostics To Include

- app version
- git short SHA
- platform and installer type
- archive mode
- scheduler install state
- keyring backend and whether a convenience secret is stored
- latest run ID, audit artifact path, or remote bundle path if relevant
- whether the problem happened in browser preview, desktop dev, or a packaged release

## Redaction Rules

Share these by default:

- warning text
- error codes
- run IDs
- audit artifact paths
- checksum or verification status
- sanitized screenshots

Do not share these by default:

- archive database files
- raw history exports
- master passwords
- API keys
- S3 access keys or secret access keys
- full prompt or note bodies if they contain personal browsing data not needed for the bug

## Support Bundle Policy

PathKeep does not currently ship an automatic support-bundle exporter. The intended operator path is still manual and redacted:

- capture metadata from Settings, Schedule, Security, Audit, or Remote Backup verification
- attach only the smallest artifact needed to reproduce or confirm the issue
- prefer audit/report files over canonical archive data

## Maintainer Expectations

If you ask a user for extra artifacts:

- explain why you need them
- ask for the narrowest possible file set
- prefer derived or audit artifacts over canonical history data
- explicitly remind the user not to send secrets
