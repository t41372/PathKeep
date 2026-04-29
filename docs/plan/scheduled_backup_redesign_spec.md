# Scheduled Backup Redesign Spec

Date: 2026-04-29  
Scope: Ticket A Phase 1 plus the design brief for Phase 2. Phase 3 implementation must wait for confirmed design mockups.

## Product Goal

Scheduled backup should read as a persistent system setting, not as a one-off operations task. Users need to answer four questions quickly:

1. Is native scheduled backup installed?
2. What interval is configured?
3. Can I install, update, or remove it here?
4. If I skip setup during onboarding, where do I find it later?

The route remains `/schedule`. Sidebar placement moves from `OPERATIONS` to `SYSTEM`. Navigation label changes to:

- English: `Scheduled Backup Settings`
- Simplified Chinese: `定时备份设置`
- Traditional Chinese: `定時備份設定`

## State Machine

`ScheduleStatus.installState` remains the source field. UI may derive a display state from `installState`, `SchedulePlan.applySupported`, busy state, and the local config draft.

| State                     | Meaning                                                                          | Primary actions                                      | Secondary actions                                           |
| ------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------- |
| `loading`                 | Preview/status still loading                                                     | None                                                 | Show skeleton/loading row                                   |
| `unavailable`             | Preview/status command failed                                                    | Retry by refreshing route                            | Show error detail                                           |
| `not-installed`           | No canonical schedule installed                                                  | Install schedule                                     | Change interval, view preview/manual steps                  |
| `installed`               | Canonical schedule matches current config                                        | Remove schedule                                      | Change interval, view config/status/audit                   |
| `update-needed`           | User changed interval or status reports mismatch with current settings           | Update schedule                                      | Revert unsaved interval, remove schedule, view diff/preview |
| `mismatch`                | Canonical native artifact exists but no longer matches current generated plan    | Reinstall schedule                                   | Remove schedule, view detected file, view warning           |
| `permission-warning`      | Native status could not inspect files/tasks                                      | View recovery guidance                               | Retry, copy/open detected path if available                 |
| `legacy-install-detected` | Known pre-rename LaunchAgent exists or is loaded                                 | Review legacy warning and install canonical schedule | Open/copy legacy path, manual removal guidance              |
| `manual-review`           | Platform has preview/manual artifacts but app-side apply/status is not supported | Follow manual steps                                  | Copy commands/files, view generated artifacts               |
| `busy-apply`              | Install/update request is running                                                | None                                                 | Busy overlay with current action                            |
| `busy-remove`             | Remove request is running                                                        | None                                                 | Busy overlay with current action                            |
| `action-error`            | Last apply/remove failed                                                         | Retry action                                         | Show error and keep current status visible                  |

State priority:

1. `loading` / `unavailable`
2. `busy-*`
3. `legacy-install-detected`
4. `permission-warning`
5. `mismatch` / `update-needed`
6. `installed`
7. `manual-review`
8. `not-installed`

Legacy wins over healthy installed display because duplicate legacy and canonical schedules can trigger multiple workers.

## Main Settings Page Requirements

The page should be rebuilt around a settings/control layout while keeping PME transparency available.

Top status band:

- Platform label and native mechanism.
- Install state badge.
- One sentence status explanation.
- Primary CTA based on state: install, update, remove, or manual setup.

Configuration panel:

- Backup trigger interval selector using the existing interval options at minimum (`6`, `12`, `24`, `72` hours) and preserving the current config field `dueAfterHours`.
- Health-check cadence display from `scheduleCheckIntervalHours`, read-only for this ticket.
- Selected browser profiles summary.
- Last successful backup.
- Scheduler label.
- Apply support / manual review state.

Action panel:

- `Install schedule` when `not-installed`.
- `Update schedule` when the user changed interval or current artifact is mismatched.
- `Remove schedule` when installed, mismatched, legacy-detected, or detected files exist.
- Buttons must disable while busy and must not hide the current status.
- `Remove schedule` removes only the current canonical schedule unless backend reports otherwise; legacy state must explain manual cleanup.

Review panel:

- Keep Preview / Manual / Execute / Verify tabs or an equivalent segmented control.
- Preview shows generated artifacts.
- Manual shows platform-specific steps.
- Execute shows commands plus install/remove CTAs.
- Verify shows install state, detected files, warnings, latest audit path, and last action result.

Error and warning copy:

- `not-installed`: no native schedule is installed yet.
- `installed`: scheduled backup is installed and matches current settings.
- `update-needed`: interval changed; install/update to write the new native schedule.
- `mismatch`: native schedule exists but does not match current settings.
- `permission-warning`: PathKeep cannot inspect the native scheduler; check file/task permissions.
- `legacy-install-detected`: an older background task is present. PathKeep will not migrate or remove it automatically because the namespace rename was a clean break.
- `manual-review`: this platform needs manual setup.

## Onboarding Requirements

Onboarding keeps a dedicated backup schedule step after Security and before Ready.

Happy path:

- Show concise interval options.
- Show a small native-platform preview summary.
- Let the user continue without installing the schedule immediately.
- If app-side apply is supported and design confirmation chooses to offer install during onboarding, use the same schedule action component/command path as the settings page. The first implementation may defer native install to the Schedule route if the design presents onboarding as config-only.

Skip path:

- Provide an explicit `Skip scheduled backup` action.
- Skip must not call `apply_schedule`, `remove_schedule`, or any backup mutation.
- Skip should preserve selected interval only if the user already changed it; otherwise leave current config untouched.
- After skip, show copy that scheduled backup can be configured later in `Scheduled Backup Settings` under `System`.

Suggested skip copy:

- English: `You can turn on scheduled backup later in System -> Scheduled Backup Settings.`
- Simplified Chinese: `你之后可以在“系统”里的“定时备份设置”中开启定时备份。`
- Traditional Chinese: `你之後可以在「系統」裡的「定時備份設定」中開啟定時備份。`

Ready summary:

- If schedule was skipped, summarize as `Scheduled backup: not installed yet`.
- If interval was selected but not installed, summarize as `Backup interval selected; native schedule not installed yet`.
- If installed during onboarding, summarize as `Scheduled backup installed`.

## Component And Data Boundaries

- Reuse existing commands: `preview_schedule`, `schedule_status`, `apply_schedule`, `remove_schedule`.
- Reuse config save path for `dueAfterHours`.
- Do not add a new Tauri command for Ticket A.
- Do not alter native detection or scheduler behavior in Ticket A.
- Do not alter backup worker execution.
- Prefer shared component extraction only if both Schedule page and Onboarding need the same interval/action UI. The shared owner should accept loaded `SchedulePlan` / `ScheduleStatus` and callbacks rather than fetch data itself.

## Phase 2 Design Brief

Use image generation to create mockups in PathKeep's existing desktop-app style:

- Quiet operational UI.
- Dense but scannable settings controls.
- No marketing hero.
- No nested cards.
- Strong status hierarchy without oversized decorative panels.
- Native desktop sidebar and topbar retained.
- Avoid purple/blue gradient dominance and decorative orbs.

Required mockups:

1. Main `Scheduled Backup Settings` page, `not-installed`.
2. Main page, `installed`.
3. Main page, error/attention state covering `legacy-install-detected` or `permission-warning`.
4. Onboarding backup schedule step with interval selection and preview.
5. Onboarding skip path hint.

Reference inputs for image generation:

- Current Schedule page screenshot.
- Current Onboarding schedule step screenshot.
- This state/action spec.

Design confirmation gate:

- Stop after mockups are generated.
- Do not implement Phase 3 UI until the design direction is confirmed.
