/**
 * @file use-settings-updater-state.test.tsx
 * @description Hook-level updater coverage for Settings and Maintenance.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify the route-owned updater state machine without mounting Settings.
 * - Cover preview, unsupported, failed, relaunch, and release-link branches.
 *
 * ## Not responsible for
 * - Re-testing updater download transport internals.
 * - Re-testing the render-only updater section.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider plus spies on the backend and update helpers.
 *
 * ## Performance notes
 * - Keeps updater regression coverage at hook level so strict coverage does not require a full app-shell render for each branch.
 */

import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { I18nProvider } from '../../lib/i18n'
import type { AppBuildInfo, AppSnapshot } from '../../lib/types'
import * as updateLib from '../../lib/update'
import { useSettingsUpdaterState } from './use-settings-updater-state'

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

const buildInfo = {
  version: '0.1.0',
} as AppBuildInfo

const snapshot = {
  config: {},
} as AppSnapshot

function renderUpdaterState({
  currentSnapshot = snapshot,
}: {
  currentSnapshot?: AppSnapshot | null
} = {}) {
  return renderHook(
    () =>
      useSettingsUpdaterState({
        buildInfo,
        snapshot: currentSnapshot,
      }),
    { wrapper: Wrapper },
  )
}

describe('useSettingsUpdaterState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('skips update actions until route state has an app snapshot', async () => {
    const checkForAppUpdate = vi.spyOn(updateLib, 'checkForAppUpdate')
    const downloadAndInstallAppUpdate = vi.spyOn(
      updateLib,
      'downloadAndInstallAppUpdate',
    )
    const { result } = renderUpdaterState({ currentSnapshot: null })

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
      await result.current.updater.onDownloadAndInstallUpdate()
    })

    expect(checkForAppUpdate).not.toHaveBeenCalled()
    expect(downloadAndInstallAppUpdate).not.toHaveBeenCalled()
    expect(result.current.updater.updateInstallState.phase).toBe('idle')
  })

  test('maps unsupported, error, and up-to-date update checks into review state', async () => {
    const checkForAppUpdate = vi
      .spyOn(updateLib, 'checkForAppUpdate')
      .mockResolvedValueOnce({
        availability: {
          supported: false,
          checkedAt: '2026-04-25T00:00:00Z',
          available: false,
          currentVersion: '0.1.0',
          version: null,
          notes: null,
          publishedAt: null,
          error: null,
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      })
      .mockResolvedValueOnce({
        availability: {
          supported: false,
          checkedAt: '2026-04-25T00:00:00Z',
          available: false,
          currentVersion: '0.1.0',
          version: null,
          notes: null,
          publishedAt: null,
          error: 'Packaged build required',
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      })
      .mockResolvedValueOnce({
        availability: {
          supported: true,
          checkedAt: '2026-04-25T00:01:00Z',
          available: false,
          currentVersion: '0.1.0',
          version: null,
          notes: null,
          publishedAt: null,
          error: 'Release channel unreachable',
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      })
      .mockResolvedValueOnce({
        availability: {
          supported: true,
          checkedAt: '2026-04-25T00:02:00Z',
          available: true,
          currentVersion: '0.1.0',
          version: null,
          notes: null,
          publishedAt: null,
          error: null,
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      })
      .mockResolvedValueOnce({
        availability: {
          supported: true,
          checkedAt: '2026-04-25T00:03:00Z',
          available: false,
          currentVersion: '0.1.0',
          version: '0.1.0',
          notes: null,
          publishedAt: null,
          error: null,
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: null,
      })

    const { result } = renderUpdaterState()

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    expect(result.current.updater.updateInstallState).toMatchObject({
      phase: 'unsupported',
      message:
        'This surface only works in the desktop app. Browser preview can open the release page instead.',
    })

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    expect(result.current.updater.updateInstallState).toMatchObject({
      phase: 'unsupported',
      message: 'Packaged build required',
    })

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    expect(result.current.updater.updateInstallState).toMatchObject({
      phase: 'error',
      message: 'Release channel unreachable',
    })

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    expect(result.current.updater.updateInstallState).toMatchObject({
      phase: 'available',
      message:
        'PathKeep Not available is available. Review the notes below before installing.',
    })

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    expect(result.current.updater.updateInstallState.phase).toBe('uptodate')
    expect(checkForAppUpdate).toHaveBeenCalledWith('0.1.0')
  })

  test('downloads pending updates and exposes release/relaunch actions', async () => {
    const pendingUpdate = {
      currentVersion: '0.1.0',
      version: '0.2.0',
      notes: 'Updater notes',
      publishedAt: '2026-04-25T00:03:00Z',
      downloadUrl: 'https://example.test/pathkeep/releases/0.2.0',
    }
    vi.spyOn(updateLib, 'checkForAppUpdate').mockResolvedValue({
      availability: {
        supported: true,
        checkedAt: '2026-04-25T00:03:00Z',
        available: true,
        currentVersion: '0.1.0',
        version: '0.2.0',
        notes: 'Updater notes',
        publishedAt: '2026-04-25T00:03:00Z',
        error: null,
        downloadUrl: pendingUpdate.downloadUrl,
      },
      pendingUpdate,
    })
    const downloadAndInstallAppUpdate = vi
      .spyOn(updateLib, 'downloadAndInstallAppUpdate')
      .mockResolvedValue({
        phase: 'installed',
        version: '0.2.0',
        downloadedBytes: 128,
        contentLength: 128,
        message: 'Installed',
      })
    const relaunchAfterUpdate = vi
      .spyOn(updateLib, 'relaunchAfterUpdate')
      .mockResolvedValue(true)
    const openExternalUrl = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue(pendingUpdate.downloadUrl)

    const { result } = renderUpdaterState()

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    await act(async () => {
      await result.current.updater.onDownloadAndInstallUpdate()
      await result.current.updater.onRelaunchForUpdate()
      await result.current.updater.onOpenReleasePage()
    })

    expect(downloadAndInstallAppUpdate).toHaveBeenCalledWith(
      pendingUpdate,
      expect.any(Function),
    )
    expect(relaunchAfterUpdate).toHaveBeenCalledTimes(1)
    expect(openExternalUrl).toHaveBeenCalledWith(pendingUpdate.downloadUrl)
  })

  test('falls back to unavailable copy and the releases page on check failures', async () => {
    vi.spyOn(updateLib, 'checkForAppUpdate').mockRejectedValue('offline')
    const openExternalUrl = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue(updateLib.RELEASES_PAGE_URL)

    const { result } = renderUpdaterState()

    await act(async () => {
      await result.current.updater.onCheckForUpdates()
    })
    await act(async () => {
      await result.current.updater.onOpenReleasePage()
    })

    expect(result.current.updater.updateInstallState).toMatchObject({
      phase: 'error',
      message: 'Unavailable',
    })
    expect(openExternalUrl).toHaveBeenCalledWith(updateLib.RELEASES_PAGE_URL)
  })
})
