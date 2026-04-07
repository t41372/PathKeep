import { useEffect, useState, type ReactNode } from 'react'
import { backend } from '../lib/backend'
import type {
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
} from '../lib/types'
import { ShellDataContext } from './shell-data-context'

export function ShellDataProvider({ children }: { children: ReactNode }) {
  const [buildInfo, setBuildInfo] = useState<AppBuildInfo | null>(null)
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null)
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null)
  const [loading, setLoading] = useState(true)
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)

  async function refreshAppData(showSpinner = true) {
    if (showSpinner) {
      setLoading(true)
    }
    setError(null)

    try {
      const [nextSnapshot, nextBuildInfo, nextDashboard] = await Promise.all([
        backend.getAppSnapshot(),
        backend.getAppBuildInfo(),
        backend.loadDashboardSnapshot(),
      ])
      setSnapshot(nextSnapshot)
      setBuildInfo(nextBuildInfo)
      setDashboard(nextDashboard)
      setRefreshKey((value) => value + 1)
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not load the latest archive state.',
      )
      throw nextError
    } finally {
      if (showSpinner) {
        setLoading(false)
      }
    }
  }

  useEffect(() => {
    void refreshAppData()
  }, [])

  async function saveConfig(config: AppConfig) {
    setBusyAction('Saving archive choices')
    setNotice(null)
    setError(null)

    try {
      const nextSnapshot = await backend.saveConfig(config)
      const nextDashboard = await backend.loadDashboardSnapshot()
      setSnapshot(nextSnapshot)
      setDashboard(nextDashboard)
      setRefreshKey((value) => value + 1)
      return nextSnapshot
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not save the updated archive settings.',
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
    setBusyAction('Preparing the archive')
    setNotice(null)
    setError(null)

    try {
      const nextSnapshot = await backend.initializeArchive(config, databaseKey)
      const nextDashboard = await backend.loadDashboardSnapshot()
      setSnapshot(nextSnapshot)
      setDashboard(nextDashboard)
      setNotice(
        'Archive initialized. Review the first backup before automation.',
      )
      setRefreshKey((value) => value + 1)
      return nextSnapshot
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not initialize the archive.',
      )
      throw nextError
    } finally {
      setBusyAction(null)
    }
  }

  async function runBackup() {
    setBusyAction('Running a manual backup')
    setNotice(null)
    setError(null)

    try {
      const report = await backend.runBackupNow(false)
      await refreshAppData(false)
      setNotice(
        report.dueSkipped
          ? (report.reason ?? 'The archive is still within the due window.')
          : `Manual backup finished${report.run ? ` as run #${report.run.id}` : ''}.`,
      )
      return report
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : 'PathKeep could not complete the manual backup.',
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
