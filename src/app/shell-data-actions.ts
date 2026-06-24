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
import type { BusyOverlayState } from './shell-data-context'
import {
  backupStepLabels,
  buildBackupOverlay,
  type ShellTranslator,
} from './shell-data-helpers'
import type { ShellTask } from './shell-tasks'

type StateSetter<T> = (value: T | ((current: T) => T)) => void

interface ShellDataActionDeps {
  t: ShellTranslator
  setLanguagePreference: I18nContextValue['setLanguagePreference']
  refreshDashboardSnapshot: (
    nextSnapshot: AppSnapshot,
    options?: { surfaceErrors?: boolean },
  ) => Promise<void> | void
  refreshAppData: (showSpinner?: boolean) => Promise<void>
  clearLoadedState: () => void
  showBusyOverlay: (next: BusyOverlayState) => void
  clearBusyOverlay: () => void
  setNotice: StateSetter<string | null>
  setError: StateSetter<string | null>
  setSnapshot: StateSetter<AppSnapshot | null>
  setAppLockStatus: StateSetter<AppLockStatus | null>
  setRefreshKey: StateSetter<number>
  archiveTasks?: ShellArchiveTaskActionHooks
}

interface ShellArchiveTaskActionHooks {
  beginBackupTask: () => { task: ShellTask } | { blockedBy: ShellTask }
  updateBackupTask: (taskId: string, progress: BackupProgressEvent) => void
  finishBackupTask: (taskId: string, report: BackupReport) => void
  failBackupTask: (taskId: string, message: string) => void
}

function incrementRefreshKey(setRefreshKey: StateSetter<number>) {
  setRefreshKey((value) => value + 1)
}

function formatShellActionError(
  nextError: unknown,
  command: string,
  t: ShellTranslator,
) {
  if (
    nextError instanceof Error &&
    isSafariAccessIssueMessage(nextError.message)
  ) {
    return t('shell.safariFullDiskAccessBackupError')
  }
  return describeError(nextError, command)
}

function isSafariAccessIssueMessage(message: string) {
  return (
    message.includes('Safari History.db is not readable yet') ||
    message.includes('Full Disk Access')
  )
}

function backupCompletionNotice(report: BackupReport, t: ShellTranslator) {
  if (report.dueSkipped) {
    return report.reason ?? t('shell.manualBackupDueWindow')
  }

  if (report.run) {
    return report.warnings.some(isSafariAccessIssueMessage)
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
  clearLoadedState,
  showBusyOverlay,
  clearBusyOverlay,
  setNotice,
  setError,
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
          throw new Error(
            t('jobs.archiveTaskAlreadyRunningBody', {
              task: taskStart.blockedBy.title,
            }),
          )
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
        const message = formatShellActionError(nextError, 'run_backup_now', t)
        setError(message)
        if (taskId) {
          archiveTasks?.failBackupTask(taskId, message)
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
  }
}
