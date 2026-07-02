/**
 * @file shell-data-actions.ts
 * @description Shell action factory for config, backup, and app-lock mutations that should not live inline inside the provider state owner.
 * @module app/shell-data
 *
 * ## Responsibilities
 * - Build the shell action callbacks that mutate app state through the backend client.
 * - Keep busy-overlay, notice, and error handling consistent across shell-level mutations.
 * - Reuse the same dashboard refresh and shell refresh hooks after successful actions.
 *
 * ## Not responsible for
 * - Owning React state or deciding when polling/bootstrap effects run.
 * - Holding pure helper logic such as runtime scope keys or backup-progress mapping.
 *
 * ## Dependencies
 * - Depends on the backend client, backup-progress subscription surface, and shell-data helper functions.
 * - Consumed by `src/app/shell-data.tsx`.
 *
 * ## Performance notes
 * - Reuses the existing shell refresh paths instead of adding extra polling owners or redundant snapshot fetches.
 */

import { backend } from '../lib/backend-client'
import { describeError } from '../lib/errors'
import { subscribeToBackupProgress } from '../lib/ipc/backup-progress'
import { runLocalSemanticSetup } from '../lib/ipc/semantic-setup'
import { waitForNextPaint } from '../lib/wait-for-next-paint'
import type {
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  BackupProgressEvent,
  BackupReport,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from '../lib/types'
import type { I18nContextValue } from '../lib/i18n/context'
import type { BusyOverlayState, ShellErrorKind } from './shell-data-context'
import {
  backupStepLabels,
  buildBackupOverlay,
  type ShellTranslator,
} from './shell-data-helpers'
import type { ShellTask } from './shell-tasks'

type StateSetter<T> = (value: T | ((current: T) => T)) => void

/**
 * Marker `Error.name` for the "a backup/import is already running" guard. A
 * concurrency conflict is NOT a data-safety failure — the blocking task already
 * published a warning notification — so the backup action re-throws it WITHOUT
 * raising the red failure alert (severity-matched feedback, no crying wolf).
 */
const BACKUP_CONFLICT_ERROR_NAME = 'BackupConflictError'

interface ShellDataActionDeps {
  t: ShellTranslator
  setLanguagePreference: I18nContextValue['setLanguagePreference']
  refreshDashboardSnapshot: (
    nextSnapshot: AppSnapshot,
    options?: { surfaceErrors?: boolean },
  ) => Promise<void> | void
  refreshAppData: (showSpinner?: boolean) => Promise<void>
  /**
   * Re-reads the shell runtime status (AI queue + intelligence snapshot). The
   * onboarding local-semantic-search opt-in kicks this right after enqueuing the
   * index build so the running build job appears in the ambient task bar
   * immediately instead of waiting up to one idle poll cycle.
   */
  refreshRuntimeStatus: () => Promise<unknown>
  clearLoadedState: () => void
  showBusyOverlay: (next: BusyOverlayState) => void
  clearBusyOverlay: () => void
  setNotice: StateSetter<string | null>
  /**
   * Sets the user-facing error message. The provider wires this to a setter that
   * also resets `errorKind` to `null`, so a plain error never leaves a stale FDA
   * classification behind. The FDA path re-asserts the kind via `setErrorKind`
   * immediately after.
   */
  setError: StateSetter<string | null>
  /**
   * Sets the locale-independent error classification consumed by the shell to
   * decide whether to show remediation affordances (e.g. the Full Disk Access
   * deep-link). Only set to a non-null kind alongside a matching `setError`.
   */
  setErrorKind: (value: ShellErrorKind) => void
  /**
   * Preserves the RAW backend error text (untranslated) for the failure
   * surface's copy-able diagnostic report. Set right after `setError` on a
   * genuine failure; `setError` itself clears it first so it never drifts from
   * the message being shown.
   */
  setRawError: StateSetter<string | null>
  setSnapshot: StateSetter<AppSnapshot | null>
  setAppLockStatus: StateSetter<AppLockStatus | null>
  setRefreshKey: StateSetter<number>
  archiveTasks?: ShellArchiveTaskActionHooks
}

interface ShellArchiveTaskActionHooks {
  beginBackupTask: () => { task: ShellTask } | { blockedBy: ShellTask }
  updateBackupTask: (taskId: string, progress: BackupProgressEvent) => void
  finishBackupTask: (taskId: string, report: BackupReport) => void
  /**
   * Marks the backup task failed. `silent: true` records the failure in the
   * task ledger WITHOUT publishing a red danger bell — used for the
   * lock-required case, where the unlock gate is the remediation surface and a
   * loud failure notification would be crying wolf.
   */
  failBackupTask: (
    taskId: string,
    message: string,
    options?: { silent?: boolean },
  ) => void
}

function incrementRefreshKey(setRefreshKey: StateSetter<number>) {
  setRefreshKey((value) => value + 1)
}

/**
 * Classifies a raw shell-action failure into the user-facing message AND a
 * stable, locale-independent kind. The Full Disk Access decision is made on the
 * RAW backend error (which always carries the ASCII `"Full Disk Access"`
 * marker), never on the translated UI copy — so a zh-CN / zh-TW user still gets
 * the FDA remediation affordance even though the displayed string is Chinese.
 */
function formatShellActionError(
  nextError: unknown,
  command: string,
  t: ShellTranslator,
): { message: string; isFullDiskAccess: boolean } {
  if (
    nextError instanceof Error &&
    isFullDiskAccessIssueMessage(nextError.message)
  ) {
    return {
      message: t('shell.fullDiskAccessBackupError'),
      isFullDiskAccess: true,
    }
  }
  return {
    message: describeError(nextError, command),
    isFullDiskAccess: false,
  }
}

/**
 * Returns `true` when the error message indicates a Full Disk Access / macOS
 * permission denial. Covers both the legacy Safari-specific text (still
 * produced when only Safari is blocked) and the new generic backend contract
 * that always contains `"Full Disk Access"` for any browser that can't be read
 * due to a missing TCC entitlement.
 */
export function isFullDiskAccessIssueMessage(message: string) {
  return (
    message.includes('Safari History.db is not readable yet') ||
    message.includes('Full Disk Access')
  )
}

/**
 * Returns `true` when the backup failed because the archive is encrypted and
 * no session key is available. The gate handles these failures — no toast.
 */
export function isLockRequiredError(error: unknown): boolean {
  if (!(error instanceof Error)) return false
  const msg = error.message
  return (
    msg.includes('database key is required') ||
    msg.includes('encrypted archive requires') ||
    msg.includes('no session key') ||
    /archive.*locked/i.test(msg)
  )
}

function backupCompletionNotice(report: BackupReport, t: ShellTranslator) {
  if (report.dueSkipped) {
    return report.reason ?? t('shell.manualBackupDueWindow')
  }

  if (report.run) {
    return report.warnings.some(isFullDiskAccessIssueMessage)
      ? t('shell.safariFullDiskAccessBackupWarning', { runId: report.run.id })
      : t('shell.manualBackupFinished', { runId: report.run.id })
  }

  return t('common.complete')
}

/**
 * Creates the shell mutation callbacks that the provider publishes through context.
 *
 * Splitting these closures out keeps `ShellDataProvider` readable without changing the
 * public context contract or introducing a second state owner.
 */
export function createShellDataActions({
  t,
  setLanguagePreference,
  refreshDashboardSnapshot,
  refreshAppData,
  refreshRuntimeStatus,
  clearLoadedState,
  showBusyOverlay,
  clearBusyOverlay,
  setNotice,
  setError,
  setErrorKind,
  setRawError,
  setSnapshot,
  setAppLockStatus,
  setRefreshKey,
  archiveTasks,
}: ShellDataActionDeps) {
  return {
    /**
     * Persists the latest app configuration, updates shell language immediately,
     * and refreshes dashboard data against the newly selected archive scope.
     *
     * Pass `{ quiet: true }` for the all-auto-save Settings path: a tiny config
     * write fired on every individual toggle / select / blur must NOT throw the
     * blocking full-screen `BusyOverlay` (that would freeze the main thread on
     * each control and violates the fluidity constraint). In quiet mode the
     * snapshot / language / app-lock / dashboard refresh still run exactly the
     * same — only the overlay is suppressed, so the section's inline "Saved" chip
     * is the sole confirmation. Explicit, user-initiated archive-choice saves
     * (onboarding, schedule, jobs) leave `quiet` unset and keep the overlay.
     */
    saveConfig: async (config: AppConfig, options?: { quiet?: boolean }) => {
      const quiet = options?.quiet === true
      if (!quiet) {
        showBusyOverlay({
          label: t('shell.savingArchiveChoices'),
          detail: t('shell.savingArchiveChoicesDetail'),
        })
      }
      setNotice(null)
      setError(null)

      try {
        if (!quiet) {
          await waitForNextPaint()
        }
        const nextSnapshot = await backend.saveConfig(config)
        setLanguagePreference(nextSnapshot.config.preferredLanguage)
        setAppLockStatus(nextSnapshot.appLockStatus)
        setSnapshot(nextSnapshot)
        incrementRefreshKey(setRefreshKey)
        void refreshDashboardSnapshot(nextSnapshot)
        return nextSnapshot
      } catch (nextError) {
        setError(describeError(nextError, 'save_config'))
        throw nextError
      } finally {
        if (!quiet) {
          clearBusyOverlay()
        }
      }
    },

    /**
     * Initializes the archive, promotes the chosen language into the shared i18n
     * context, and seeds the shell with the freshly initialized snapshot.
     */
    initializeArchive: async (
      config: AppConfig,
      databaseKey?: string | null,
    ) => {
      showBusyOverlay({
        label: t('shell.preparingArchive'),
        detail: t('shell.preparingArchiveDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextSnapshot = await backend.initializeArchive(
          config,
          databaseKey,
        )
        setLanguagePreference(nextSnapshot.config.preferredLanguage)
        setAppLockStatus(nextSnapshot.appLockStatus)
        setSnapshot(nextSnapshot)
        setNotice(t('shell.initializedNotice'))
        incrementRefreshKey(setRefreshKey)
        void refreshDashboardSnapshot(nextSnapshot)
        return nextSnapshot
      } catch (nextError) {
        setError(describeError(nextError, 'initialize_archive'))
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },

    /**
     * Runs a manual backup while streaming worker progress into the shell-level busy overlay.
     *
     * The action keeps the shared PME wording intact and reuses the existing shell refresh path
     * once the worker reports completion.
     */
    runBackup: async (): Promise<BackupReport> => {
      const backupSteps = backupStepLabels(t)
      let unsubscribe = () => {}
      let taskId: string | null = null

      showBusyOverlay({
        label: t('shell.runningManualBackup'),
        detail: t('shell.runningManualBackupDetail'),
        progressLabel: t('shell.backupProgressPending'),
        progressValue: null,
        steps: backupSteps,
        activeStep: 0,
        background: true,
      })
      setNotice(null)
      setError(null)

      try {
        const taskStart = archiveTasks?.beginBackupTask()
        if (taskStart && 'blockedBy' in taskStart) {
          const conflict = new Error(
            t('jobs.archiveTaskAlreadyRunningBody', {
              task: taskStart.blockedBy.title,
            }),
          )
          conflict.name = BACKUP_CONFLICT_ERROR_NAME
          throw conflict
        }
        taskId = taskStart?.task.id ?? null
        unsubscribe = await subscribeToBackupProgress((progress) => {
          showBusyOverlay(buildBackupOverlay(progress, t))
          if (taskId) {
            archiveTasks?.updateBackupTask(taskId, progress)
          }
        })
        await waitForNextPaint()
        showBusyOverlay({
          label: t('shell.backupWritingArchive'),
          detail: t('shell.backupWritingArchiveDetail'),
          progressLabel: t('shell.backupRecordProgressPending'),
          progressValue: null,
          steps: backupSteps,
          activeStep: 1,
          background: true,
        })
        const report = await backend.runBackupNow(false)
        if (taskId) {
          archiveTasks?.finishBackupTask(taskId, report)
        }
        showBusyOverlay({
          label: t('shell.refreshingArchiveViews'),
          detail: t('shell.refreshingArchiveViewsDetail'),
          progressLabel: `3 / ${backupSteps.length.toLocaleString()}`,
          progressValue: 100,
          steps: backupSteps,
          activeStep: 2,
          background: true,
        })
        void refreshAppData(false)
        setNotice(backupCompletionNotice(report, t))
        return report
      } catch (nextError) {
        // A "backup already running" conflict is a benign concurrency guard, not a
        // data-safety failure — beginBackupTask already published a warning
        // notification. Raising the red failure alert for it would be crying wolf,
        // so re-throw without touching the error surface (the busy overlay still
        // clears in the `finally` below).
        if (
          nextError instanceof Error &&
          nextError.name === BACKUP_CONFLICT_ERROR_NAME
        ) {
          throw nextError
        }

        // When the archive is locked the unlock gate will appear automatically
        // (it shows whenever archiveStatus.encrypted && !unlocked). Set the
        // lock-required kind so the gate knows to retry the backup on success.
        // No toast — the gate IS the remediation surface.
        if (isLockRequiredError(nextError)) {
          const rawMessage = describeError(nextError, 'run_backup_now')
          if (taskId) {
            // Record the failed run, but suppress the danger bell: the unlock
            // gate (auto-shown while locked) is the remediation surface.
            archiveTasks?.failBackupTask(taskId, rawMessage, { silent: true })
          }
          try {
            await refreshAppData(false)
          } finally {
            // Clear any stale error text; the gate is the surface.
            setError(null)
            setRawError(null)
            setErrorKind('lock-required')
          }
          throw nextError
        }

        const { message, isFullDiskAccess } = formatShellActionError(
          nextError,
          'run_backup_now',
          t,
        )
        // The raw, untranslated backend text — preserved for the failure
        // surface's copy-able diagnostic report (the displayed `message` is the
        // localized FDA copy for permission denials).
        const rawMessage = describeError(nextError, 'run_backup_now')
        if (taskId) {
          archiveTasks?.failBackupTask(taskId, message)
        }
        try {
          // The backend has ALREADY recorded this attempt as a failed run; refresh so it surfaces in
          // the dashboard/audit (they read the shell snapshot, which only reloads on refresh).
          await refreshAppData(false)
        } finally {
          // ALWAYS surface the error afterward — even if the refresh itself throws — so a failed
          // backup is NEVER silent (not in the audit ledger, not in the banner). `refreshAppData`
          // resets the error, so the message + FDA classification + raw detail are re-asserted here,
          // last. (FDA is classified from the RAW error, never by parsing the translated `message`,
          // which would miss the non-ASCII locales.)
          setError(message)
          setRawError(rawMessage)
          setErrorKind(isFullDiskAccess ? 'full-disk-access' : 'backup')
        }
        if (nextError instanceof Error && message !== nextError.message) {
          throw new Error(message)
        }
        throw nextError
      } finally {
        unsubscribe()
        clearBusyOverlay()
      }
    },

    /**
     * Saves a new app-lock passcode and then refreshes shell bootstrap state so lock-sensitive
     * routes do not keep stale capabilities or recovery metadata.
     */
    setAppLockPasscode: async (request: SetAppLockPasscodeRequest) => {
      showBusyOverlay({
        label: t('shell.settingAppLockPasscode'),
        detail: t('shell.settingAppLockPasscodeDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextStatus = await backend.setAppLockPasscode(request)
        setAppLockStatus(nextStatus)
        void refreshAppData(false)
        return nextStatus
      } catch (nextError) {
        setError(describeError(nextError, 'set_app_lock_passcode'))
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },

    /**
     * Removes the saved app-lock passcode and rehydrates shell state so the UI
     * reflects the new unlock requirements immediately.
     */
    clearAppLockPasscode: async () => {
      showBusyOverlay({
        label: t('shell.clearingAppLockPasscode'),
        detail: t('shell.clearingAppLockPasscodeDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextStatus = await backend.clearAppLockPasscode()
        setAppLockStatus(nextStatus)
        void refreshAppData(false)
        return nextStatus
      } catch (nextError) {
        setError(describeError(nextError, 'clear_app_lock_passcode'))
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },

    /**
     * Locks the current app session, clears shell read models, and bumps the refresh key
     * so route guards and shell chrome all see the locked transition as one event.
     */
    lockAppSession: async (reason?: string | null) => {
      showBusyOverlay({
        label: t('shell.lockingApp'),
        detail: t('shell.lockingAppDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextStatus = await backend.lockAppSession(reason ?? null)
        setAppLockStatus(nextStatus)
        clearLoadedState()
        incrementRefreshKey(setRefreshKey)
        return nextStatus
      } catch (nextError) {
        setError(describeError(nextError, 'lock_app_session'))
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },

    /**
     * Unlocks the app session and then reuses the regular shell refresh path instead of
     * manually rebuilding every shell read model in two places.
     */
    unlockAppSession: async (request: UnlockAppSessionRequest) => {
      showBusyOverlay({
        label: t('shell.unlockingApp'),
        detail: t('shell.unlockingAppDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextStatus = await backend.unlockAppSession(request)
        setAppLockStatus(nextStatus)
        void refreshAppData(false)
        return nextStatus
      } catch (nextError) {
        setError(describeError(nextError, 'unlock_app_session'))
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },

    /**
     * Runs the REAL onboarding "Enable AI" opt-in in the background: downloads the on-device static
     * embedding model, then enqueues a full index build (which drains via the AI queue). This lives in
     * the shell-data layer — not inside the onboarding route — so the trigger survives leaving
     * onboarding (the route unmounts on navigate) and so it can compose with the shell's ambient task
     * bar (the enqueued index-build job surfaces there via the runtime poller).
     */
    startLocalSemanticSetup: async (): Promise<void> => {
      try {
        await runLocalSemanticSetup()
      } catch {
        // Best-effort + OPTIONAL: onboarding AI setup must never surface a blocking error.
        // The model-download phase already surfaces in the Activity page; the user can retry
        // the index build from Settings → AI. Swallow so a flaky download never breaks finish.
      } finally {
        // Make the enqueued index-build job appear in the ambient bar immediately instead of
        // waiting up to one 15s idle poll cycle.
        void refreshRuntimeStatus()
      }
    },
  }
}
