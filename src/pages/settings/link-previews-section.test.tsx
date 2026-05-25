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
import type {
  AppSnapshot,
  OgImageCleanupMode,
  OgImageFetchMode,
} from '@/lib/types'
import { LinkPreviewsSection } from './link-previews-section'
import { clampNumber, parseBlocklist } from './link-previews-helpers'

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
    const textarea = screen.getByTestId<HTMLTextAreaElement>(
      'link-previews-blocklist-input',
    )
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

  test('switching cleanup mode to "off" persists { mode: "off" }', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    // Render with non-off initial mode so clicking "off" actually toggles.
    render(
      withShell({
        ogImageFetchEnabled: true,
        cleanup: { mode: 'timeTtl', maxAgeDays: 60 },
        saveConfig,
      }),
    )
    await userEvent.click(screen.getByTestId('link-previews-cleanup-mode-off'))
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.cleanup).toEqual({
      mode: 'off',
    })
  })

  test('TimeTtl numeric input clamps the persisted value to [1, 3650] days', () => {
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

  test('TimeTtl numeric input clamps below the floor too', () => {
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

  test('SizeCap numeric input persists bytes converted from MB', () => {
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

  // ── Fetch mode + budgets + Rebuild now coverage ──────────────────

  test('renders the fetch-mode segmented control with snapshot value selected', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: true, fetchMode: 'on_demand' }))
    expect(
      screen
        .getByTestId('link-previews-fetch-mode-on_demand')
        .getAttribute('aria-checked'),
    ).toBe('true')
    expect(
      screen
        .getByTestId('link-previews-fetch-mode-background')
        .getAttribute('aria-checked'),
    ).toBe('false')
  })

  test('selecting a different fetch mode persists it via saveConfig', async () => {
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
        fetchMode: 'background',
        saveConfig,
      }),
    )
    await userEvent.click(screen.getByTestId('link-previews-fetch-mode-off'))
    const written = saveConfig.mock.calls.at(-1)?.[0].ogImage
    expect(written.fetchMode).toBe('off')
    // Other fields preserved unchanged.
    expect(written.fetchEnabled).toBe(true)
    expect(written.dailyRefetchBudget).toBe(50)
    expect(written.newVisitPrefetchBudget).toBe(100)
  })

  test('clicking the currently-selected fetch mode is a no-op (skips saveConfig)', async () => {
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
        fetchMode: 'background',
        saveConfig,
      }),
    )
    await userEvent.click(
      screen.getByTestId('link-previews-fetch-mode-background'),
    )
    expect(saveConfig).not.toHaveBeenCalled()
  })

  test('fetch-mode buttons are disabled when fetchEnabled is off', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: false, fetchMode: 'background' }))
    expect(screen.getByTestId('link-previews-fetch-mode-off')).toBeDisabled()
    expect(
      screen.getByTestId('link-previews-fetch-mode-background'),
    ).toBeDisabled()
  })

  test('daily refetch budget renders the snapshot value', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(
      withShell({
        ogImageFetchEnabled: true,
        dailyRefetchBudget: 123,
      }),
    )
    const input = screen.getByTestId<HTMLInputElement>(
      'link-previews-daily-refetch-budget',
    )
    expect(input.value).toBe('123')
  })

  test('daily refetch budget persists in-range value via saveConfig', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    fireEvent.change(screen.getByTestId('link-previews-daily-refetch-budget'), {
      target: { value: '250' },
    })
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.dailyRefetchBudget).toBe(
      250,
    )
  })

  test('daily refetch budget clamps above the maximum (5000)', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    fireEvent.change(screen.getByTestId('link-previews-daily-refetch-budget'), {
      target: { value: '999999' },
    })
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.dailyRefetchBudget).toBe(
      5000,
    )
  })

  test('daily refetch budget clamps to 0 for negative values', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    fireEvent.change(screen.getByTestId('link-previews-daily-refetch-budget'), {
      target: { value: '-9' },
    })
    expect(saveConfig.mock.calls.at(-1)?.[0].ogImage.dailyRefetchBudget).toBe(0)
  })

  test('daily refetch budget skips saveConfig when value is unchanged', () => {
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
        dailyRefetchBudget: 50,
        saveConfig,
      }),
    )
    fireEvent.change(screen.getByTestId('link-previews-daily-refetch-budget'), {
      target: { value: '50' },
    })
    expect(saveConfig).not.toHaveBeenCalled()
  })

  test('daily refetch input is disabled when fetchEnabled is off', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: false }))
    expect(
      screen.getByTestId('link-previews-daily-refetch-budget'),
    ).toBeDisabled()
  })

  test('prefetch budget input persists in-range value', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    fireEvent.change(screen.getByTestId('link-previews-prefetch-budget'), {
      target: { value: '777' },
    })
    expect(
      saveConfig.mock.calls.at(-1)?.[0].ogImage.newVisitPrefetchBudget,
    ).toBe(777)
  })

  test('prefetch budget clamps above the maximum (5000)', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const saveConfig = vi.fn().mockResolvedValue(undefined)
    render(withShell({ ogImageFetchEnabled: true, saveConfig }))
    fireEvent.change(screen.getByTestId('link-previews-prefetch-budget'), {
      target: { value: '60000' },
    })
    expect(
      saveConfig.mock.calls.at(-1)?.[0].ogImage.newVisitPrefetchBudget,
    ).toBe(5000)
  })

  test('prefetch budget skips saveConfig when value is unchanged', () => {
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
        newVisitPrefetchBudget: 100,
        saveConfig,
      }),
    )
    fireEvent.change(screen.getByTestId('link-previews-prefetch-budget'), {
      target: { value: '100' },
    })
    expect(saveConfig).not.toHaveBeenCalled()
  })

  test('prefetch budget disabled when fetchEnabled is off', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: false }))
    expect(screen.getByTestId('link-previews-prefetch-budget')).toBeDisabled()
  })

  test('prefetch budget disabled when fetch mode is not Background', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: true, fetchMode: 'on_demand' }))
    expect(screen.getByTestId('link-previews-prefetch-budget')).toBeDisabled()
  })

  test('prefetch budget remains enabled when mode is Background + fetchEnabled', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: true, fetchMode: 'background' }))
    expect(
      screen.getByTestId('link-previews-prefetch-budget'),
    ).not.toBeDisabled()
  })

  test('Rebuild now calls backend.prefetchOgImages with the default budget', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    const prefetchSpy = vi
      .spyOn(backend, 'prefetchOgImages')
      .mockResolvedValue([12, 9])
    render(withShell({ ogImageFetchEnabled: true }))
    await userEvent.click(screen.getByTestId('link-previews-rebuild-now'))
    expect(prefetchSpy).toHaveBeenCalledTimes(1)
    expect(prefetchSpy).toHaveBeenCalledWith(500)
  })

  test('Rebuild now surfaces enqueued/succeeded counts in the summary', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    vi.spyOn(backend, 'prefetchOgImages').mockResolvedValue([42, 30])
    render(withShell({ ogImageFetchEnabled: true }))
    await userEvent.click(screen.getByTestId('link-previews-rebuild-now'))
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-summary')).toHaveTextContent(
        'Enqueued 42, succeeded 30.',
      ),
    )
  })

  test('Rebuild now refreshes stats after the worker call resolves', async () => {
    const statsSpy = vi.spyOn(backend, 'getOgImageStorageStats')
    statsSpy.mockResolvedValueOnce({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    statsSpy.mockResolvedValueOnce({
      rowCount: 42,
      blobCount: 30,
      totalBytes: 1024,
      oldestFetchedAt: null,
    })
    vi.spyOn(backend, 'prefetchOgImages').mockResolvedValue([42, 30])
    render(withShell({ ogImageFetchEnabled: true }))
    await userEvent.click(screen.getByTestId('link-previews-rebuild-now'))
    await waitFor(() =>
      expect(screen.getByTestId('link-previews-stats')).toHaveTextContent('42'),
    )
  })

  test('Rebuild now button is disabled when fetchEnabled is off', () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    render(withShell({ ogImageFetchEnabled: false }))
    expect(screen.getByTestId('link-previews-rebuild-now')).toBeDisabled()
  })

  test('Rebuild now clears the pending state even when the worker throws', async () => {
    vi.spyOn(backend, 'getOgImageStorageStats').mockResolvedValue({
      rowCount: 0,
      blobCount: 0,
      totalBytes: 0,
      oldestFetchedAt: null,
    })
    vi.spyOn(backend, 'prefetchOgImages').mockRejectedValue(
      new Error('worker offline'),
    )
    render(withShell({ ogImageFetchEnabled: true }))
    const button = screen.getByTestId('link-previews-rebuild-now')
    await userEvent.click(button).catch(() => undefined)
    // After the promise rejects, the button must re-enable so the user
    // can retry — otherwise a transient error permanently locks the
    // affordance until reload.
    await waitFor(() => expect(button).not.toBeDisabled())
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
  fetchMode?: OgImageFetchMode
  dailyRefetchBudget?: number
  newVisitPrefetchBudget?: number
}) {
  const value: ShellDataContextValue = {
    buildInfo: null,
    appLockStatus: null,
    snapshot: makeSnapshot({
      ogImageFetchEnabled: overrides.ogImageFetchEnabled,
      blockedHosts: overrides.blockedHosts ?? [],
      cleanup: overrides.cleanup ?? { mode: 'off' as const },
      fetchMode: overrides.fetchMode ?? 'background',
      dailyRefetchBudget: overrides.dailyRefetchBudget ?? 50,
      newVisitPrefetchBudget: overrides.newVisitPrefetchBudget ?? 100,
    }),
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

function makeSnapshot(options: {
  ogImageFetchEnabled: boolean
  blockedHosts: string[]
  cleanup: OgImageCleanupMode
  fetchMode: OgImageFetchMode
  dailyRefetchBudget: number
  newVisitPrefetchBudget: number
}): AppSnapshot {
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
        fetchEnabled: options.ogImageFetchEnabled,
        fetchMode: options.fetchMode,
        dailyRefetchBudget: options.dailyRefetchBudget,
        newVisitPrefetchBudget: options.newVisitPrefetchBudget,
        blockedHosts: options.blockedHosts,
        cleanup: options.cleanup,
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
