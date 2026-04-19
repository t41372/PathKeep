/**
 * This module owns the shell-level front-end read model, bootstrap hydration, and busy-state orchestration.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `ShellDataProvider`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { backend } from '../lib/backend-client'
import { subscribeToBackupProgress } from '../lib/ipc/backup-progress'
import { useI18nContext } from '../lib/i18n'
import type {
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  BackupProgressEvent,
  DashboardSnapshot,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from '../lib/types'
import { type BusyOverlayState, ShellDataContext } from './shell-data-context'

/**
 * Waits one frame so a busy overlay or loading transition can paint before the
 * next expensive async step begins.
 *
 * We use this in trust-critical flows such as backup and shell refresh where a
 * frozen frame would make PathKeep feel like it started work before explaining
 * what it was doing.
 */
function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    if (
      typeof window === 'undefined' ||
      typeof window.requestAnimationFrame !== 'function'
    ) {
      resolve()
      return
    }

    let settled = false
    /**
     * Settles the pending paint wait exactly once, regardless of which fallback
     * path wins first.
     */
    const finish = () => {
      if (settled) return
      settled = true
      resolve()
    }

    window.requestAnimationFrame(() => finish())
    window.setTimeout(finish, 16)
  })
}

/**
 * Detects the locked-state refusal messages that should kick the shell back
 * onto the explicit App Lock flow instead of surfacing a generic error.
 */
function isAppLockError(error: unknown) {
  return (
    error instanceof Error &&
    /currently locked|unlock the app|unlock pathkeep/i.test(error.message)
  )
}

/**
 * Builds a shell-safe dashboard fallback when the archive has not been
 * initialized yet but the dashboard read model still failed to load.
 *
 * This keeps the shell usable enough to reach onboarding instead of trapping
 * first-run desktop sessions behind a generic archive read error.
 */
function buildUninitializedDashboardFallback(
  snapshot: AppSnapshot,
): DashboardSnapshot {
  return {
    generatedAt: new Date().toISOString(),
    totalProfiles: 0,
    totalUrls: 0,
    totalVisits: 0,
    totalDownloads: 0,
    lastSuccessfulBackupAt: null,
    recentRuns: snapshot.recentRuns,
    storage: {
      archiveDatabaseBytes: 0,
      sourceEvidenceDatabaseBytes: 0,
      searchDatabaseBytes: 0,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      snapshotBytes: 0,
      exportBytes: 0,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 0,
      intelligenceBlobBytes: 0,
    },
    nextAction: null,
  }
}

/**
 * Returns whether the shell is allowed to try the best-effort keyring auto
 * unlock path during bootstrap.
 *
 * The point is not to hide archive locking; it is to avoid asking the user to
 * repeat an unlock step when they explicitly chose "remember key in keyring"
 * and the platform says a stored secret is available.
 */
function shouldAttemptKeyringAutoUnlock(snapshot: AppSnapshot) {
  return (
    snapshot.archiveStatus.encrypted &&
    !snapshot.archiveStatus.unlocked &&
    snapshot.config.rememberDatabaseKeyInKeyring &&
    snapshot.keyringStatus.available &&
    snapshot.keyringStatus.storedSecret
  )
}

/**
 * Provides the front-end shell read model and shell-level actions to the rest
 * of the app.
 *
 * This provider is where PathKeep turns desktop command responses, progress
 * events, app-lock state, and bootstrap errors into a single route-friendly
 * context instead of making every page talk to the backend on its own.
 */
