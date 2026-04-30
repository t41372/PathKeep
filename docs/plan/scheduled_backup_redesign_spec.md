# Scheduled Backup State-Machine Redesign Spec

Date: 2026-04-29  
Scope: user-directed `/schedule` redesign implementation. This supersedes the earlier Phase 2 mockup / Phase 3 waiting gate for Scheduled Backup Settings.

## Product Goal

Scheduled backup is a persistent system setting, not a one-off operations task. The route should answer five questions without exposing every possible operation at once:

1. What is PathKeep checking right now?
2. Is native scheduled backup installed and healthy?
3. What should I do next in this state?
4. What manual path exists if the automatic path is unavailable?
5. How do I recover if install, verification, repair, or removal fails?

The route remains `/schedule`. Sidebar placement remains under `SYSTEM` with this label set:

- English: `Scheduled Backup Settings`
- Simplified Chinese: `定时备份设置`
- Traditional Chinese: `定時備份設定`

## State Machine

`ScheduleStatus.installState` is still the native backend read-model field, but the route renders only the product-level states below. `useScheduleWorkflow` owns detection, busy progress, timestamps, action result feedback, and post-action transitions.

| UI state          | Meaning                                                             | Primary path                                                | Recovery path                                                                |
| ----------------- | ------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `CHECKING`        | Initial load or manual re-detect is reading preview/status          | Show spinner plus what is being checked                     | Finish with `checkedAt`; on failure show unavailable state with retry        |
| `NOT_INSTALLED`   | No canonical schedule is installed and no blocking issue is present | Configure interval and install automatically                | Expand manual install steps; after manual completion run detection again     |
| `INSTALLED_OK`    | Canonical schedule is installed, loaded, and matches current config | Verify, inspect details, update changed interval, or remove | Manual details remain available; verification can be rerun inline            |
| `INSTALLED_WARN`  | Installed but attention is needed                                   | Repair known issues, verify, update/reinstall, or remove    | Manual repair steps; only dismiss issues marked `dismissible` by the backend |
| `INSTALLED_ERROR` | Installed but PathKeep cannot trust operation or inspection         | Reinstall, remove, re-detect, or copy diagnostics           | Manual removal path and diagnostic payload for support                       |

State derivation rules:

- Loading, missing status, or stale preview/status pairs map to `CHECKING`.
- `ScheduleIssue.severity = error` and `permission-warning` map to `INSTALLED_ERROR`.
- `not-installed` maps to `NOT_INSTALLED`.
- `legacy-install-detected`, `mismatch`, `manual-review`, missing `lastSuccessfulBackupAt`, or a visible warning issue maps to `INSTALLED_WARN`.
- Everything else with an installed canonical schedule maps to `INSTALLED_OK`.

## Backend Contract

The frontend must consume typed read-model fields instead of parsing raw English warning strings.

- `SchedulePlan.manualStepDetails`: structured manual steps with copy keys, command/file content, optional directory path, and per-step capability flags.
- `ScheduleStatus.issues`: typed issue rows with severity, localized copy keys, concrete evidence, repair action, and dismissibility.
- `ScheduleStatus.verificationChecks`: typed check rows for canonical artifact presence, load/query status, mismatch, legacy evidence, and permission failures.
- `ScheduleStatus.checkedAt`: timestamp for the latest detection pass.
- `ScheduleStatus.lastAction`: optional durable action result when a native operation can report one.
- `ApplyResult.stepResults`: action-level verification rows for install/remove/repair progress.
- `warnings` remains for compatibility and diagnostics, but route copy must prefer typed `issues` / `verificationChecks`.

Existing commands stay in use:

- `preview_schedule`
- `schedule_status`
- `apply_schedule`
- `remove_schedule`

New command:

- `repair_schedule`: explicit user-triggered repair for known scheduler conflicts. macOS currently uses it only to remove known pre-rename PathKeep LaunchAgent labels after user confirmation; it must not silently migrate/remove unknown scheduler artifacts.

Native scheduler ownership:

- `vault-platform::scheduler` is a facade.
- `scheduler/macos.rs` owns launchd plist generation, install/remove, loaded checks, legacy detection, and legacy repair.
- `scheduler/windows.rs` owns declaration-free Task Scheduler XML generation and `schtasks` apply/status/remove. Status normalization tolerates `schtasks /Query /XML` returning an XML declaration even though generated install XML omits one.
- `scheduler/linux.rs` owns manual-review systemd timer artifacts.
- `scheduler/audit.rs` owns schedule audit artifact writing and latest-audit lookup.

## Route Workflow

`src/pages/schedule/use-schedule-workflow.ts` is the route-owned workflow owner:

- On mount, call `preview_schedule` and `schedule_status` once and render `CHECKING` until both complete.
- Manual re-detect resets the UI to `CHECKING`, then displays `偵測完成於 HH:MM:SS` / localized equivalent through `checkedAt` or the route timestamp.
- Install/update persists the draft interval first, refreshes the plan, then runs the native apply action.
- Remove calls `remove_schedule` and refreshes status afterward.
- Repair calls `repair_schedule` and refreshes status afterward.
- Verify reruns detection and renders typed verification checks inline.
- Copy diagnostics serializes typed status/issue/check evidence; it does not expose private browsing records.

