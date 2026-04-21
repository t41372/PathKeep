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
import {
  type BusyOverlayState,
  ShellDataContext,
  type ShellRuntimeStatus,
} from './shell-data-context'
import { createShellDataActions } from './shell-data-actions'
import {
  buildUninitializedDashboardFallback,
  countActiveRuntimeJobs,
  emptyRuntimeStatus,
  isAppLockError,
  runtimeStatusScopeKey,
  shouldAttemptKeyringAutoUnlock,
} from './shell-data-helpers'

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
  const [runtimeStatus, setRuntimeStatus] =
    useState<ShellRuntimeStatus>(emptyRuntimeStatus)
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
  const runtimeRefreshPromiseRef = useRef<Promise<ShellRuntimeStatus> | null>(
    null,
  )
  const runtimeRefreshScopeKeyRef = useRef<string | null>(null)
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
    setDashboardLoading(false)
    setRuntimeStatus(emptyRuntimeStatus())
  }, [])

  const refreshDashboardSnapshot = useCallback(
    async (
      nextSnapshot: AppSnapshot | null,
      options: { surfaceErrors?: boolean } = {},
    ) => {
      const { surfaceErrors = false } = options
      if (!nextSnapshot) {
        setDashboard(null)
        setDashboardLoading(false)
        return
      }

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

  const refreshRuntimeStatus = useCallback(
    async (nextSnapshot: AppSnapshot | null = snapshot) => {
      const nextScopeKey = runtimeStatusScopeKey(nextSnapshot)
      if (
        !nextSnapshot?.config.initialized ||
        !nextSnapshot.archiveStatus.unlocked
      ) {
        const nextStatus = emptyRuntimeStatus()
        runtimeRefreshPromiseRef.current = null
        runtimeRefreshScopeKeyRef.current = nextScopeKey
        setRuntimeStatus(nextStatus)
        return nextStatus
      }

      if (
        runtimeRefreshPromiseRef.current &&
        runtimeRefreshScopeKeyRef.current === nextScopeKey
      ) {
        return runtimeRefreshPromiseRef.current
      }

      setRuntimeStatus((current) => ({
        ...current,
        loading: true,
        error: null,
      }))

      const nextRequest = Promise.all([
        backend.loadAiQueueStatus(),
        backend.loadIntelligenceRuntime(),
      ])
        .then(([nextAiQueue, nextRuntime]) => {
          const nextStatus: ShellRuntimeStatus = {
            aiQueue: nextAiQueue,
            intelligence: nextRuntime,
            loading: false,
            error: null,
          }
          setRuntimeStatus(nextStatus)
          return nextStatus
        })
        .catch((nextError) => {
          const message =
            nextError instanceof Error
              ? nextError.message
              : t('common.notAvailable')
          const nextStatus: ShellRuntimeStatus = {
            aiQueue: null,
            intelligence: null,
            loading: false,
            error: message,
          }
          setRuntimeStatus(nextStatus)
          return nextStatus
        })
        .finally(() => {
          if (runtimeRefreshPromiseRef.current === nextRequest) {
            runtimeRefreshPromiseRef.current = null
          }
        })

      runtimeRefreshScopeKeyRef.current = nextScopeKey
      runtimeRefreshPromiseRef.current = nextRequest
      return nextRequest
    },
    [snapshot, t],
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
    if (!snapshot?.config.initialized || !snapshot.archiveStatus.unlocked) {
      setRuntimeStatus(emptyRuntimeStatus())
      return
    }

    let cancelled = false
    let timeoutId: number | null = null

    const scheduleNext = (delayMs: number) => {
      if (cancelled || typeof window === 'undefined') return
      timeoutId = window.setTimeout(() => {
        void load()
      }, delayMs)
    }

    const load = async () => {
      const next = await refreshRuntimeStatus(snapshot)
      if (cancelled) return
      scheduleNext(countActiveRuntimeJobs(next) > 0 ? 3000 : 15000)
    }

    void load()

    return () => {
      cancelled = true
      if (timeoutId !== null && typeof window !== 'undefined') {
        window.clearTimeout(timeoutId)
      }
    }
  }, [
    refreshKey,
    refreshRuntimeStatus,
    snapshot,
    snapshot?.archiveStatus.unlocked,
    snapshot?.config.initialized,
  ])

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
