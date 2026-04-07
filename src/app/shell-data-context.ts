import { createContext, useContext } from 'react'
import type {
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  BackupReport,
  DashboardSnapshot,
} from '../lib/types'

export interface ShellDataContextValue {
  buildInfo: AppBuildInfo | null
  snapshot: AppSnapshot | null
  dashboard: DashboardSnapshot | null
  loading: boolean
  busyAction: string | null
  error: string | null
  notice: string | null
  refreshKey: number
  refreshAppData: () => Promise<void>
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  initializeArchive: (
    config: AppConfig,
    databaseKey?: string | null,
  ) => Promise<AppSnapshot>
  runBackup: () => Promise<BackupReport>
  clearNotice: () => void
}

export const ShellDataContext = createContext<ShellDataContextValue | null>(
  null,
)

export function useShellData() {
  const value = useContext(ShellDataContext)

  if (!value) {
    throw new Error('useShellData must be used inside ShellDataProvider')
  }

  return value
}
