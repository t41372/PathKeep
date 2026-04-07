import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { backend } from '../lib/backend'
import { useI18nContext } from '../lib/i18n'
import type {
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
} from '../lib/types'
import { ShellDataContext } from './shell-data-context'

function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    if (typeof window === 'undefined') {
      resolve()
      return
    }
    window.requestAnimationFrame(() => resolve())
  })
}

export function ShellDataProvider({ children }: { children: ReactNode }) {
  const { setLanguagePreference, t } = useI18nContext()
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)

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
            : t('shell.loadingLatestArchiveState'),
        )
        throw nextError
      } finally {
        if (showSpinner) {
          setLoading(false)
        }
      }
    },
    [setLanguagePreference, t],
  )

  useEffect(() => {
    void refreshAppData()
  }, [refreshAppData])

  async function saveConfig(config: AppConfig) {
    setBusyAction(t('shell.savingArchiveChoices'))
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
      setBusyAction(null)
    }
  }

  async function initializeArchive(
    config: AppConfig,
    databaseKey?: string | null,
  ) {
    setBusyAction(t('shell.preparingArchive'))
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
      setBusyAction(null)
    }
  }

  async function runBackup() {
    setBusyAction(t('shell.runningManualBackup'))
    setNotice(null)
    setError(null)

    try {
      await waitForNextPaint()
      const report = await backend.runBackupNow(false)
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
      setBusyAction(null)
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
