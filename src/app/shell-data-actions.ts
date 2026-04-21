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
import { subscribeToBackupProgress } from '../lib/ipc/backup-progress'
import { waitForNextPaint } from '../lib/wait-for-next-paint'
import type {
  AppConfig,
  AppLockStatus,
  AppSnapshot,
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

type StateSetter<T> = (value: T | ((current: T) => T)) => void

interface ShellDataActionDeps {
  t: ShellTranslator
  setLanguagePreference: I18nContextValue['setLanguagePreference']
  refreshDashboardSnapshot: (
    nextSnapshot: AppSnapshot | null,
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
}

function incrementRefreshKey(setRefreshKey: StateSetter<number>) {
  setRefreshKey((value) => value + 1)
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
}: ShellDataActionDeps) {
  return {
    /**
     * Persists the latest app configuration, updates shell language immediately,
     * and refreshes dashboard data against the newly selected archive scope.
     */
    saveConfig: async (config: AppConfig) => {
      showBusyOverlay({
        label: t('shell.savingArchiveChoices'),
        detail: t('shell.savingArchiveChoicesDetail'),
      })
      setNotice(null)
      setError(null)

      try {
        await waitForNextPaint()
        const nextSnapshot = await backend.saveConfig(config)
        setLanguagePreference(nextSnapshot.config.preferredLanguage)
        setAppLockStatus(nextSnapshot.appLockStatus)
        setSnapshot(nextSnapshot)
        incrementRefreshKey(setRefreshKey)
        void refreshDashboardSnapshot(nextSnapshot)
        return nextSnapshot
      } catch (nextError) {
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.savingSettingsFailed'),
        )
        throw nextError
      } finally {
        clearBusyOverlay()
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
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.initializeArchiveFailed'),
        )
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

      showBusyOverlay({
        label: t('shell.runningManualBackup'),
        detail: t('shell.runningManualBackupDetail'),
        progressLabel: `1 / ${backupSteps.length.toLocaleString()}`,
        progressValue: 33,
        steps: backupSteps,
        activeStep: 0,
      })
      setNotice(null)
      setError(null)

      try {
        unsubscribe = await subscribeToBackupProgress((progress) => {
          showBusyOverlay(buildBackupOverlay(progress, t))
        })
        await waitForNextPaint()
        showBusyOverlay({
          label: t('shell.backupWritingArchive'),
          detail: t('shell.backupWritingArchiveDetail'),
          progressLabel: `2 / ${backupSteps.length.toLocaleString()}`,
          progressValue: 67,
          steps: backupSteps,
          activeStep: 1,
        })
        const report = await backend.runBackupNow(false)
        showBusyOverlay({
          label: t('shell.refreshingArchiveViews'),
          detail: t('shell.refreshingArchiveViewsDetail'),
          progressLabel: `3 / ${backupSteps.length.toLocaleString()}`,
          progressValue: 100,
          steps: backupSteps,
          activeStep: 2,
        })
        void refreshAppData(false)
        setNotice(
          report.dueSkipped
            ? (report.reason ?? t('shell.manualBackupDueWindow'))
            : report.run
              ? t('shell.manualBackupFinished', { runId: report.run.id })
              : t('common.complete'),
        )
        return report
      } catch (nextError) {
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.manualBackupFailed'),
        )
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
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.setAppLockPasscodeFailed'),
        )
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
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.clearAppLockPasscodeFailed'),
        )
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
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.lockAppFailed'),
        )
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
        setError(
          nextError instanceof Error
            ? nextError.message
            : t('shell.unlockAppFailed'),
        )
        throw nextError
      } finally {
        clearBusyOverlay()
      }
    },
  }
}