export function ShellDataProvider({ children }: { children: ReactNode }) {
  const { setLanguagePreference, t } = useI18nContext()
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [appLockStatus, setAppLockStatus] = useState<AppLockStatus | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [busyOverlay, setBusyOverlay] = useState<BusyOverlayState | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const idleTimerRef = useRef<number | null>(null)
  const attemptedKeyringAutoUnlockRef = useRef(false)
  const surfacedCrashReportPathRef = useRef<string | null>(null)
  const loadingLatestArchiveState = t('shell.loadingLatestArchiveState')

  /**
   * Shows the shell-wide busy overlay for a long-running action.
   *
   * Centralizing this here keeps backup, initialize, lock, and config flows on
   * the same loading grammar instead of each action inventing its own spinner.
   */
  function showBusyOverlay(next: BusyOverlayState) {
    setBusyAction(next.label)
    setBusyOverlay(next)
  }

  /**
   * Clears the shell-wide busy overlay after a long-running action settles.
   */
  function clearBusyOverlay() {
    setBusyAction(null)
    setBusyOverlay(null)
  }

  const clearIdleTimer = useCallback(() => {
    if (typeof window === 'undefined' || idleTimerRef.current === null) {
      return
    }

    window.clearTimeout(idleTimerRef.current)
    idleTimerRef.current = null
  }, [])

  const clearLoadedState = useCallback(() => {
    setSnapshot(null)
    setDashboard(null)
  }, [])

  /**
   * Converts a low-level backup progress event into the readable busy-overlay
   * state shown by the shell.
   *
   * This is part of the PME honesty contract: users should see which phase a
   * backup is in, which profile is being processed, and roughly how far along
   * the run has moved.
   */
  function backupOverlay(progress: BackupProgressEvent): BusyOverlayState {
    const backupSteps = [
      t('shell.backupStepPrepare'),
      t('shell.backupStepArchive'),
      t('shell.backupStepRefresh'),
    ]
    const stepProgress =
      progress.totalSteps > 0
        ? (Math.min(progress.step + 1, progress.totalSteps) /
            progress.totalSteps) *
          100
        : null
    const profileCurrent =
      progress.phase === 'stage-profile' || progress.phase === 'ingest-profile'
        ? progress.completedProfiles + 1
        : progress.completedProfiles
    const profileDetail =
      progress.profileId && progress.totalProfiles > 0
        ? t('shell.backupProfileProgress', {
            profileId: progress.profileId,
            current: profileCurrent,
            total: progress.totalProfiles,
          })
        : null
    const progressLabel =
      progress.totalProfiles > 0
        ? `${profileCurrent.toLocaleString()} / ${progress.totalProfiles.toLocaleString()}`
        : `${Math.min(progress.step + 1, progress.totalSteps).toLocaleString()} / ${progress.totalSteps.toLocaleString()}`

    switch (progress.phase) {
      case 'prepare':
        return {
          label: t('shell.runningManualBackup'),
          detail: t('shell.runningManualBackupDetail'),
          progressLabel,
          progressValue: stepProgress,
          steps: backupSteps,
          activeStep: 0,
        }
      case 'stage-profile':
      case 'ingest-profile':
        return {
          label: t('shell.backupWritingArchive'),
          detail: profileDetail ?? t('shell.backupWritingArchiveDetail'),
          progressLabel,
          progressValue:
            progress.totalProfiles > 0
              ? (profileCurrent / progress.totalProfiles) * 100
              : stepProgress,
          steps: backupSteps,
          activeStep: 1,
        }
      case 'finalize':
        return {
          label: t('shell.refreshingArchiveViews'),
          detail: t('shell.backupFinalizeProgress', {
            current: progress.completedProfiles,
            total: progress.totalProfiles,
          }),
          progressLabel,
          progressValue:
            progress.totalProfiles > 0
              ? (progress.completedProfiles / progress.totalProfiles) * 100
              : stepProgress,
          steps: backupSteps,
          activeStep: 2,
        }
      default:
        return {
          label: t('shell.runningManualBackup'),
          detail: t('shell.runningManualBackupDetail'),
          progressLabel,
          progressValue: stepProgress,
          steps: backupSteps,
          activeStep: 0,
        }
    }
  }

  const refreshAppData = useCallback(
    async (showSpinner = true) => {
      if (showSpinner) {
        setLoading(true)
        await waitForNextPaint()
      }
      setError(null)

      try {
        const [nextLockStatus, nextBuildInfo] = await Promise.all([
          backend.loadAppLockStatus(),
          backend.getAppBuildInfo(),
        ])
        setAppLockStatus(nextLockStatus)
        setBuildInfo(nextBuildInfo)

        if (nextLockStatus.locked) {
          clearLoadedState()
          setNotice(null)
          setRefreshKey((value) => value + 1)
          return
        }

        let nextSnapshot = await backend.getAppSnapshot()
        if (
          !attemptedKeyringAutoUnlockRef.current &&
          shouldAttemptKeyringAutoUnlock(nextSnapshot)
        ) {
          attemptedKeyringAutoUnlockRef.current = true
          try {
            const key = await backend.keyringGetDatabaseKey()
            if (key) {
              await backend.setSessionDatabaseKey(key)
              nextSnapshot = await backend.getAppSnapshot()
            }
          } catch {
            // Auto-unlock is best-effort; keep the locked snapshot and let the
            // user fall back to the explicit unlock controls if keychain access
            // is denied or unavailable for this session.
          }
        }

        setLanguagePreference(nextSnapshot.config.preferredLanguage, {
          persist: false,
        })
        setSnapshot(nextSnapshot)
        setBuildInfo(nextBuildInfo)
        setAppLockStatus(nextSnapshot.appLockStatus)
        try {
          const nextDashboard = await backend.loadDashboardSnapshot()
          setDashboard(nextDashboard)
        } catch (dashboardError) {
          if (nextSnapshot.config.initialized) {
            throw dashboardError
          }

          setDashboard(buildUninitializedDashboardFallback(nextSnapshot))
        }
        setRefreshKey((value) => value + 1)
      } catch (nextError) {
        if (isAppLockError(nextError)) {
          try {
            const nextLockStatus = await backend.loadAppLockStatus()
            if (nextLockStatus.locked) {
              setAppLockStatus(nextLockStatus)
              clearLoadedState()
              setNotice(null)
              setRefreshKey((value) => value + 1)
              return
            }
          } catch {
            // Fall back to the generic error path below if the lock refresh fails.
          }
        }
        setError(
          nextError instanceof Error
            ? nextError.message
            : loadingLatestArchiveState,
        )
        throw nextError
      } finally {
        if (showSpinner) {
          setLoading(false)
        }
      }
    },
    [clearLoadedState, loadingLatestArchiveState, setLanguagePreference],
  )

  const armIdleDeadline = useEffectEvent((idleTimeoutMinutes: number) => {
    clearIdleTimer()

    idleTimerRef.current = window.setTimeout(() => {
      void backend
        .lockAppSession('idle-timeout')
        .then((nextStatus) => {
          setAppLockStatus(nextStatus)
          clearLoadedState()
          setNotice(null)
          setError(null)
          setRefreshKey((value) => value + 1)
        })
        .catch((nextError) => {
          setError(
            nextError instanceof Error
              ? nextError.message
              : t('shell.lockAppFailed'),
          )
        })
    }, idleTimeoutMinutes * 60_000)
  })

  useEffect(() => {
    void refreshAppData().catch(() => undefined)
  }, [refreshAppData])

  useEffect(() => {
    const crashReportPath =
      snapshot?.runtimeDiagnostics.latestCrashReport?.path ?? null
    if (
      !crashReportPath ||
      surfacedCrashReportPathRef.current === crashReportPath ||
      notice !== null
    ) {
      return
    }
    surfacedCrashReportPathRef.current = crashReportPath
    setNotice(t('shell.runtimeCrashNotice'))
  }, [notice, snapshot?.runtimeDiagnostics.latestCrashReport?.path, t])

  useEffect(() => {
    if (
      !appLockStatus?.enabled ||
      appLockStatus.locked ||
      busyAction !== null
    ) {
      clearIdleTimer()
      return
    }

    /**
     * Explains how schedule idle reset works.
     *
     * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
     */
    const scheduleIdleReset = () => {
      armIdleDeadline(appLockStatus.idleTimeoutMinutes)
    }

    /**
     * Handles visibility.
     *
     * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
     */
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        armIdleDeadline(appLockStatus.idleTimeoutMinutes)
      }
    }

    for (const eventName of ['pointerdown', 'keydown', 'mousemove', 'focus']) {
      window.addEventListener(eventName, scheduleIdleReset, { passive: true })
    }
    document.addEventListener('visibilitychange', handleVisibility)
    armIdleDeadline(appLockStatus.idleTimeoutMinutes)

    return () => {
      clearIdleTimer()
      for (const eventName of [
        'pointerdown',
        'keydown',
        'mousemove',
        'focus',
      ]) {
        window.removeEventListener(eventName, scheduleIdleReset)
      }
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [
    appLockStatus?.enabled,
    appLockStatus?.idleTimeoutMinutes,
    appLockStatus?.locked,
    busyAction,
    clearIdleTimer,
  ])

  /**
   * Explains how save config works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function saveConfig(config: AppConfig) {
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
      setRefreshKey((value) => value + 1)
      void backend
        .loadDashboardSnapshot()
        .then((nextDashboard) => setDashboard(nextDashboard))
        .catch(() => undefined)
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
  }

  /**
   * Explains how initialize archive works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function initializeArchive(
    config: AppConfig,
    databaseKey?: string | null,
  ) {
    showBusyOverlay({
      label: t('shell.preparingArchive'),
      detail: t('shell.preparingArchiveDetail'),
    })
    setNotice(null)
    setError(null)

    try {
      await waitForNextPaint()
      const nextSnapshot = await backend.initializeArchive(config, databaseKey)
      const nextDashboard = await backend.loadDashboardSnapshot()
      setLanguagePreference(nextSnapshot.config.preferredLanguage)
      setAppLockStatus(nextSnapshot.appLockStatus)
      setSnapshot(nextSnapshot)
      setDashboard(nextDashboard)
      setNotice(t('shell.initializedNotice'))
      setRefreshKey((value) => value + 1)
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
  }

  /**
   * Explains how run backup works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function runBackup() {
    const backupSteps = [
      t('shell.backupStepPrepare'),
      t('shell.backupStepArchive'),
      t('shell.backupStepRefresh'),
    ]
    /**
     * Explains how unsubscribe works.
     *
     * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
     */
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
        showBusyOverlay(backupOverlay(progress))
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
      await refreshAppData(false)
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
  }

  /**
   * Explains how set app lock passcode works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function setAppLockPasscode(request: SetAppLockPasscodeRequest) {
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
      await refreshAppData(false)
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
  }

  /**
   * Explains how clear app lock passcode works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function clearAppLockPasscode() {
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
      await refreshAppData(false)
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
  }

  /**
   * Explains how lock app session works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function lockAppSession(reason?: string | null) {
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
      setRefreshKey((value) => value + 1)
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
  }

  /**
   * Explains how unlock app session works.
   *
   * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
   */
  async function unlockAppSession(request: UnlockAppSessionRequest) {
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
      await refreshAppData(false)
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
  }

  return (
    <ShellDataContext.Provider
      value={{
        buildInfo,
        appLockStatus,
        snapshot,
        dashboard,
        loading,
        busyAction,
        busyOverlay,
        error,
        notice,
        refreshKey,
        refreshAppData: () => refreshAppData(),
        saveConfig,
        initializeArchive,
        runBackup,
        setAppLockPasscode,
        clearAppLockPasscode,
        lockAppSession,
        unlockAppSession,
        clearNotice: () => setNotice(null),
      }}
    >
      {children}
    </ShellDataContext.Provider>
  )
}
