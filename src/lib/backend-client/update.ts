import type { AppUpdateCheckResult, UpdateInstallState } from '../types'
import { call } from './shared'

export const updateClient = {
  checkForAppUpdate: () => call<AppUpdateCheckResult>('check_for_app_update'),
  downloadAndInstallAppUpdate: (expectedVersion?: string | null) =>
    call<UpdateInstallState>('download_and_install_app_update', {
      request: { expectedVersion: expectedVersion ?? null },
    }),
  relaunchAfterUpdate: () => call<boolean>('relaunch_after_update'),
}
