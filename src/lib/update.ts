import { isTauri } from '@tauri-apps/api/core'
import { backend } from './backend-client'
import { subscribeToUpdaterProgress } from './ipc/updater-progress'
import type {
  AppUpdateCheckResult,
  PendingAppUpdate,
  UpdateAvailability,
  UpdateInstallState,
} from './types'

export type { PendingAppUpdate }

export const RELEASES_PAGE_URL =
  'https://github.com/t41372/BrowserHistoryBackup/releases'

function nowIso() {
  return new Date().toISOString()
}

function previewAvailability(
  currentVersion?: string | null,
): AppUpdateCheckResult {
  return {
    availability: {
      supported: false,
      checkedAt: nowIso(),
      available: false,
      currentVersion: currentVersion ?? null,
      version: null,
      notes: null,
      publishedAt: null,
      error:
        'In-browser preview cannot check desktop update channels. Use a packaged desktop build instead.',
      downloadUrl: RELEASES_PAGE_URL,
    },
    pendingUpdate: null,
  }
}

export function initialUpdateInstallState(): UpdateInstallState {
  return {
    phase: 'idle',
    version: null,
    downloadedBytes: null,
    contentLength: null,
    message: null,
  }
}

export async function checkForAppUpdate(
  currentVersion?: string | null,
): Promise<AppUpdateCheckResult> {
  if (!isTauri()) {
    return previewAvailability(currentVersion)
  }

  const result = await backend.checkForAppUpdate()
  return {
    availability: {
      ...result.availability,
      currentVersion:
        result.availability.currentVersion ?? currentVersion ?? null,
      downloadUrl: result.availability.downloadUrl ?? RELEASES_PAGE_URL,
    } satisfies UpdateAvailability,
    pendingUpdate: result.pendingUpdate,
  }
}

export async function downloadAndInstallAppUpdate(
  pendingUpdate: PendingAppUpdate,
  onStateChange?: (state: UpdateInstallState) => void,
) {
  if (!isTauri()) {
    const unsupported = {
      phase: 'unsupported',
      version: pendingUpdate.version,
      message: 'In-browser preview cannot download or install desktop updates.',
      downloadedBytes: null,
      contentLength: null,
    } satisfies UpdateInstallState
    onStateChange?.(unsupported)
    return unsupported
  }

  let lastProgressPhase: UpdateInstallState['phase'] | null = null
  const unsubscribe = await subscribeToUpdaterProgress((state) => {
    lastProgressPhase = state.phase
    onStateChange?.(state)
  })

  try {
    const result = await backend.downloadAndInstallAppUpdate(
      pendingUpdate.version,
    )
    if (lastProgressPhase !== result.phase) {
      onStateChange?.(result)
    }
    return result
  } catch (error) {
    const failed = {
      phase: 'error',
      version: pendingUpdate.version,
      downloadedBytes: null,
      contentLength: null,
      message:
        error instanceof Error
          ? error.message
          : `PathKeep could not install ${pendingUpdate.version}.`,
    } satisfies UpdateInstallState
    onStateChange?.(failed)
    return failed
  } finally {
    unsubscribe()
  }
}

export async function relaunchAfterUpdate() {
  if (!isTauri()) {
    return false
  }

  return await backend.relaunchAfterUpdate()
}
