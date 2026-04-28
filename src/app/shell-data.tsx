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
import { useI18nContext } from '../lib/i18n'
import { waitForNextPaint } from '../lib/wait-for-next-paint'
import type {
  AppBuildInfo,
  AppLockStatus,
  AppSnapshot,
  DashboardSnapshot,
} from '../lib/types'
import { type BusyOverlayState, ShellDataContext } from './shell-data-context'
import { createShellDataActions } from './shell-data-actions'
import {
  buildUninitializedDashboardFallback,
  countActiveRuntimeJobs,
  isAppLockError,
  shouldAttemptKeyringAutoUnlock,
} from './shell-data-helpers'
import { useShellRuntimeStatus } from './shell-runtime-status'

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
  const [dashboardLoading, setDashboardLoading] = useState(false)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [busyOverlay, setBusyOverlay] = useState<BusyOverlayState | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const idleTimerRef = useRef<number | null>(null)
  const attemptedKeyringAutoUnlockRef = useRef(false)
  const surfacedCrashReportPathRef = useRef<string | null>(null)
  const dashboardRefreshTokenRef = useRef(0)
  const activeRuntimeJobsRef = useRef(0)
  const loadingLatestArchiveState = t('shell.loadingLatestArchiveState')
  const { runtimeStatus, refreshRuntimeStatus, resetRuntimeStatus } =
    useShellRuntimeStatus({
      snapshot,
      refreshKey,
      t,
    })

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

  // Stryker disable ArrayDeclaration: this callback only touches stable refs and browser timer APIs.
  const clearIdleTimer = useCallback(() => {
    // Stryker disable next-line ConditionalExpression: ShellDataProvider only runs in the browser; the window guard is defensive for non-DOM execution.
    if (typeof window === 'undefined' || idleTimerRef.current === null) {
      return
    }

    window.clearTimeout(idleTimerRef.current)
    idleTimerRef.current = null
  }, [])
  // Stryker restore ArrayDeclaration

  // Stryker disable ArrayDeclaration: resetRuntimeStatus is stable, so any constant dependency array preserves this callback contract.
  const clearLoadedState = useCallback(() => {
    setSnapshot(null)
    setDashboard(null)
    setDashboardLoading(false)
    resetRuntimeStatus()
  }, [resetRuntimeStatus])
  // Stryker restore ArrayDeclaration

  const refreshDashboardSnapshot = useCallback(
    async (
      nextSnapshot: AppSnapshot,
      options: { surfaceErrors?: boolean } = {},
    ) => {
      const { surfaceErrors = false } = options
      // Stryker disable next-line ArithmeticOperator: the token only needs a new unique value; increasing or decreasing is equivalent for equality checks.
      const refreshToken = dashboardRefreshTokenRef.current + 1
      dashboardRefreshTokenRef.current = refreshToken
      setDashboardLoading(true)

      try {
        const nextDashboard = await backend.loadDashboardSnapshot()
        if (dashboardRefreshTokenRef.current !== refreshToken) {
          return
        }
        setDashboard(nextDashboard)
      } catch (dashboardError) {
        if (dashboardRefreshTokenRef.current !== refreshToken) {
          return
        }

        if (nextSnapshot.config.initialized && surfaceErrors) {
          setError(
            dashboardError instanceof Error
              ? dashboardError.message
              : loadingLatestArchiveState,
          )
          return
        }

        setDashboard(buildUninitializedDashboardFallback(nextSnapshot))
      } finally {
        if (dashboardRefreshTokenRef.current === refreshToken) {
          setDashboardLoading(false)
        }
      }
    },
    [loadingLatestArchiveState],
  )

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
        setRefreshKey((value) => value + 1)
        void refreshDashboardSnapshot(nextSnapshot, { surfaceErrors: true })
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
    [
      clearLoadedState,
      loadingLatestArchiveState,
      refreshDashboardSnapshot,
      setLanguagePreference,
    ],
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

    const scheduleIdleReset = () => {
      armIdleDeadline(appLockStatus.idleTimeoutMinutes)
    }

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

  useEffect(() => {
    const activeRuntimeJobs = countActiveRuntimeJobs(runtimeStatus)
    const hadActiveRuntimeJobs = activeRuntimeJobsRef.current > 0
    activeRuntimeJobsRef.current = activeRuntimeJobs

    if (
      !hadActiveRuntimeJobs ||
      activeRuntimeJobs > 0 ||
      !snapshot?.config.initialized ||
      !snapshot.archiveStatus.unlocked
    ) {
      return
    }

    setRefreshKey((value) => value + 1)
    void refreshDashboardSnapshot(snapshot, { surfaceErrors: true })
  }, [refreshDashboardSnapshot, runtimeStatus, snapshot])

  const {
    saveConfig,
    initializeArchive,
    runBackup,
    setAppLockPasscode,
    clearAppLockPasscode,
    lockAppSession,
    unlockAppSession,
  } = createShellDataActions({
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
  })

  return (
    <ShellDataContext.Provider
      value={{
        buildInfo,
        appLockStatus,
        snapshot,
        dashboard,
        dashboardLoading,
        runtimeStatus,
        loading,
        busyAction,
        busyOverlay,
        error,
        notice,
        refreshKey,
        refreshAppData: () => refreshAppData(),
        refreshRuntimeStatus: () => refreshRuntimeStatus(),
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
