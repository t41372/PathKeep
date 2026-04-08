import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { backend } from '../lib/backend'
import { useI18nContext } from '../lib/i18n'
import type {
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
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

export function ShellDataProvider({ children }: { children: ReactNode }) {
  const { setLanguagePreference, t } = useI18nContext()
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [busyOverlay, setBusyOverlay] = useState<BusyOverlayState | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const loadingLatestArchiveState = t('shell.loadingLatestArchiveState')

  function showBusyOverlay(next: BusyOverlayState) {
    setBusyAction(next.label)
    setBusyOverlay(next)
  }

  function clearBusyOverlay() {
    setBusyAction(null)
    setBusyOverlay(null)
  }

  const refreshAppData = useCallback(
    async (showSpinner = true) => {
      if (showSpinner) {
        setLoading(true)
        await waitForNextPaint()
      }
      setError(null)

      try {
        const [nextSnapshot, nextBuildInfo, nextDashboard] = await Promise.all([
          backend.getAppSnapshot(),
          backend.getAppBuildInfo(),
          backend.loadDashboardSnapshot(),
        ])
        setLanguagePreference(nextSnapshot.config.preferredLanguage, {
          persist: false,
        })
        setSnapshot(nextSnapshot)
        setBuildInfo(nextBuildInfo)
        setDashboard(nextDashboard)
        setRefreshKey((value) => value + 1)
      } catch (nextError) {
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
    [loadingLatestArchiveState, setLanguagePreference],
  )

  useEffect(() => {
    void refreshAppData()
  }, [refreshAppData])

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

    showBusyOverlay({
      label: t('shell.runningManualBackup'),
      detail: t('shell.runningManualBackupDetail'),
      steps: backupSteps,
      activeStep: 0,
    })
    setNotice(null)
    setError(null)

    try {
      await waitForNextPaint()
      showBusyOverlay({
        label: t('shell.backupWritingArchive'),
        detail: t('shell.backupWritingArchiveDetail'),
        steps: backupSteps,
        activeStep: 1,
      })
      const report = await backend.runBackupNow(false)
      showBusyOverlay({
        label: t('shell.refreshingArchiveViews'),
        detail: t('shell.refreshingArchiveViewsDetail'),
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
      clearBusyOverlay()
    }
  }

  return (
    <ShellDataContext.Provider
      value={{
        buildInfo,
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
        clearNotice: () => setNotice(null),
      }}
    >
      {children}
    </ShellDataContext.Provider>
  )
}
