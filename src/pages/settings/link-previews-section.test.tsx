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

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import type { AppSnapshot, OgImageCleanupMode } from '@/lib/types'
import {
  LinkPreviewsSection,
  clampNumber,
  parseBlocklist,
} from './link-previews-section'

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

  test('blocklist input parses + saves blockedHosts (canonicalized + de-duped)', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    const textarea = screen.getByTestId('link-previews-blocklist-input')
    await userEvent.type(
      textarea,
      'Example.com{Enter}# inline comment{Enter}example.com{Enter}other.example.org',
    )
    await userEvent.click(screen.getByTestId('link-previews-blocklist-save'))
    expect(saveConfig).toHaveBeenCalledTimes(1)
    expect(saveConfig.mock.calls[0][0].ogImage.blockedHosts).toEqual([
      'example.com',
      'other.example.org',
    ])
  })

  test('blocklist reset reverts the draft to the persisted snapshot', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(
      withShell({
        ogImageFetchEnabled: true,
        blockedHosts: ['initial.example.test'],
      }),
    )
    const textarea = screen.getByTestId(
      'link-previews-blocklist-input',
    ) as HTMLTextAreaElement
    await userEvent.type(textarea, '{Enter}draft.example.test')
    expect(textarea.value).toContain('draft.example.test')
    await userEvent.click(screen.getByTestId('link-previews-blocklist-reset'))
    expect(textarea.value).toBe('initial.example.test')
  })

  test('switching cleanup mode persists the default per-mode arg', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    await userEvent.click(
      screen.getByTestId('link-previews-cleanup-mode-timeTtl'),
    )
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup).toEqual({
      mode: 'timeTtl',
      maxAgeDays: 60,
    })

    await userEvent.click(
      screen.getByTestId('link-previews-cleanup-mode-sizeCap'),
    )
    const sizeCap = saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup
    expect(sizeCap.mode).toBe('sizeCap')
    expect(sizeCap.maxBytes).toBe(200 * 1024 * 1024)

    await userEvent.click(screen.getByTestId('link-previews-cleanup-mode-lru'))
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup.mode).toBe('lru')
  })

  test('TimeTtl numeric input clamps the persisted value to [1, 3650] days', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(
      withShell({
        ogImageFetchEnabled: true,
        cleanup: { mode: 'timeTtl', maxAgeDays: 30 },
        saveConfig,
      }),
    )
    const input = screen.getByTestId('link-previews-max-age-days')
    fireEvent.change(input, { target: { value: '9999' } })
    const lastClamped = saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup
    expect(lastClamped.mode).toBe('timeTtl')
    expect(lastClamped.maxAgeDays).toBe(3650)
  })

  test('TimeTtl numeric input clamps below the floor too', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(
      withShell({
        ogImageFetchEnabled: true,
        cleanup: { mode: 'timeTtl', maxAgeDays: 30 },
        saveConfig,
      }),
    )
    fireEvent.change(screen.getByTestId('link-previews-max-age-days'), {
      target: { value: '0' },
    })
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup.maxAgeDays).toBe(1)
  })

  test('SizeCap numeric input persists bytes converted from MB', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(
      withShell({
        ogImageFetchEnabled: true,
        cleanup: { mode: 'sizeCap', maxBytes: 200 * 1024 * 1024 },
        saveConfig,
      }),
    )
    fireEvent.change(screen.getByTestId('link-previews-max-bytes-mb'), {
      target: { value: '512' },
    })
    const last = saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup
    expect(last.maxBytes).toBe(512 * 1024 * 1024)
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
  blockedHosts?: string[]
  cleanup?: OgImageCleanupMode
}) {
  const value: ShellDataContextValue = {
    buildInfo: null,
    appLockStatus: null,
    snapshot: makeSnapshot(
      overrides.ogImageFetchEnabled,
      overrides.blockedHosts ?? [],
      overrides.cleanup ?? { mode: 'off' as const },
    ),
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

function makeSnapshot(
  ogImageFetchEnabled: boolean,
  blockedHosts: string[],
  cleanup: OgImageCleanupMode,
): AppSnapshot {
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
        blockedHosts,
        cleanup,
      },
    },
    archiveStatus: {} as never,
    aiStatus: {} as never,
    browserProfiles: [],
    recentRuns: [],
  } as unknown as AppSnapshot
}

describe('parseBlocklist', () => {
  test('trims, lowercases, drops empty + commented lines, and de-duplicates', () => {
    expect(
      parseBlocklist(
        '\n  Example.com  \nexample.com\n#comment\nfoo.test\nFOO.test\n',
      ),
    ).toEqual(['example.com', 'foo.test'])
  })

  test('returns an empty array for empty / whitespace-only input', () => {
    expect(parseBlocklist('')).toEqual([])
    expect(parseBlocklist('   \n\t\n')).toEqual([])
  })
})

describe('clampNumber', () => {
  test('clamps below min and above max', () => {
    expect(clampNumber(-1, 0, 10, 5)).toBe(0)
    expect(clampNumber(99, 0, 10, 5)).toBe(10)
  })

  test('parses string inputs and falls back on NaN/empty', () => {
    expect(clampNumber('42', 0, 100, 5)).toBe(42)
    expect(clampNumber('not a number', 0, 100, 5)).toBe(5)
    expect(clampNumber('', 0, 100, 5)).toBe(5)
  })

  test('floors fractional input to an integer', () => {
    expect(clampNumber(3.9, 0, 10, 0)).toBe(3)
  })
})
