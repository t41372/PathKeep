/**
 * This test file protects the front-end helper and contract logic in Update.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { UpdateInstallState } from './types'

/**
 * Exposes the legacy preview-aware backend facade consumed by older routes and tests.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
const backend = vi.hoisted(() => ({
  checkForAppUpdate: vi.fn(),
  downloadAndInstallAppUpdate: vi.fn(),
  relaunchAfterUpdate: vi.fn(() => Promise.resolve(true)),
}))

const { hasDesktopCommandTransportMock, hasTauriGuestApiMock } = vi.hoisted(
  () => ({
    hasDesktopCommandTransportMock: vi.fn(() => false),
    hasTauriGuestApiMock: vi.fn(() => false),
  }),
)

const subscribeToUpdaterProgress = vi.hoisted(() =>
  vi.fn((listener: unknown) => {
    void listener
    return Promise.resolve(() => {})
  }),
)

vi.mock('./backend-client', () => ({
  backend,
}))

vi.mock('./ipc/updater-progress', () => ({
  subscribeToUpdaterProgress,
}))

vi.mock('./runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
  hasTauriGuestApi: hasTauriGuestApiMock,
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
    hasDesktopCommandTransportMock.mockReturnValue(false)
    hasTauriGuestApiMock.mockReturnValue(false)
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

    const unknownVersion = await checkForAppUpdate()
    expect(unknownVersion.availability.currentVersion).toBeNull()
  })

  test('reports unsupported preview installs through the state callback', async () => {
    const states: UpdateInstallState[] = []

    const result = await downloadAndInstallAppUpdate(
      {
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: null,
        publishedAt: null,
        downloadUrl: 'https://example.com/latest.json',
      },
      (state) => {
        states.push(state)
      },
    )

    expect(backend.downloadAndInstallAppUpdate).not.toHaveBeenCalled()
    expect(result).toMatchObject({
      phase: 'unsupported',
      version: '0.2.0',
      downloadedBytes: null,
      contentLength: null,
    })
    expect(states).toEqual([result])
  })

  test('maps an available desktop update into release metadata', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
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

    backend.checkForAppUpdate.mockResolvedValueOnce({
      availability: {
        supported: true,
        checkedAt: '2026-04-10T00:00:00Z',
        available: false,
        currentVersion: null,
        version: null,
        notes: null,
        publishedAt: null,
        error: null,
        downloadUrl: null,
      },
      pendingUpdate: null,
    })
    const fallbackResult = await checkForAppUpdate('0.1.1')
    expect(fallbackResult.availability).toMatchObject({
      currentVersion: '0.1.1',
      downloadUrl: RELEASES_PAGE_URL,
    })

    backend.checkForAppUpdate.mockResolvedValueOnce({
      availability: {
        supported: true,
        checkedAt: '2026-04-10T00:00:00Z',
        available: false,
        currentVersion: null,
        version: null,
        notes: null,
        publishedAt: null,
        error: null,
        downloadUrl: null,
      },
      pendingUpdate: null,
    })
    const unknownCurrentVersion = await checkForAppUpdate()
    expect(unknownCurrentVersion.availability.currentVersion).toBeNull()
  })

  test('subscribes to updater progress and returns the desktop install result', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    hasTauriGuestApiMock.mockReturnValue(true)
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

  test('allows desktop-bridge installs without updater progress events', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    hasTauriGuestApiMock.mockReturnValue(false)
    backend.downloadAndInstallAppUpdate.mockResolvedValue({
      phase: 'installed',
      version: '0.2.0',
      downloadedBytes: 100,
      contentLength: 100,
      message: 'Installed through the desktop bridge.',
    })

    const result = await downloadAndInstallAppUpdate({
      currentVersion: '0.1.0',
      version: '0.2.0',
      notes: null,
      publishedAt: null,
      downloadUrl: 'https://example.com/latest.json',
    })

    expect(subscribeToUpdaterProgress).not.toHaveBeenCalled()
    expect(backend.downloadAndInstallAppUpdate).toHaveBeenCalledWith('0.2.0')
    expect(result.phase).toBe('installed')
  })

  test('does not duplicate the final updater state when progress already reported it', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    hasTauriGuestApiMock.mockReturnValue(true)
    const states: UpdateInstallState[] = []
    const installedState: UpdateInstallState = {
      phase: 'installed',
      version: '0.2.0',
      downloadedBytes: 100,
      contentLength: 100,
      message: 'PathKeep 0.2.0 is ready. Restart to finish switching versions.',
    }
    subscribeToUpdaterProgress.mockImplementation((listener: unknown) => {
      const emit = listener as (state: UpdateInstallState) => void
      emit(installedState)
      return Promise.resolve(vi.fn())
    })
    backend.downloadAndInstallAppUpdate.mockResolvedValue(installedState)

    const result = await downloadAndInstallAppUpdate(
      {
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: null,
        publishedAt: null,
        downloadUrl: 'https://example.com/latest.json',
      },
      (state) => states.push(state),
    )

    expect(result).toBe(installedState)
    expect(states).toEqual([installedState])
  })

  test('maps failed desktop installs into a recoverable install state', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(true)
    backend.downloadAndInstallAppUpdate.mockRejectedValueOnce(
      new Error('signature mismatch'),
    )
    const states: UpdateInstallState[] = []

    const result = await downloadAndInstallAppUpdate(
      {
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: null,
        publishedAt: null,
        downloadUrl: 'https://example.com/latest.json',
      },
      (state) => {
        states.push(state)
      },
    )

    expect(result).toMatchObject({
      phase: 'error',
      version: '0.2.0',
      message: 'signature mismatch',
    })
    expect(states).toEqual([result])

    backend.downloadAndInstallAppUpdate.mockRejectedValueOnce('offline')
    await expect(
      downloadAndInstallAppUpdate({
        currentVersion: '0.1.0',
        version: '0.3.0',
        notes: null,
        publishedAt: null,
        downloadUrl: 'https://example.com/latest.json',
      }),
    ).resolves.toMatchObject({
      phase: 'error',
      version: '0.3.0',
      message: 'offline',
    })
  })

  test('keeps relaunch behind the desktop command boundary', async () => {
    expect(initialUpdateInstallState()).toEqual({
      phase: 'idle',
      version: null,
      downloadedBytes: null,
      contentLength: null,
      message: null,
    })

    expect(await relaunchAfterUpdate()).toBe(false)
    expect(backend.relaunchAfterUpdate).not.toHaveBeenCalled()

    hasDesktopCommandTransportMock.mockReturnValue(true)
    expect(await relaunchAfterUpdate()).toBe(true)
    expect(backend.relaunchAfterUpdate).toHaveBeenCalledTimes(1)
  })
})
