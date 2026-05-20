/**
 * Smoke test for the paper-redesign Link previews (og:image) Settings
 * section.
 *
 * Verifies:
 * - Section renders with localized labels.
 * - Toggle reflects AppConfig.ogImage.fetchEnabled and writes through saveConfig.
 * - Cache stats render and update after Run-cleanup and Clear-all actions.
 * - Clear-all is guarded by window.confirm.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import type { AppSnapshot } from '@/lib/types'
import { LinkPreviewsSection } from './link-previews-section'

describe('LinkPreviewsSection', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('renders title + intro and reflects fetch_enabled from the snapshot', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: true }))
    expect(
      screen.getByTestId('settings-link-previews-section'),
    ).toBeInTheDocument()
    expect(screen.getByText('Link previews')).toBeInTheDocument()
    const toggle = screen.getByTestId('link-previews-fetch-toggle')
    expect(toggle.getAttribute('aria-checked')).toBe('true')
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-stats')).toHaveTextContent(
        'No previews cached yet.',
      ),
    )
  })

  test('toggling fetch enabled writes through saveConfig with the new value', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    await userEvent.click(screen.getByTestId('link-previews-fetch-toggle'))
    expect(saveConfig).toHaveBeenCalledTimes(1)
    const written = saveConfig.mock.calls[0][0]
    expect(written.ogImage.fetchEnabled).toBe(false)
  })

  test('Run cleanup calls backend.runOgImageCleanup and refreshes stats', async () => {
    const statsSpy = vi.spyOn(backend, 'getOgImageStorageStats')
    statsSpy.mockResolvedValueOnce({
      rowCount: 5,
      blobCount: 4,
      totalBytes: 1024,
      oldestFetchedAt: null,
    })
    statsSpy.mockResolvedValueOnce({
      rowCount: 3,
      blobCount: 2,
      totalBytes: 512,
      oldestFetchedAt: null,
    })
    const cleanupSpy = vi
      .spyOn(backend, 'runOgImageCleanup')
      .mockResolvedValue({
        deletedRows: 2,
        deletedBlobs: 2,
        reclaimedBytes: 512,
      })
    render(withShell({ ogImageFetchEnabled: true }))
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-stats')).toHaveTextContent('5'),
    )
    await userEvent.click(screen.getByTestId('link-previews-run-cleanup'))
    expect(cleanupSpy).toHaveBeenCalled()
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-summary')).toHaveTextContent(
        'Deleted 2 rows, 2 blobs',
      ),
    )
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-stats')).toHaveTextContent('3'),
    )
  })

  test('Clear all is guarded by window.confirm', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 10,
      blobCount: 8,
      totalBytes: 4096,
      oldestFetchedAt: null,
    })
    const clearSpy = vi.spyOn(backend, 'clearOgImageCache').mockResolvedValue({
      deletedRows: 10,
      deletedBlobs: 8,
      reclaimedBytes: 4096,
    })
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false)
    render(withShell({ ogImageFetchEnabled: true }))
    await userEvent.click(screen.getByTestId('link-previews-clear-all'))
    expect(confirmSpy).toHaveBeenCalled()
    expect(clearSpy).not.toHaveBeenCalled()

    confirmSpy.mockReturnValue(true)
    await userEvent.click(screen.getByTestId('link-previews-clear-all'))
    expect(clearSpy).toHaveBeenCalled()
  })
})

function withShell(overrides: {
  ogImageFetchEnabled: boolean
  saveConfig?: ShellDataContextValue['saveConfig']
}) {
  const value: ShellDataContextValue = {
    buildInfo: null,
    appLockStatus: null,
    snapshot: makeSnapshot(overrides.ogImageFetchEnabled),
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    notice: null,
    refreshKey: 0,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn(),
    saveConfig: overrides.saveConfig ?? vi.fn().mockResolvedValue(undefined),
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn(),
    clearNotice: vi.fn(),
  } as ShellDataContextValue

  return (
    <I18nProvider>
      <ShellDataContext.Provider value={value}>
        <LinkPreviewsSection />
      </ShellDataContext.Provider>
    </I18nProvider>
  )
}

function makeSnapshot(ogImageFetchEnabled: boolean): AppSnapshot {
  return {
    config: {
      initialized: true,
      archiveMode: 'Plaintext' as const,
      preferredLanguage: 'en' as const,
      dueAfterHours: 72,
      scheduleCheckIntervalHours: 6,
      checkpointDays: 90,
      captureFavicons: true,
      selectedProfileIds: [],
      gitEnabled: true,
      rememberDatabaseKeyInKeyring: false,
      appAutostart: false,
      explorerBackgroundPrefetchPages: 1,
      appLock: {
        enabled: false,
        biometricEnabled: false,
      } as never,
      remoteBackup: {} as never,
      enrichment: {} as never,
      deterministic: {} as never,
      ai: {} as never,
      ogImage: {
        fetchEnabled: ogImageFetchEnabled,
        blockedHosts: [],
        cleanup: { mode: 'off' as const },
      },
    },
    archiveStatus: {} as never,
    aiStatus: {} as never,
    browserProfiles: [],
    recentRuns: [],
  } as unknown as AppSnapshot
}
