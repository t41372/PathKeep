import { isTauri } from '@tauri-apps/api/core'
import { relaunch } from '@tauri-apps/plugin-process'
import { check } from '@tauri-apps/plugin-updater'
import type { UpdateAvailability, UpdateInstallState } from './types'

type RawInstallableUpdate = NonNullable<Awaited<ReturnType<typeof check>>>

interface InstallableUpdate {
  version: string
  date?: string | null
  body?: string | null
  downloadAndInstall: RawInstallableUpdate['downloadAndInstall']
}

export interface PendingAppUpdate {
  currentVersion?: string | null
  version: string
  notes?: string | null
  publishedAt?: string | null
  update: InstallableUpdate
}

export interface AppUpdateCheckResult {
  availability: UpdateAvailability
  pendingUpdate: PendingAppUpdate | null
}

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

  const checkedAt = nowIso()
  try {
    const update = await check()
    if (!update) {
      return {
        availability: {
          supported: true,
          checkedAt,
          available: false,
          currentVersion: currentVersion ?? null,
          version: currentVersion ?? null,
          notes: null,
          publishedAt: null,
          error: null,
          downloadUrl: RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      }
    }

    const installableUpdate: InstallableUpdate = {
      version: update.version,
      date: update.date ?? null,
      body: update.body ?? null,
      downloadAndInstall: update.downloadAndInstall.bind(update),
    }

    return {
      availability: {
        supported: true,
        checkedAt,
        available: true,
        currentVersion: currentVersion ?? null,
        version: installableUpdate.version,
        notes: installableUpdate.body ?? null,
        publishedAt: installableUpdate.date ?? null,
        error: null,
        downloadUrl: RELEASES_PAGE_URL,
      },
      pendingUpdate: {
        currentVersion: currentVersion ?? null,
        version: installableUpdate.version,
        notes: installableUpdate.body ?? null,
        publishedAt: installableUpdate.date ?? null,
        update: installableUpdate,
      },
    }
  } catch (error) {
    return {
      availability: {
        supported: true,
        checkedAt,
        available: false,
        currentVersion: currentVersion ?? null,
        version: null,
        notes: null,
        publishedAt: null,
        error:
          error instanceof Error
            ? error.message
            : 'PathKeep could not check for updates right now.',
        downloadUrl: RELEASES_PAGE_URL,
      },
      pendingUpdate: null,
    }
  }
}

export async function downloadAndInstallAppUpdate(
  pendingUpdate: PendingAppUpdate,
  onStateChange?: (state: UpdateInstallState) => void,
) {
  if (!isTauri()) {
    const unsupported = {
      phase: 'unsupported',
      message: 'In-browser preview cannot download or install desktop updates.',
      downloadedBytes: null,
      contentLength: null,
    } satisfies UpdateInstallState
    onStateChange?.(unsupported)
    return unsupported
  }

  let downloadedBytes = 0
  let contentLength: number | null = null
  try {
    onStateChange?.({
      phase: 'downloading',
      downloadedBytes: 0,
      contentLength: null,
      message: `Downloading PathKeep ${pendingUpdate.version}…`,
    })

    await pendingUpdate.update.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          contentLength = event.data.contentLength ?? null
          onStateChange?.({
            phase: 'downloading',
            downloadedBytes,
            contentLength,
            message: `Downloading PathKeep ${pendingUpdate.version}…`,
          })
          break
        case 'Progress':
          downloadedBytes += event.data.chunkLength
          onStateChange?.({
            phase: 'downloading',
            downloadedBytes,
            contentLength,
            message: `Downloading PathKeep ${pendingUpdate.version}…`,
          })
          break
        case 'Finished':
          onStateChange?.({
            phase: 'installing',
            downloadedBytes,
            contentLength,
            message: `Installing PathKeep ${pendingUpdate.version}…`,
          })
          break
      }
    })

    const installed = {
      phase: 'installed',
      downloadedBytes,
      contentLength,
      message: `PathKeep ${pendingUpdate.version} is ready. Restart to finish switching versions.`,
    } satisfies UpdateInstallState
    onStateChange?.(installed)
    return installed
  } catch (error) {
    const failed = {
      phase: 'error',
      downloadedBytes,
      contentLength,
      message:
        error instanceof Error
          ? error.message
          : `PathKeep could not install ${pendingUpdate.version}.`,
    } satisfies UpdateInstallState
    onStateChange?.(failed)
    return failed
  }
}

export async function relaunchAfterUpdate() {
  if (!isTauri()) {
    return false
  }

  await relaunch()
  return true
}
