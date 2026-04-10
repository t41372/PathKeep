import { beforeEach, describe, expect, test, vi } from 'vitest'

const { isTauri } = vi.hoisted(() => ({
  isTauri: vi.fn(() => false),
}))
const { relaunch } = vi.hoisted(() => ({
  relaunch: vi.fn(() => Promise.resolve()),
}))
const { check } = vi.hoisted(() => ({
  check: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri,
}))

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch,
}))

vi.mock('@tauri-apps/plugin-updater', () => ({
  check,
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
    relaunch.mockClear()
    check.mockReset()
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
    check.mockResolvedValue({
      version: '0.2.0',
      date: '2026-04-10T00:00:00Z',
      body: 'Bug fixes and updater wiring.',
      downloadAndInstall: vi.fn(),
    })

    const result = await checkForAppUpdate('0.1.0')

    expect(result.pendingUpdate).toMatchObject({
      version: '0.2.0',
      currentVersion: '0.1.0',
    })
    expect(result.availability).toMatchObject({
      supported: true,
      available: true,
      version: '0.2.0',
      publishedAt: '2026-04-10T00:00:00Z',
    })
  })

  test('tracks download and install progress before returning restart-ready state', async () => {
    isTauri.mockReturnValue(true)
    const states: string[] = []
    const pendingUpdate = {
      currentVersion: '0.1.0',
      version: '0.2.0',
      notes: null,
      publishedAt: null,
      update: {
        version: '0.2.0',
        date: null,
        body: null,
        downloadAndInstall: vi.fn((handler) => {
          handler({
            event: 'Started',
            data: { contentLength: 100 },
          })
          handler({
            event: 'Progress',
            data: { chunkLength: 40 },
          })
          handler({
            event: 'Finished',
            data: {},
          })
          return Promise.resolve()
        }),
      },
    }

    const result = await downloadAndInstallAppUpdate(pendingUpdate, (state) => {
      states.push(state.phase)
    })

    expect(states).toEqual([
      'downloading',
      'downloading',
      'downloading',
      'installing',
      'installed',
    ])
    expect(result.phase).toBe('installed')
    expect(result.downloadedBytes).toBe(40)
    expect(result.contentLength).toBe(100)
  })

  test('keeps relaunch behind the desktop boundary', async () => {
    expect(initialUpdateInstallState()).toEqual({
      phase: 'idle',
      downloadedBytes: null,
      contentLength: null,
      message: null,
    })

    expect(await relaunchAfterUpdate()).toBe(false)
    expect(relaunch).not.toHaveBeenCalled()

    isTauri.mockReturnValue(true)
    expect(await relaunchAfterUpdate()).toBe(true)
    expect(relaunch).toHaveBeenCalledTimes(1)
  })
})
