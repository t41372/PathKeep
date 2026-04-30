# Scheduled Backup Detection Audit

Date: 2026-04-29  
Scope: Ticket B Phase 1 audit plus the root-cause record needed by Ticket A. This document audits detection/status logic only. UI redesign belongs to `scheduled_backup_redesign_spec.md`.

## Current Host Truth

The local macOS host currently shows three different scheduler truths:

- `~/Library/LaunchAgents/dev.codex.browser-history-backup.backup.plist` exists and `launchctl print gui/501/dev.codex.browser-history-backup.backup` reports it as loaded with `state = spawn scheduled`.
- `~/Library/LaunchAgents/dev.codex.pathkeep.backup.plist` exists, but `launchctl print gui/501/dev.codex.pathkeep.backup` reports the service is not loaded.
- `launchctl print gui/501/com.yi-ting.pathkeep.backup` reports the canonical PathKeep service is not found.

That means the observed "macOS background task exists but UI says scheduled backup is not installed" is not just a frontend badge issue. The system has legacy scheduler artifacts from pre-`com.yi-ting.pathkeep` namespaces, while the current backend only evaluates the canonical label `com.yi-ting.pathkeep.backup`. The correct current product state is not "canonical schedule installed"; it is "legacy schedule detected and needs review."

## Current Code Path

The frontend calls:

- `preview_schedule` through `backend.previewSchedule()`
- `schedule_status` through `backend.scheduleStatus()`
- `apply_schedule` through `backend.applySchedule(plan)`
- `remove_schedule` through `backend.removeSchedule(plan)`

The Rust path is:

- `src-tauri/src/commands/schedule.rs`
- `src-tauri/src/worker_bridge/schedule.rs`
- `src-tauri/crates/vault-worker/src/schedule.rs`
- `src-tauri/crates/vault-platform/src/scheduler.rs`

`vault-worker` loads app config and passes `due_after_hours` plus `schedule_check_interval_hours` into `vault-platform::scheduler`. Worker backup execution is intentionally separate and is not audited here.

## Platform Mechanisms

### macOS

Current implementation:

- Generates a LaunchAgent plist with label from `vault-platform::test_support::schedule_label()`, defaulting to `com.yi-ting.pathkeep.backup`.
- Uses `RunAtLoad = true`.
- Uses `StartInterval = min(due_after_hours, schedule_check_interval_hours) * 3600`, rounded to whole minutes, so custom minute-level backup cadences can wake often enough while preserving the worker's `--due-only` guard.
- Program arguments run the desktop executable with `--worker backup --due-only`.
- Applies by writing the plist to `~/Library/LaunchAgents/<label>.plist`, then running `launchctl bootout gui/<uid> <label>` followed by `launchctl bootstrap gui/<uid> <plist_path>`.
- Removes by running `launchctl bootout gui/<uid> <label>` and deleting the canonical plist.
- Status only reads `~/Library/LaunchAgents/<current-label>.plist` and compares the file contents exactly with the current generated plist.

Correctness assessment for macOS 15 Sequoia:

- The LaunchAgent mechanism is appropriate for a per-user desktop backup worker.
- `RunAtLoad` plus `StartInterval` is valid launchd plist usage, but `StartInterval` can miss intervals during sleep. This is acceptable because PathKeep's worker uses `--due-only` and decides whether backup is actually due after wake/login.
- Status detection is incomplete. It misses legacy loaded/background tasks with old labels and does not verify whether the canonical job is actually loaded in launchd after the plist write.
- Exact plist string comparison is intentionally strict for current-plan mismatch, but it can report `mismatch` when only executable path or generated XML/plist formatting changes. That is acceptable if the UI gives a clear reinstall/update affordance.

High-priority issue:

- `SCHED-MAC-001`: legacy LaunchAgents from known pre-rename namespaces are invisible to status. This can make the UI say "not installed" while macOS still shows a PathKeep/Browser History Backup background task.

Medium-priority issues:

- `SCHED-MAC-002`: status does not verify `launchctl print gui/<uid>/<label>`, so a current plist can be reported as installed even when it is not loaded.
- `SCHED-MAC-003`: remove only targets the current canonical label; legacy tasks need explicit review/removal guidance rather than silent cleanup because namespace rename is a clean break.

### Windows

Current implementation:

