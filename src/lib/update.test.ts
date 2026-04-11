import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { UpdateInstallState } from './types'

const { isTauri } = vi.hoisted(() => ({
  isTauri: vi.fn(() => false),
}))

const backend = vi.hoisted(() => ({
  checkForAppUpdate: vi.fn(),
  downloadAndInstallAppUpdate: vi.fn(),
  relaunchAfterUpdate: vi.fn(() => Promise.resolve(true)),
}))

const subscribeToUpdaterProgress = vi.hoisted(() =>
  vi.fn((listener: unknown) => {
    void listener
    return Promise.resolve(() => {})
  }),
)

vi.mock('@tauri-apps/api/core', () => ({
  isTauri,
}))

vi.mock('./backend-client', () => ({
  backend,
}))

vi.mock('./ipc/updater-progress', () => ({
  subscribeToUpdaterProgress,
}))

import {
  RELEASES_PAGE_URL,
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  initialUpdateInstallState,
  relaunchAfterUpdate,
} from './update'

describe('update helpers', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    backend.checkForAppUpdate.mockReset()
    backend.downloadAndInstallAppUpdate.mockReset()
    backend.relaunchAfterUpdate.mockReset()
    backend.relaunchAfterUpdate.mockResolvedValue(true)
    subscribeToUpdaterProgress.mockReset()
    subscribeToUpdaterProgress.mockResolvedValue(() => {})
  })

  test('returns a truthful browser-preview fallback', async () => {
    const result = await checkForAppUpdate('0.1.0')

    expect(result.pendingUpdate).toBeNull()
    expect(result.availability).toMatchObject({
      supported: false,
      available: false,
      currentVersion: '0.1.0',
      downloadUrl: RELEASES_PAGE_URL,
    })
  })

  test('maps an available desktop update into release metadata', async () => {
    isTauri.mockReturnValue(true)
    backend.checkForAppUpdate.mockResolvedValue({
      availability: {
        supported: true,
        checkedAt: '2026-04-10T00:00:00Z',
        available: true,
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: 'Bug fixes and updater wiring.',
        publishedAt: '2026-04-10T00:00:00Z',
        error: null,
        downloadUrl: 'https://example.com/latest.json',
      },
      pendingUpdate: {
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: 'Bug fixes and updater wiring.',
        publishedAt: '2026-04-10T00:00:00Z',
        downloadUrl: 'https://example.com/latest.json',
      },
    })

    const result = await checkForAppUpdate('0.1.0')

    expect(backend.checkForAppUpdate).toHaveBeenCalledTimes(1)
    expect(result.pendingUpdate).toMatchObject({
      version: '0.2.0',
      currentVersion: '0.1.0',
    })
    expect(result.availability).toMatchObject({
      supported: true,
      available: true,
      version: '0.2.0',
      publishedAt: '2026-04-10T00:00:00Z',
      downloadUrl: 'https://example.com/latest.json',
    })
  })

  test('subscribes to updater progress and returns the desktop install result', async () => {
    isTauri.mockReturnValue(true)
    const states: string[] = []
    const unsubscribe = vi.fn()
    subscribeToUpdaterProgress.mockImplementation((listener: unknown) => {
      const emit = listener as (state: UpdateInstallState) => void
      emit({
        phase: 'downloading',
        version: '0.2.0',
        downloadedBytes: 40,
        contentLength: 100,
        message: 'Downloading PathKeep 0.2.0...',
      })
      return Promise.resolve(unsubscribe)
    })
    backend.downloadAndInstallAppUpdate.mockResolvedValue({
      phase: 'installed',
      version: '0.2.0',
      downloadedBytes: 100,
      contentLength: 100,
      message: 'PathKeep 0.2.0 is ready. Restart to finish switching versions.',
    })

    const result = await downloadAndInstallAppUpdate(
      {
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: null,
        publishedAt: null,
        downloadUrl: 'https://example.com/latest.json',
      },
      (state) => {
        states.push(state.phase)
      },
    )

    expect(subscribeToUpdaterProgress).toHaveBeenCalledTimes(1)
    expect(backend.downloadAndInstallAppUpdate).toHaveBeenCalledWith('0.2.0')
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    expect(states).toEqual(['downloading', 'installed'])
    expect(result.phase).toBe('installed')
    expect(result.downloadedBytes).toBe(100)
    expect(result.contentLength).toBe(100)
  })

  test('keeps relaunch behind the desktop boundary', async () => {
    expect(initialUpdateInstallState()).toEqual({
      phase: 'idle',
      version: null,
      downloadedBytes: null,
      contentLength: null,
      message: null,
    })

    expect(await relaunchAfterUpdate()).toBe(false)
    expect(backend.relaunchAfterUpdate).not.toHaveBeenCalled()

    isTauri.mockReturnValue(true)
    expect(await relaunchAfterUpdate()).toBe(true)
    expect(backend.relaunchAfterUpdate).toHaveBeenCalledTimes(1)
  })
})
