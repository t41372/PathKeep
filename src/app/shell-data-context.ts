import { createContext, useContext } from 'react'
import type {
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  BackupReport,
  DashboardSnapshot,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from '../lib/types'

export interface BusyOverlayState {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  steps?: string[]
  activeStep?: number
}

export interface ShellDataContextValue {
  buildInfo: AppBuildInfo | null
  appLockStatus: AppLockStatus | null
  snapshot: AppSnapshot | null
  dashboard: DashboardSnapshot | null
  loading: boolean
  busyAction: string | null
  busyOverlay: BusyOverlayState | null
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
  setAppLockPasscode: (
    request: SetAppLockPasscodeRequest,
  ) => Promise<AppLockStatus>
  clearAppLockPasscode: () => Promise<AppLockStatus>
  lockAppSession: (reason?: string | null) => Promise<AppLockStatus>
  unlockAppSession: (request: UnlockAppSessionRequest) => Promise<AppLockStatus>
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