- Generates Task Scheduler XML with a `LogonTrigger`, a repeating `TimeTrigger`, `StartWhenAvailable = true`, `MultipleInstancesPolicy = IgnoreNew`, `InteractiveToken`, and least-privilege run level. The generated XML intentionally omits the XML declaration so `schtasks /Create /XML` does not reject the file with `unable to switch the encoding`.
- Applies by writing the generated XML to the schedule directory and running `schtasks /Create /TN <label> /XML <file> /F`.
- Status runs `schtasks /Query /TN <label> /XML`.
- Status compares normalized XML text with the generated XML to decide `installed` versus `mismatch`.
- Remove runs `schtasks /Delete /TN <label> /F`.

Correctness assessment for Windows 11 24H2:

- `schtasks` is an appropriate v1 implementation surface and can create, query, and delete local scheduled tasks.
- `/Query /XML /TN <task>` is a valid way to inspect task XML and verify recurrence.
- `StartWhenAvailable = true` matches the missed-run recovery requirement.
- XML declaration handling is part of the Windows compatibility contract: PathKeep writes declaration-free UTF-8 XML and strips an optional declaration from queried XML before comparison.
- Exact normalized XML comparison may still report `mismatch` if Windows canonicalizes or injects fields into the returned XML. Existing tests stub this path, but release validation still needs a real Windows VM.

High-priority issue:

- `SCHED-WIN-002` fixed 2026-04-30: Windows `schtasks /Create /XML` rejected the generated file with `ERROR: The task XML is malformed. (1,40)::ERROR: unable to switch the encoding` when the XML declaration carried an explicit encoding. The generated XML now omits the declaration and status normalization tolerates queried XML with a declaration.

Medium-priority issue:

- `SCHED-WIN-001`: real Windows Task Scheduler can normalize XML beyond whitespace. If VM validation shows false mismatches, status should compare semantic fields instead of full XML text.

### Linux

Current implementation:

- Generates a systemd user service and timer.
- Timer uses `OnCalendar=<computed cadence>` and `Persistent=true`. For minute-level intervals that systemd calendar cannot express exactly, PathKeep chooses the nearest safe divisor wake cadence and lets the worker's `--due-only` guard enforce the configured backup interval.
- UI exposes manual steps only.
- `apply_supported = false`.
- Status returns `manual-review` with a warning that automatic detection is only implemented on macOS/Windows.
- Cron is not implemented.

Correctness assessment for Ubuntu 24.04 LTS, Fedora 41, and current Arch systemd:

- Using systemd user timers is the right primary implementation for mainstream Linux desktop distributions.
- `Persistent=true` with `OnCalendar=` is the correct missed-run mechanism; systemd documents that `Persistent=` only has an effect for timers configured with `OnCalendar=`.
- Keeping Linux as `manual-review` is truthful. It does not claim automatic install/status support that is not implemented.
- Cron is outside the current product implementation and should not be surfaced as supported.

High-priority issue:

- None confirmed from source inspection.

Medium-priority issue:

- `SCHED-LINUX-001`: `linux_on_calendar` emits simple `*-*-* 00/<hours>:00:00` expressions. Before Linux automatic apply/status is promoted, validate the generated calendar expressions with `systemd-analyze calendar` on Ubuntu, Fedora, and Arch.

## Root Cause For Ticket A Observation

The local host has legacy LaunchAgent artifacts:

- Loaded: `dev.codex.browser-history-backup.backup`
- Present but not loaded: `dev.codex.pathkeep.backup`
- Missing canonical service: `com.yi-ting.pathkeep.backup`

The current status code only checks `~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist`. Because the canonical file/service is absent, it reports `not-installed`. macOS still shows a background task because a legacy label remains loaded.

Therefore:

- Backend detection has a high-priority gap: it does not surface known legacy schedule artifacts.
- UI display is not the primary root cause for this specific host state, but the UI currently lacks a clear legacy-state workflow once backend detection exposes it.
- Ticket A should display `legacy-install-detected` as an attention state with install/update/remove guidance. Ticket B should expose the state without auto-migrating or auto-removing legacy tasks.

## High-Priority Fix List

Only the following issue is approved for Ticket B Phase 2 in this slice:

1. `SCHED-MAC-001`: detect known legacy macOS LaunchAgent labels and return `legacy-install-detected` with detected file/service evidence and a warning.

Out of scope for this slice:

- Rewriting backup execution.
- Migrating old LaunchAgents.
- Removing legacy LaunchAgents automatically.
- Replacing exact current-plan comparison with semantic plist/XML comparison.
- Promoting Linux automatic apply/status.

## Reference Anchors

- macOS launchd plist semantics: https://keith.github.io/xcode-man-pages/launchd.plist.5.html
- Windows `schtasks`: https://learn.microsoft.com/en-us/windows/win32/taskschd/schtasks
- Windows `schtasks /query`: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-query
- systemd timer semantics: https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html
