# Troubleshooting

This guide is user-facing. It focuses on the surfaces PathKeep already exposes in the UI: scheduler review, keyring posture, archive mode, remote backup PME, and build / path diagnostics.

## Collect These Facts First

Before changing settings or filing a bug report, capture:

- app version
- git short SHA
- platform and installer type
- archive mode (`Plaintext` or `Encrypted`)
- scheduler state
- keyring backend
- latest run ID or audit artifact path if the problem came from backup / import / rebuild / remote backup

The Settings page now exposes the app data root, archive database path, audit repository path, version, and git short SHA to make this easier.

## Scheduler

### The Schedule page says `manual-review`, `mismatch`, or `permission-warning`

- Open the Schedule page and compare the generated artifact with the installed one.
- Use the manual install instructions shown in the page instead of editing scheduler files blindly.
- If you previously installed an older PathKeep or BrowserHistoryBackup artifact, remove that legacy artifact first.

### Scheduler changes do not seem to apply

- Re-open the Schedule page and verify the install state after apply or remove.
- Open the scheduler audit artifact from the page and confirm which file PathKeep actually wrote.
- On Linux, remember that the supported path is `systemd --user` and behavior can vary if the user session is not persistent.

## Safari And Permissions

### Safari is visible but still unreadable

- On macOS, grant Full Disk Access to PathKeep and then run the backup again.
- PathKeep intentionally keeps the Safari profile visible instead of hiding it, so `needs access` is not the same thing as `unsupported`.

### A browser profile disappeared after reinstall or path changes

- Re-open Settings and confirm the archive data root did not move unexpectedly.
- Re-run onboarding or the browser/profile selection flow if the local browser path changed.
- If this happened after a platform reinstall, include the old and new data-root paths in your support report.

## Encryption, Unlock, And Keyring

### Encrypted archive keeps asking for the password

- Check the Security page to see whether the keyring backend is available.
- On Linux, encrypted mode is still supported even when no keyring is available, but the unlock must stay manual.
- If the session is locked after restart, that is expected unless you explicitly stored the convenience unlock secret in a supported keyring.

### You forgot the archive password

- PathKeep does not offer a fake recovery flow here.
- If you do not have another valid unlock path, treat the encrypted archive as unrecoverable.
- Do not delete the archive immediately; keep the files intact while you confirm whether another machine or keyring still has the unlock material.

## App Lock

### PathKeep keeps opening on the lock screen

- If App Lock is enabled, startup locked state is expected.
- Unlock with the App Lock passcode, not the archive encryption password unless you deliberately set them to the same value.
- If the app re-locks too aggressively, reduce the idle timeout in Settings after you unlock.

### Biometric unlock is unavailable

- On macOS, Touch ID can still be temporarily unavailable because of OS state, enrollment, lockout, or cancellation. PathKeep falls back to the App Lock passcode when that happens.
- On Windows and Linux, this is expected: the current build remains passcode-only and shows biometric controls as truthful unsupported / degradation state.
- App Lock is still a passcode-first UI session lock, even when Touch ID is available on macOS.

### You forgot the App Lock passcode

- PathKeep does not offer a fake recovery flow here.
- Use the lock screen or Settings to open the config path, then follow your local support / recovery process for resetting the UI session lock.
- Resetting App Lock does not recover or change the archive encryption password.

## Remote Backup

### Preview works but execute does not

- Confirm the remote config is saved.
- Confirm credentials are stored.
- Re-run `Preview` and make sure the bundle path, object key, and destination URL look correct before `Execute`.

### Verify fails

- If the archive is encrypted, unlock PathKeep before verifying the bundle.
- Review which verification check failed:
  - bundle version
  - required entries
  - checksum
  - restore readiness
- Keep the failing bundle and its checksum result. Do not overwrite it until you understand the mismatch.

## Derived State And AI

### Search or insights look stale after provider changes

- Open Settings and use the derived-state controls to rebuild or clear derived data.
- Remember that clearing derived state does not delete canonical archive history, manifests, or rollback data.
- If the issue is provider-specific, include the provider format, model, and the latest job or run ID in your report.

## What To Attach In A Support Report

- screenshots of the page that shows the warning
- exact error text
- latest run ID, audit path, or bundle path if available
- whether the issue reproduces in browser preview, desktop dev, or a packaged build

Do not attach:

- archive database files
- API keys
- master passwords
- remote backup secret values
- raw history exports unless a maintainer explicitly asks for a minimal repro fixture

For the full support checklist, use [SUPPORT.md](./SUPPORT.md).
