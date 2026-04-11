import type {
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from '../types'
import { call } from './shared'

export const appClient = {
  getBuildInfo: () => call<AppBuildInfo>('app_build_info'),
  getLockStatus: () => call<AppLockStatus>('app_lock_status'),
  getSnapshot: () => call<AppSnapshot>('app_snapshot'),
  saveConfig: (config: AppConfig) =>
    call<AppSnapshot>('save_config', { config }),
  setSessionDatabaseKey: (databaseKey: string) =>
    call<void>('set_session_database_key', { databaseKey }),
  clearSessionDatabaseKey: () => call<void>('clear_session_database_key'),
  setAppLockPasscode: (request: SetAppLockPasscodeRequest) =>
    call<AppLockStatus>('set_app_lock_passcode', { request }),
  clearAppLockPasscode: () => call<AppLockStatus>('clear_app_lock_passcode'),
  lockAppSession: (reason?: string | null) =>
    call<AppLockStatus>('lock_app_session', { reason }),
  unlockAppSession: (request: UnlockAppSessionRequest) =>
    call<AppLockStatus>('unlock_app_session', { request }),
}