Every async action must show inline progress with the current step count and a localized message. Completed actions must leave an inline result:

- Success: show concrete outcome, audit path when present, and follow-up status.
- Failure: show the error plus an actionable recovery path.
- No modal progress is allowed for schedule actions.

## State-Local UI Requirements

`CHECKING`:

- Spinner / loading affordance.
- Short explanation that PathKeep is checking the native scheduler and generated plan.

`NOT_INSTALLED`:

- Configuration block is editable before install: preset shortcuts `6h`, `12h`, `24h`, `72h` plus a custom whole-minute input with a minimum of 1 minute.
- Browser profile list is read-only and links to `/settings#settings-profiles` with explicit copy: `Settings > Browser Profiles`.
- Primary action is automatic install.
- Manual install expands into structured steps with command/file content and verification controls.

`INSTALLED_OK`:

- Summary shows interval, selected browsers, install method/platform, last successful backup or `尚未成功執行`, scheduler label, and last verification timestamp.
- Actions are verify, view details, update after interval change, and remove.
- Details show generated file contents, detected paths, audit path, and verification rows.

`INSTALLED_WARN`:

- Issues must explain the problem, why it matters, evidence, and consequence.
- Known legacy LaunchAgent warning uses repair action `repair-legacy`; it is not dismissible because duplicate background tasks can run twice.
- Non-blocking warnings may be dismissible only when backend marks them `dismissible`.
- Manual repair path remains visible and `我已完成操作` reruns detection.

`INSTALLED_ERROR`:

- The route must show concrete issue rows when available: unreadable plist/task, canonical agent not loaded, permission/query failure, or backend command failure.
- Available actions are reinstall, remove/manual removal, re-detect, and copy diagnostics.
- Manual removal path must never leave the user without a next step.

## Manual Mode

Manual mode is state-local and first-class for install, remove, repair, and verify:

- Each step has a one-line purpose.
- Each step has a collapsed reason/background section.
- Commands render as copyable code blocks.
- File creation/edit steps show full file contents and target path.
- Directory hints use the platform's reveal/open helper when the path is safe to open; otherwise the path remains visible for manual navigation.
- Steps can expose automatic and verification buttons only when `canAutoRun` / `canVerify` is true.
- `一鍵全部自動執行` calls the state-appropriate native action.
- `我已完成操作` reruns detection and shows verification results.

## Onboarding Boundary

The current state-machine redesign is route-owned. Onboarding keeps the existing schedule intent behavior from the previous closeout:

- The schedule setup step may collect the interval and install/skip intent. The same preset-plus-custom interval control is shared with `/schedule`.
- Native install still waits until archive initialization and optional keyring setup have completed.
- Skip never calls `apply_schedule`, `remove_schedule`, or `repair_schedule`; it only tells the user they can return to `System -> Scheduled Backup Settings`.

## Validation Requirements

- Rust scheduler tests must cover not installed, installed loaded, installed but not loaded, loaded without plist, mismatch, permission/read failure, legacy detection, and legacy repair.
- Vitest must cover `ScheduleUiState` mapping, workflow detection/action recovery, route rendering for all five states, custom interval persistence, manual mode controls, read-only profile link, and i18n parity.
- `bun run check` remains the per-commit gate.
- After code changes, relaunch the debug desktop app and validate `/schedule` with Computer Use so stale WebView state cannot be mistaken for current source.

## Implementation Validation (2026-04-29)

- `cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform repair_schedule -- --test-threads=1`, targeted Schedule / workflow / command Vitest slices, `bun run test:e2e`, and `bun run check` passed. The full check covered base checks, 100% JS/Rust coverage, browser-preview e2e, desktop-bridge truth gate, and desktop-contract mutation.
- Computer Use validation used the freshly built repo debug bundle at `src-tauri/target/debug/bundle/macos/PathKeep.app`, not the stale `/Applications/PathKeep.app`. The live `/schedule` page showed the new `INSTALLED_WARN` legacy state, integrated issue detail, `重新偵測` timestamp update, and expanded manual repair steps with plist content, command, open-path hint, and per-step verification controls.
- The truth pass deliberately did not click repair, reinstall, or remove controls, because those actions would mutate the user's real LaunchAgents without explicit confirmation. Native behavior remains covered by Rust/platform tests and the desktop bridge truth gate.
- `bunx tauri build --debug` produced the debug `.app` bundle needed for the truth pass, then failed during DMG bundling. That bundling failure was not used as a release gate for this work block.
- Follow-up Computer Use validation with explicit LaunchAgent mutation permission found one host-real edge case: both canonical and legacy jobs could remain loaded after their plist files were gone. The backend now reports canonical loaded-without-plist as `INSTALLED_ERROR`, and macOS remove/repair uses the `gui/$UID/<label>` service target so loaded jobs without plist files can be unloaded. The retest exercised reinstall, legacy repair, verify, details, interval update, remove, manual install fallback, and final reinstall; the host ended in `INSTALLED_OK` with `StartInterval=21600` and no known legacy service loaded.
- Custom interval follow-up: `/schedule` and Onboarding now keep `6h / 12h / 24h / 72h` as presets while allowing any positive whole-minute custom value. Non-preset values show in the custom field, persist to `dueAfterHours` as fractional hours when needed, regenerate preview/apply artifacts, and survive native reinstall/update validation.
