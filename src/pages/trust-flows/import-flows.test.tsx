/**
 * @file import-flows.test.tsx
 * @description Protects ImportPage trust-flow regressions after splitting the original mega-suite.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Keep import preview, execute, revert, and doctor-review behavior stable.
 * - Preserve workflow disclosure, progress overlay, and selected-batch review affordances.
 * - Reuse the canonical trust-flow harness while keeping ImportPage's mocked Tauri boundaries local.
 *
 * ## Non-Responsibilities
 * - Does not redefine shared trust-flow helpers or route harness ownership.
 * - Does not cover schedule, security, settings, or audit trust surfaces.
 *
 * ## Dependencies
 * - Depends on the shared trust-flow harness for seeded snapshots and route rendering.
 * - Owns the mocked Tauri core and import-progress boundary that ImportPage consumes directly.
 *
 * ## Performance Notes
 * - Reuses the seeded archive harness so splitting this suite does not multiply heavy route setup cost.
 */

import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator, createTranslator } from '../../lib/i18n'
import { subscribeToImportProgress as subscribeToImportProgressModule } from '../../lib/ipc/import-progress'
import { macosFullDiskAccessSettingsUrl } from '../../lib/platform-guidance'
import type {
  ImportBatchDetail,
  ImportBatchOverview,
  ImportProgressEvent,
} from '../../lib/types'
import { backend } from '../../lib/backend-client'
import { ImportPage } from '../import'
import {
  expectHtmlElement,
  renderTrustPage,
  resetTrustFlowHarness,
  seedInitializedSnapshot,
} from './test-helpers'

const { invoke, isTauri, subscribeToImportProgress } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
}))

