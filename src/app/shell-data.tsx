import {
  useCallback,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { backend } from '../lib/backend'
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
    const finish = () => {
      if (settled) return
      settled = true
      resolve()
    }

    window.requestAnimationFrame(() => finish())
    window.setTimeout(finish, 16)
  })
}

function isAppLockError(error: unknown) {
  return (
    error instanceof Error &&
    /currently locked|unlock the app|unlock pathkeep/i.test(error.message)
  )
}

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
  const loadingLatestArchiveState = t('shell.loadingLatestArchiveState')

  function showBusyOverlay(next: BusyOverlayState) {
    setBusyAction(next.label)
    setBusyOverlay(next)
  }

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

        const [nextSnapshot, nextDashboard] = await Promise.all([
          backend.getAppSnapshot(),
          backend.loadDashboardSnapshot(),
        ])
        setLanguagePreference(nextSnapshot.config.preferredLanguage, {
          persist: false,
        })
        setSnapshot(nextSnapshot)
        setBuildInfo(nextBuildInfo)
        setDashboard(nextDashboard)
        setAppLockStatus(nextSnapshot.appLockStatus)
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
    void refreshAppData()
  }, [refreshAppData])

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

  async function runBackup() {
    const backupSteps = [
      t('shell.backupStepPrepare'),
      t('shell.backupStepArchive'),
      t('shell.backupStepRefresh'),
    ]
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