describe('trust flows/import flows', () => {
  beforeEach(() => {
    resetTrustFlowHarness({ invoke, isTauri, subscribeToImportProgress })
  })

  test('covers import preview, execute, revert, and doctor review in a translated locale', async () => {
    const user = userEvent.setup()
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true)
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('zh-CN', 'import')
    const zhCnT = createTranslator('zh-CN')

    renderTrustPage(<ImportPage />, {
      language: 'zh-CN',
      route: '/import',
      snapshot,
    })

    await user.type(
      screen.getByPlaceholderText('/path/to/takeout.zip'),
      '/tmp/takeout',
    )
    await user.click(
      screen.getByRole('button', { name: importT('scanSource') }),
    )

    expect(await screen.findByText(importT('previewTitle'))).toBeVisible()
    expect(await screen.findByText('PathKeep trust UX notes')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: importT('confirmImport') }),
    )
    expect(await screen.findByText(importT('completeTitle'))).toBeVisible()
    expect(
      (await screen.findAllByText(importT('imported'))).length,
    ).toBeGreaterThan(0)

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: importT('revertBatch') }),
      ).toBeEnabled(),
    )
    await user.click(
      screen.getByRole('button', { name: importT('revertBatch') }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: importT('restoreBatch') }),
      ).toBeEnabled(),
    )

    await user.click(
      screen.getByRole('button', { name: importT('showHistoryTools') }),
    )
    await user.click(
      screen.getByRole('button', { name: importT('runHealthCheckAction') }),
    )
    expect(
      await screen.findByRole('heading', {
        name: new RegExp(zhCnT('common.statusNeedsAttention')),
      }),
    ).toBeVisible()

    confirmSpy.mockRestore()
  })

  test('keeps the workflow collapsed by default and prioritizes detected browser profiles over manual paths', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot,
    })

    expect(screen.getByText(importT('workflowCollapsedHint'))).toBeVisible()
    expect(
      screen.queryByText(importT('workflowPreviewTitle')),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: importT('showWorkflow') }),
    )
    expect(
      await screen.findByText(importT('workflowPreviewTitle')),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))

    expect(
      await screen.findByText(importT('detectedBrowserProfiles')),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: /Google Chrome · Primary/i }),
    ).toBeVisible()
    expect(
      screen.queryByPlaceholderText('/path/to/History'),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: importT('showManualPath') }),
    )
    expect(screen.getByPlaceholderText('/path/to/History')).toBeVisible()
  })

  test('routes Browser Direct scan and import through browser-history commands', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')
    const chromeProfile = snapshot.browserProfiles.find(
      (profile) => profile.profileId === 'chrome:Default',
    )
    expect(chromeProfile).toBeDefined()

    const previewInspection = {
      dryRun: true,
      sourcePath: chromeProfile!.historyPath!,
      recognizedFiles: [
        {
          path: chromeProfile!.historyPath!,
          kind: 'chromium-history-db',
          status: 'previewed',
          records: 1,
          classification: 'will-import',
          reasonCode: 'chromium-history-sqlite',
          reasonDetail: null,
          detectedLocale: null,
        },
      ],
      quarantinedFiles: [],
      previewEntries: [
        {
          sourcePath: chromeProfile!.historyPath!,
          url: 'https://example.com/browser-direct',
          title: 'Browser direct entry',
          visitedAt: '2026-04-20T10:00:00.000Z',
          sourceVisitId: 1,
          status: 'candidate',
        },
      ],
      candidateItems: 1,
      importedItems: 0,
      duplicateItems: 0,
      notes: ['Browser Direct preview ready.'],
      detectedLocale: null,
      previewRangeStart: '2026-04-20T10:00:00.000Z',
      previewRangeEnd: '2026-04-20T10:00:00.000Z',
      importBatch: null,
    } satisfies Awaited<ReturnType<typeof backend.inspectBrowserHistory>>

    const importedBatch: ImportBatchOverview = {
      id: 42,
      sourceKind: 'browser-history',
      sourcePath: chromeProfile!.historyPath!,
      profileId: chromeProfile!.profileId,
      createdAt: '2026-04-20T10:00:00.000Z',
      importedAt: '2026-04-20T10:01:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 1,
      importedItems: 1,
      duplicateItems: 0,
      visibleItems: 1,
      auditPath: '/tmp/browser-direct-import-audit.json',
      gitCommit: null,
    }
    const importedInspection = {
      ...previewInspection,
      dryRun: false,
      importedItems: 1,
      importBatch: importedBatch,
    } satisfies Awaited<ReturnType<typeof backend.importBrowserHistory>>
    const importedBatchDetail: ImportBatchDetail = {
      batch: importedBatch,
      previewEntries: previewInspection.previewEntries,
      recognizedFiles: previewInspection.recognizedFiles,
      quarantinedFiles: [],
      notes: ['Imported Browser Direct history.'],
      detectedLocale: null,
      previewRangeStart: previewInspection.previewRangeStart,
      previewRangeEnd: previewInspection.previewRangeEnd,
    }

    const inspectTakeoutSpy = vi.spyOn(backend, 'inspectTakeout')
    const importTakeoutSpy = vi.spyOn(backend, 'importTakeout')
    const inspectBrowserSpy = vi
      .spyOn(backend, 'inspectBrowserHistory')
      .mockResolvedValue(previewInspection)
    const importBrowserSpy = vi
      .spyOn(backend, 'importBrowserHistory')
      .mockResolvedValue(importedInspection)
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      importedBatchDetail,
    )

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot,
    })

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))
    await user.click(
      await screen.findByRole('button', { name: /Google Chrome · Primary/i }),
    )
    await user.click(
      screen.getByRole('button', { name: importT('scanSource') }),
    )

    await waitFor(() =>
      expect(inspectBrowserSpy).toHaveBeenCalledWith({
        sourcePath: chromeProfile!.historyPath,
        dryRun: true,
        browserFamily: chromeProfile!.browserFamily,
        profileId: chromeProfile!.profileId,
        browserName: chromeProfile!.browserName,
        profileName: chromeProfile!.profileName,
      }),
    )
    expect(inspectTakeoutSpy).not.toHaveBeenCalled()

    await user.click(
      screen.getByRole('button', { name: importT('confirmImport') }),
    )
    await waitFor(() =>
      expect(importBrowserSpy).toHaveBeenCalledWith({
        sourcePath: chromeProfile!.historyPath,
        dryRun: false,
        browserFamily: chromeProfile!.browserFamily,
        profileId: chromeProfile!.profileId,
        browserName: chromeProfile!.browserName,
        profileName: chromeProfile!.profileName,
      }),
    )
    expect(importTakeoutSpy).not.toHaveBeenCalled()
    expect(await screen.findByText(importT('completeTitle'))).toBeVisible()
  })

  test('shows ChatGPT Atlas as a validated Browser Direct profile', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')
    const chromeProfile = snapshot.browserProfiles.find(
      (profile) => profile.profileId === 'chrome:Default',
    )
    expect(chromeProfile).toBeDefined()
    const atlasProfile = {
      ...chromeProfile!,
      profileId: 'atlas:user-test',
      profileName: 'Atlas Work',
      browserName: 'ChatGPT Atlas',
      profilePath:
        '/Users/test/Library/Application Support/com.openai.atlas/browser-data/host/user-test',
      historyPath:
        '/Users/test/Library/Application Support/com.openai.atlas/browser-data/host/user-test/History',
      faviconsPath:
        '/Users/test/Library/Application Support/com.openai.atlas/browser-data/host/user-test/Favicons',
    }
    const snapshotWithAtlas = {
      ...snapshot,
      browserProfiles: [...snapshot.browserProfiles, atlasProfile],
    }
    const inspectBrowserSpy = vi
      .spyOn(backend, 'inspectBrowserHistory')
      .mockResolvedValue({
        dryRun: true,
        sourcePath: atlasProfile.historyPath,
        recognizedFiles: [
          {
            path: atlasProfile.historyPath,
            kind: 'chromium-history-db',
            status: 'previewed',
            records: 1,
            classification: 'will-import',
            reasonCode: 'chromium-history-sqlite',
            reasonDetail: null,
            detectedLocale: null,
          },
        ],
        quarantinedFiles: [],
        previewEntries: [],
        candidateItems: 1,
        importedItems: 0,
        duplicateItems: 0,
        notes: ['Atlas preview ready.'],
        detectedLocale: null,
        previewRangeStart: '2026-04-24T10:00:00.000Z',
        previewRangeEnd: '2026-04-24T10:00:00.000Z',
        importBatch: null,
      })

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot: snapshotWithAtlas,
    })

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))
    await user.click(
      await screen.findByRole('button', {
        name: /ChatGPT Atlas · Atlas Work/i,
      }),
    )
    await user.click(
      screen.getByRole('button', { name: importT('scanSource') }),
    )

    await waitFor(() =>
      expect(inspectBrowserSpy).toHaveBeenCalledWith({
        sourcePath: atlasProfile.historyPath,
        dryRun: true,
        browserFamily: atlasProfile.browserFamily,
        profileId: atlasProfile.profileId,
        browserName: atlasProfile.browserName,
        profileName: atlasProfile.profileName,
      }),
    )
  })

  test('opens macOS Full Disk Access settings from the Safari access warning', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')
    const snapshotWithBlockedSafari = {
      ...snapshot,
      browserProfiles: snapshot.browserProfiles.map((profile) =>
        profile.profileId === 'safari:default'
          ? { ...profile, historyExists: false }
          : profile,
      ),
    }
    const openSettingsSpy = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue(macosFullDiskAccessSettingsUrl)

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot: snapshotWithBlockedSafari,
    })

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))

    expect(
      await screen.findByText(importT('safariFullDiskAccessHint')),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: importT('openFullDiskAccessSettings'),
      }),
    )

    await waitFor(() =>
      expect(openSettingsSpy).toHaveBeenCalledWith(
        macosFullDiskAccessSettingsUrl,
      ),
    )
    expect(
      screen.queryByRole('button', { name: /Safari · Safari/i }),
    ).not.toBeInTheDocument()
  })

  test('offers the settings action when Safari scan reports Full Disk Access is missing', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')
    const openSettingsSpy = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue(macosFullDiskAccessSettingsUrl)
    vi.spyOn(backend, 'inspectBrowserHistory').mockRejectedValue(
      new Error(
        'Safari History.db is not readable yet. Grant Full Disk Access to PathKeep, then retry Browser Direct import.',
      ),
    )

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot,
    })

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))
    await user.click(
      await screen.findByRole('button', { name: /Safari · Safari/i }),
    )
    await user.click(
      screen.getByRole('button', { name: importT('scanSource') }),
    )

    expect(await screen.findByText(importT('actionErrorTitle'))).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: importT('openFullDiskAccessSettings'),
      }),
    )

    await waitFor(() =>
      expect(openSettingsSpy).toHaveBeenCalledWith(
        macosFullDiskAccessSettingsUrl,
      ),
    )
  })

  test('paints scan and import overlays before long-running import work settles', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')
    const originalRequestAnimationFrame = window.requestAnimationFrame
    let resolveInspection:
      | ((value: Awaited<ReturnType<typeof backend.inspectTakeout>>) => void)
      | null = null
    let resolveImport:
      | ((value: Awaited<ReturnType<typeof backend.importTakeout>>) => void)
      | null = null
    let importListener: ((event: ImportProgressEvent) => void) | null = null

    const previewInspection = {
      dryRun: true,
      sourcePath: '/tmp/takeout',
      recognizedFiles: [
        {
          path: '/tmp/takeout/BrowserHistory.json',
          kind: 'browser-json',
          status: 'previewed',
          records: 1,
          classification: 'will-import',
          reasonCode: 'chrome-history-json',
          reasonDetail: null,
          detectedLocale: 'en',
        },
      ],
      quarantinedFiles: [],
      previewEntries: [
        {
          sourcePath: '/tmp/takeout/BrowserHistory.json',
          url: 'https://example.com/trust',
          title: 'Trust flow entry',
          visitedAt: '2026-04-20T10:00:00.000Z',
          sourceVisitId: 1,
          status: 'candidate',
        },
      ],
      candidateItems: 1,
      importedItems: 0,
      duplicateItems: 0,
      notes: ['Preview ready.'],
      detectedLocale: 'en',
      previewRangeStart: '2026-04-20T10:00:00.000Z',
      previewRangeEnd: '2026-04-20T10:00:00.000Z',
      importBatch: null,
    } satisfies Awaited<ReturnType<typeof backend.inspectTakeout>>

    const importedBatch: ImportBatchOverview = {
      id: 7,
      sourceKind: 'takeout',
      sourcePath: '/tmp/takeout',
      profileId: 'takeout::browser-history',
      createdAt: '2026-04-20T10:00:00.000Z',
      importedAt: '2026-04-20T10:01:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 1,
      importedItems: 1,
      duplicateItems: 0,
      visibleItems: 1,
      auditPath: '/tmp/import-audit.json',
      gitCommit: null,
    }

    const importedInspection = {
      ...previewInspection,
      dryRun: false,
      importedItems: 1,
      importBatch: importedBatch,
    } satisfies Awaited<ReturnType<typeof backend.importTakeout>>

    const importedBatchDetail: ImportBatchDetail = {
      batch: importedBatch,
      previewEntries: previewInspection.previewEntries,
      recognizedFiles: previewInspection.recognizedFiles,
      quarantinedFiles: [],
      notes: ['Imported successfully.'],
      detectedLocale: 'en',
      previewRangeStart: '2026-04-20T10:00:00.000Z',
      previewRangeEnd: '2026-04-20T10:00:00.000Z',
    }

    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      writable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0)
        return 1
      },
    })

    vi.spyOn(backend, 'inspectTakeout').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveInspection = resolve
        }),
    )
    vi.spyOn(backend, 'importTakeout').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveImport = resolve
        }),
    )
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue(
      importedBatchDetail,
    )
    vi.mocked(subscribeToImportProgressModule).mockImplementation(
      (nextListener) => {
        importListener = nextListener
        return Promise.resolve(vi.fn())
      },
    )

    try {
      renderTrustPage(<ImportPage />, {
        language: 'en',
        route: '/import',
        snapshot,
      })

      await user.type(
        screen.getByPlaceholderText('/path/to/takeout.zip'),
        '/tmp/takeout',
      )
      await user.click(
        screen.getByRole('button', { name: importT('scanSource') }),
      )

      await waitFor(() =>
        expect(screen.getByText(importT('scanningTitle'))).toBeVisible(),
      )
      await waitFor(() => expect(backend.inspectTakeout).toHaveBeenCalled())

      await act(async () => {
        resolveInspection?.(previewInspection)
        await Promise.resolve()
      })

      expect(await screen.findByText(importT('previewTitle'))).toBeVisible()

      await user.click(
        screen.getByRole('button', { name: importT('confirmImport') }),
      )

      await waitFor(() =>
        expect(screen.getByText(importT('importingTitle'))).toBeVisible(),
      )
      await waitFor(() => expect(backend.importTakeout).toHaveBeenCalled())

      act(() => {
        importListener?.({
          phase: 'import-file',
          label: 'Importing browser history',
          detail: 'Processing /tmp/takeout/BrowserHistory.json (1/1)',
          current: 1,
          total: 1,
          progressPercent: null,
          logLines: [
            'Importing browser-history from /tmp/takeout/BrowserHistory.json.',
          ],
          sourcePath: '/tmp/takeout/BrowserHistory.json',
        })
      })

      await waitFor(() =>
        expect(
          screen.getAllByText(
            'Writing file 1 of 1: /tmp/takeout/BrowserHistory.json',
          ).length,
        ).toBeGreaterThan(0),
      )
      expect(screen.getByText('File 1 of 1')).toBeVisible()

      await act(async () => {
        resolveImport?.(importedInspection)
        await Promise.resolve()
      })

      expect(await screen.findByText(importT('completeTitle'))).toBeVisible()
    } finally {
      Object.defineProperty(window, 'requestAnimationFrame', {
        configurable: true,
        writable: true,
        value: originalRequestAnimationFrame,
      })
    }
  })

  test('clears stale batch detail when the newly selected preview fails', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const batches: ImportBatchOverview[] = [
      {
        id: 1,
        sourceKind: 'takeout',
        sourcePath: '/tmp/takeout-a',
        profileId: 'takeout::browser-history',
        createdAt: '2026-04-10T10:00:00.000Z',
        importedAt: '2026-04-10T10:05:00.000Z',
        revertedAt: null,
        status: 'imported',
        candidateItems: 1,
        importedItems: 1,
        duplicateItems: 0,
        visibleItems: 1,
        auditPath: '/tmp/import-audit-a.json',
        gitCommit: null,
      },
      {
        id: 2,
        sourceKind: 'takeout',
        sourcePath: '/tmp/takeout-b',
        profileId: 'takeout::browser-history',
        createdAt: '2026-04-10T11:00:00.000Z',
        importedAt: '2026-04-10T11:05:00.000Z',
        revertedAt: null,
        status: 'imported',
        candidateItems: 1,
        importedItems: 1,
        duplicateItems: 0,
        visibleItems: 1,
        auditPath: '/tmp/import-audit-b.json',
        gitCommit: null,
      },
    ]
    snapshot.recentImportBatches = batches

    const previewByBatch: Record<number, ImportBatchDetail> = {
      1: {
        batch: batches[0],
        previewEntries: [
          {
            sourcePath: '/tmp/takeout-a',
            url: 'https://example.com/first',
            title: 'First batch entry',
            visitedAt: '2026-04-10T10:04:00.000Z',
            sourceVisitId: 1,
            status: 'imported',
          },
        ],
        recognizedFiles: [],
        quarantinedFiles: [],
        notes: [],
        detectedLocale: 'en',
        previewRangeStart: '2026-04-10T10:04:00.000Z',
        previewRangeEnd: '2026-04-10T10:04:00.000Z',
      },
    }
    vi.spyOn(backend, 'previewImportBatch').mockImplementation((batchId) => {
      if (batchId in previewByBatch) {
        return Promise.resolve(previewByBatch[batchId])
      }
      return Promise.reject(new Error('Batch detail unavailable'))
    })

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import?batch=1',
      snapshot,
    })

    expect(await screen.findByText('https://example.com/first')).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: /Show history/i,
      }),
    )
    await user.click(
      screen.getByRole('button', {
        name: /Batch #2/,
      }),
    )

    expect(await screen.findByText('Batch detail unavailable')).toBeVisible()
    expect(
      screen.queryByText('https://example.com/first'),
    ).not.toBeInTheDocument()
  })

  test('keeps selected batch audit-path actions wired after the import review split', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const commonT = createNamespaceTranslator('en', 'common')
    const auditT = createNamespaceTranslator('en', 'audit')
    const batch: ImportBatchOverview = {
      id: 1,
      sourceKind: 'takeout',
      sourcePath: '/tmp/takeout-a',
      profileId: 'takeout::browser-history',
      createdAt: '2026-04-10T10:00:00.000Z',
      importedAt: '2026-04-10T10:05:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 1,
      importedItems: 1,
      duplicateItems: 0,
      visibleItems: 1,
      auditPath: '/tmp/import-audit-a.json',
      gitCommit: null,
    }

    snapshot.recentImportBatches = [batch]
    vi.spyOn(backend, 'previewImportBatch').mockResolvedValue({
      batch,
      previewEntries: [
        {
          sourcePath: '/tmp/takeout-a',
          url: 'https://example.com/first',
          title: 'First batch entry',
          visitedAt: '2026-04-10T10:04:00.000Z',
          sourceVisitId: 1,
          status: 'imported',
        },
      ],
      recognizedFiles: [],
      quarantinedFiles: [],
      notes: [],
      detectedLocale: 'en',
      previewRangeStart: '2026-04-10T10:04:00.000Z',
      previewRangeEnd: '2026-04-10T10:04:00.000Z',
    })
    const openPathSpy = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/import-audit-a.json')

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import?batch=1',
      snapshot,
    })

    const selectedBatchPanel = expectHtmlElement(
      document.querySelector('.import-review-primary'),
    )
    expect(
      await within(selectedBatchPanel).findByText(auditT('manifestPath')),
    ).toBeVisible()

    await user.click(
      within(selectedBatchPanel).getByRole('button', {
        name: commonT('openAction'),
      }),
    )
    expect(openPathSpy).toHaveBeenLastCalledWith('/tmp/import-audit-a.json')

    await user.click(
      within(selectedBatchPanel).getByRole('button', {
        name: commonT('copyAction'),
      }),
    )
    expect(
      await within(selectedBatchPanel).findByText(commonT('copiedNotice')),
    ).toBeVisible()
  })
})
