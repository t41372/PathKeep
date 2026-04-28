/**
 * @file index.test.tsx
 * @description Route-owner coverage for ImportPage's source-selection and command wiring state machine.
 * @module pages/import
 *
 * ## Responsibilities
 * - Verify ImportPage-owned profile validation, source reset, scan/import command routing, and permission recovery branches.
 * - Keep this test thin by mocking child panels that already have focused component coverage.
 *
 * ## Not responsible for
 * - Re-testing ImportWorkflowPanel layout or ImportReviewPanels rendering.
 * - Re-testing backend import parser behavior.
 *
 * ## Dependencies
 * - Mocks shell data, i18n, Tauri dialog, import progress, and child panels.
 *
 * ## Performance notes
 * - Fixtures are intentionally tiny; route-level async handlers are driven directly through captured child props.
 */

import { act, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { ShellImportTaskRequest } from '../../app/shell-data-context'
import type { ShellTask } from '../../app/shell-tasks'
import type * as coreIntelligenceApiModule from '../../lib/core-intelligence/api'
import type {
  BrowserProfile,
  ImportBatchDetail,
  ImportBatchOverview,
  TakeoutInspection,
} from '../../lib/types'
import { macosFullDiskAccessSettingsUrl } from '../../lib/platform-guidance'
import { ImportPage } from './index'
import type { ImportWorkflowPanelProps } from './workflow-panel'

const {
  clearActionErrorMock,
  importReviewStateMock,
  openMock,
  reviewPropsMock,
  shellDataMock,
  subscribeToImportProgressMock,
  workflowPropsMock,
} = vi.hoisted(() => ({
  clearActionErrorMock: vi.fn(),
  importReviewStateMock: vi.fn(),
  openMock: vi.fn(),
  reviewPropsMock: vi.fn(),
  shellDataMock: vi.fn(),
  subscribeToImportProgressMock: vi.fn(),
  workflowPropsMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: shellDataMock,
}))

vi.mock('../../lib/i18n', () => ({
  useI18n: () => ({
    language: 'en',
    t: (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}))

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: vi.fn(() => Promise.resolve()),
}))

vi.mock('../../lib/core-intelligence/api', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceApiModule>()
  return {
    ...actual,
    clearIntelligenceOverviewCache: vi.fn(),
  }
})

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress: subscribeToImportProgressMock,
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: openMock,
}))

vi.mock('./use-import-review-state', () => ({
  useImportReviewState: importReviewStateMock,
}))

vi.mock('./workflow-panel', () => ({
  ImportWorkflowPanel: (props: unknown) => {
    workflowPropsMock(props)
    return <div data-testid="workflow-panel" />
  },
}))

vi.mock('./review-panels', () => ({
  ImportReviewPanels: (props: unknown) => {
    reviewPropsMock(props)
    return <div data-testid="review-panels" />
  },
}))

describe('ImportPage route owner', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    clearActionErrorMock.mockClear()
    importReviewStateMock.mockReset()
    openMock.mockReset()
    reviewPropsMock.mockClear()
    shellDataMock.mockReset()
    subscribeToImportProgressMock.mockReset()
    workflowPropsMock.mockClear()
    shellDataMock.mockReturnValue(shellFixture())
    importReviewStateMock.mockReturnValue(importReviewStateFixture())
    subscribeToImportProgressMock.mockResolvedValue(vi.fn())
  })

  test('renders the archive setup gate before initialization', () => {
    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({ initialized: false }),
      }),
    )

    renderPage({
      snapshot: snapshotFixture({ initialized: false }),
    })

    expect(screen.getByText('import.archiveNotInitialized')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'import.goToSetup' }),
    ).toHaveAttribute('href', '/onboarding')
  })

  test('renders the archive setup gate while the shell snapshot is still absent', () => {
    renderPage({
      snapshot: null,
    })

    expect(screen.getByText('import.archiveNotInitialized')).toBeVisible()
    expect(latestWorkflowProps()).toBeUndefined()
  })

  test('passes the active shell import task into the workflow panel', () => {
    const task = shellTaskFixture({
      id: 'queued-import',
      state: 'queued',
      title: 'Queued import',
    })
    renderPage({
      shellOverrides: {
        archiveTasks: [shellTaskFixture({ kind: 'backup' }), task],
      },
    })

    expect(latestWorkflowProps().importTask).toBe(task)
  })

  test('filters validated browser profiles and handles method/profile source defaults', async () => {
    renderPage({
      snapshot: snapshotFixture({
        browserProfiles: [
          browserProfileFixture({
            browserFamily: 'safari',
            browserName: 'Safari',
            profileId: 'safari:Personal',
            profileName: 'Personal',
          }),
          browserProfileFixture({
            browserName: 'Google Chrome',
            profileId: 'chrome:Default',
            profileName: 'Default',
          }),
          browserProfileFixture({
            browserName: 'Unknown Chromium',
            profileId: 'atlas:work',
            profileName: 'Atlas Work',
          }),
          browserProfileFixture({
            browserName: 'Unknown Chromium',
            profileId: 'comet:work',
            profileName: 'Comet Work',
          }),
          browserProfileFixture({
            browserName: 'Unsupported Browser',
            profileId: 'unsupported:Default',
            profileName: 'Unsupported',
          }),
        ],
      }),
    })

    expect(latestWorkflowProps().detectedBrowserProfiles).toHaveLength(4)

    act(() => {
      latestWorkflowProps().onMethodChange('takeout')
    })
    expect(latestWorkflowProps().method).toBe('takeout')

    act(() => {
      latestWorkflowProps().onMethodChange('browser')
    })
    await waitFor(() =>
      expect(latestWorkflowProps().selectedBrowserProfileId).toBe('atlas:work'),
    )
    expect(latestWorkflowProps().sourcePath).toBe('/profiles/Default/History')

    act(() => {
      latestWorkflowProps().onSelectBrowserProfile(
        browserProfileFixture({
          historyPath: null,
          profileId: 'chrome:Missing',
        }),
      )
    })
    expect(latestWorkflowProps().sourcePath).toBe('/profiles/Default/History')
  })

  test('opens browser manual mode when no readable profile is ready', () => {
    renderPage({
      snapshot: snapshotFixture({
        browserProfiles: [
          browserProfileFixture({
            historyPath: '/profiles/Locked/History',
            historyReadable: false,
            profileId: 'chrome:Locked',
            profileName: 'Locked',
          }),
        ],
      }),
    })

    act(() => {
      latestWorkflowProps().onMethodChange('browser')
    })

    expect(latestWorkflowProps().manualPathExpanded).toBe(true)
    expect(latestWorkflowProps().sourcePath).toBe('')
    expect(latestWorkflowProps().selectedBrowserProfileId).toBeNull()
  })

  test('handles source browsing, file-picker fallbacks, and Full Disk Access recovery', async () => {
    const openExternalUrl = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue('opened')
    renderPage()

    openMock.mockResolvedValueOnce(null)
    await act(async () => {
      await latestWorkflowProps().onBrowseSource({ directory: true })
    })
    expect(latestWorkflowProps().sourcePath).toBe('')

    openMock.mockResolvedValueOnce('   ')
    await act(async () => {
      await latestWorkflowProps().onBrowseSource({ directory: false })
    })
    expect(latestWorkflowProps().sourcePath).toBe('')

    openMock.mockResolvedValueOnce('/tmp/takeout.zip')
    await act(async () => {
      await latestWorkflowProps().onBrowseSource({ directory: false })
    })
    expect(latestWorkflowProps().sourcePath).toBe('/tmp/takeout.zip')
    expect(openMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        directory: false,
        title: 'import.chooseTakeoutFile',
      }),
    )

    openMock.mockRejectedValueOnce('dialog unavailable')
    await act(async () => {
      await latestWorkflowProps().onBrowseSource({ directory: false })
    })
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'import.filePickerUnavailable',
    )

    openMock.mockRejectedValueOnce(new Error('native dialog failed'))
    await act(async () => {
      await latestWorkflowProps().onBrowseSource({ directory: false })
    })
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'native dialog failed',
    )

    importReviewStateMock.mockReturnValue(
      importReviewStateFixture({
        actionError: 'Full Disk Access required',
      }),
    )
    renderPage()
    act(() => {
      screen
        .getByRole('button', {
          name: 'import.openFullDiskAccessSettings',
        })
        .click()
    })
    expect(openExternalUrl).toHaveBeenCalledWith(macosFullDiskAccessSettingsUrl)

    openExternalUrl.mockRejectedValueOnce(new Error('settings failed'))
    await act(async () => {
      await latestWorkflowProps().onOpenFullDiskAccessSettings()
    })
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'settings failed',
    )
  })

  test('routes scan and import callbacks through takeout and browser commands', async () => {
    const refreshAppData = vi
      .fn()
      .mockRejectedValue(new Error('refresh failed'))
    const previewImportBatch = vi
      .spyOn(backend, 'previewImportBatch')
      .mockResolvedValue(importBatchDetailFixture())
    const inspectTakeout = vi
      .spyOn(backend, 'inspectTakeout')
      .mockResolvedValue(inspectionFixture({ dryRun: true }))
    const importTakeout = vi
      .spyOn(backend, 'importTakeout')
      .mockResolvedValue(inspectionFixture({ dryRun: false }))
    const inspectBrowserHistory = vi
      .spyOn(backend, 'inspectBrowserHistory')
      .mockResolvedValue(inspectionFixture({ dryRun: true }))
    const importBrowserHistory = vi
      .spyOn(backend, 'importBrowserHistory')
      .mockResolvedValue(
        inspectionFixture({
          dryRun: false,
          importBatch: null,
        }),
      )
    const unsubscribe = vi.fn()
    subscribeToImportProgressMock.mockImplementation((onProgress) => {
      onProgress({ phase: 'importing', processedRecords: 1 })
      return Promise.resolve(unsubscribe)
    })

    renderPage({ refreshAppData })

    act(() => {
      latestWorkflowProps().onSourcePathChange('/tmp/takeout.zip')
    })
    await act(async () => {
      await latestWorkflowProps().onScan()
    })
    expect(inspectTakeout).toHaveBeenCalledWith({
      sourcePath: '/tmp/takeout.zip',
      dryRun: true,
    })
    expect(latestWorkflowProps().step).toBe('preview')

    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(importTakeout).toHaveBeenCalledWith({
      sourcePath: '/tmp/takeout.zip',
      dryRun: false,
    })
    expect(previewImportBatch).toHaveBeenCalledWith(77)
    expect(latestReviewState().selectedBatchIds).toContain(77)
    expect(latestReviewState().loadedBatchDetail?.batch.id).toBe(77)
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'refresh failed',
    )
    expect(unsubscribe).toHaveBeenCalled()

    act(() => {
      latestWorkflowProps().onImportAnother()
    })
    expect(latestWorkflowProps().step).toBe('select')
    expect(latestWorkflowProps().sourcePath).toBe('')

    act(() => {
      latestWorkflowProps().onMethodChange('browser')
    })
    await act(async () => {
      await latestWorkflowProps().onScan()
    })
    expect(inspectBrowserHistory).toHaveBeenCalledWith(
      expect.objectContaining({
        sourcePath: '/profiles/Default/History',
        dryRun: true,
        profileId: 'chrome:Default',
      }),
    )

    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(importBrowserHistory).toHaveBeenCalledWith(
      expect.objectContaining({
        sourcePath: '/profiles/Default/History',
        dryRun: false,
        profileId: 'chrome:Default',
      }),
    )
  })

  test('routes browser manual imports without discovered profile metadata', async () => {
    const inspectBrowserHistory = vi
      .spyOn(backend, 'inspectBrowserHistory')
      .mockResolvedValue(inspectionFixture({ dryRun: true }))
    const importBrowserHistory = vi
      .spyOn(backend, 'importBrowserHistory')
      .mockResolvedValue(
        inspectionFixture({ dryRun: false, importBatch: null }),
      )
    renderPage({
      snapshot: snapshotFixture({
        browserProfiles: [
          browserProfileFixture({
            historyPath: '/profiles/Locked/History',
            historyReadable: false,
            profileId: 'chrome:Locked',
            profileName: 'Locked',
          }),
        ],
      }),
    })

    act(() => {
      latestWorkflowProps().onMethodChange('browser')
    })
    act(() => {
      latestWorkflowProps().onSourcePathChange('/manual/History')
    })
    await act(async () => {
      await latestWorkflowProps().onScan()
    })
    expect(inspectBrowserHistory).toHaveBeenCalledWith({
      sourcePath: '/manual/History',
      dryRun: true,
      browserFamily: null,
      profileId: null,
      browserName: null,
      profileName: null,
    })

    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(importBrowserHistory).toHaveBeenCalledWith({
      sourcePath: '/manual/History',
      dryRun: false,
      browserFamily: null,
      profileId: null,
      browserName: null,
      profileName: null,
    })
  })

  test('reports scan/import and batch preview failures without keeping the busy state stuck', async () => {
    vi.spyOn(backend, 'inspectTakeout').mockRejectedValue(
      new Error('scan failed'),
    )
    vi.spyOn(backend, 'importTakeout').mockRejectedValue(
      new Error('import failed'),
    )
    renderPage()

    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(latestWorkflowProps().step).toBe('select')

    await act(async () => {
      await latestWorkflowProps().onScan()
    })
    expect(latestReviewState().reportedErrors).toHaveLength(0)

    act(() => {
      latestWorkflowProps().onSourcePathChange('/tmp/takeout.zip')
    })
    await act(async () => {
      await latestWorkflowProps().onScan()
    })
    expect(latestWorkflowProps().step).toBe('select')
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'scan failed',
    )

    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(latestWorkflowProps().importing).toBe(false)
    expect(latestWorkflowProps().step).toBe('preview')
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'import failed',
    )

    vi.mocked(backend.importTakeout).mockResolvedValueOnce(
      inspectionFixture({ dryRun: false }),
    )
    vi.spyOn(backend, 'previewImportBatch').mockRejectedValueOnce(
      new Error('batch preview failed'),
    )
    await act(async () => {
      await latestWorkflowProps().onImport()
    })
    expect(latestReviewState().loadedBatchDetail).toBeNull()
    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'batch preview failed',
    )
  })

  test('keeps confirm state when the shell import action returns a running task', async () => {
    const runningTask = shellTaskFixture({ title: 'Existing import task' })
    const runImport = vi.fn().mockResolvedValue(runningTask)
    renderPage({
      shellOverrides: { runImport },
    })

    act(() => {
      latestWorkflowProps().onSourcePathChange('/tmp/takeout.zip')
    })
    await act(async () => {
      await latestWorkflowProps().onImport()
    })

    expect(runImport).toHaveBeenCalled()
    expect(latestWorkflowProps().step).toBe('confirm')
    expect(latestWorkflowProps().importing).toBe(false)
  })

  test('starts browser imports without an inspection total when preview is skipped', async () => {
    const runningTask = shellTaskFixture({ title: 'Existing browser import' })
    const runImport = vi.fn().mockResolvedValue(runningTask)
    renderPage({
      shellOverrides: { runImport },
    })

    act(() => {
      latestWorkflowProps().onMethodChange('browser')
    })
    act(() => {
      latestWorkflowProps().onSourcePathChange('/manual/History')
    })
    await act(async () => {
      await latestWorkflowProps().onImport()
    })

    expect(runImport).toHaveBeenCalledWith(
      expect.objectContaining({
        method: 'browser',
        expectedRecords: null,
        sourceLabel: '/manual/History',
      }),
    )
  })

  test('reports an action error when the shell import action is unavailable', async () => {
    renderPage({
      shellOverrides: { runImport: undefined },
    })

    act(() => {
      latestWorkflowProps().onSourcePathChange('/tmp/takeout.zip')
    })
    await act(async () => {
      await latestWorkflowProps().onImport()
    })

    expect(latestReviewState().reportedErrors.at(-1)?.message).toBe(
      'import.actionErrorTitle',
    )
  })
})

function renderPage({
  refreshAppData = vi.fn().mockResolvedValue(undefined),
  shellOverrides = {},
  snapshot = snapshotFixture(),
}: {
  refreshAppData?: () => Promise<void>
  shellOverrides?: Partial<ReturnType<typeof shellFixture>>
  snapshot?: ReturnType<typeof snapshotFixture> | null
} = {}) {
  shellDataMock.mockReturnValue(
    shellFixture({
      refreshAppData,
      snapshot,
      ...shellOverrides,
    }),
  )
  return render(
    <MemoryRouter>
      <ImportPage />
    </MemoryRouter>,
  )
}

function latestWorkflowProps() {
  return workflowPropsMock.mock.calls.at(-1)?.[0] as ImportWorkflowPanelProps
}

function latestReviewState() {
  return importReviewStateMock.mock.results.at(-1)?.value as ReturnType<
    typeof importReviewStateFixture
  >
}

function shellFixture(
  overrides: Partial<{
    archiveTasks: ShellTask[]
    refreshAppData: () => Promise<void>
    runImport:
      | ((
          request: ShellImportTaskRequest,
        ) => Promise<TakeoutInspection | ShellTask>)
      | undefined
    snapshot: ReturnType<typeof snapshotFixture> | null
  }> = {},
) {
  return {
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    runImport: vi.fn(async (request: ShellImportTaskRequest) => {
      const unsubscribe = await subscribeToImportProgressMock(() => undefined)
      try {
        return request.method === 'takeout'
          ? await backend.importTakeout(request.request)
          : await backend.importBrowserHistory(request.request)
      } finally {
        unsubscribe()
      }
    }),
    snapshot: snapshotFixture(),
    ...overrides,
  }
}

function shellTaskFixture(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'task-import',
    kind: 'import',
    state: 'running',
    title: 'Import task',
    detail: 'Writing archive records',
    startedAt: '2026-04-27T10:00:00.000Z',
    updatedAt: '2026-04-27T10:01:00.000Z',
    finishedAt: null,
    progressValue: null,
    logEntries: [],
    ...overrides,
  }
}

function snapshotFixture({
  browserProfiles = [browserProfileFixture()],
  initialized = true,
}: {
  browserProfiles?: BrowserProfile[]
  initialized?: boolean
} = {}) {
  return {
    browserProfiles,
    config: {
      initialized,
    },
    recentImportBatches: [],
  }
}

function importReviewStateFixture(
  overrides: Partial<ReturnType<typeof importReviewStateFixtureBase>> = {},
) {
  return {
    ...importReviewStateFixtureBase(),
    ...overrides,
  }
}

function importReviewStateFixtureBase() {
  const reportedErrors: Error[] = []
  const selectedBatchIds: number[] = []
  return {
    actionError: null as string | null,
    activeBatchDetail: null,
    clearActionError: clearActionErrorMock,
    handleBatchMutation: vi.fn(),
    handleRepairHealth: vi.fn(),
    handleRunDoctor: vi.fn(),
    handleSupportPathCopy: vi.fn(),
    handleSupportPathOpen: vi.fn(),
    healthReport: null,
    loadedBatchDetail: undefined as ImportBatchDetail | null | undefined,
    loadingBatch: false,
    recentImportBatches: [],
    repairNotice: null,
    reportedErrors,
    reportActionError: vi.fn((error: unknown) => {
      reportedErrors.push(
        error instanceof Error ? error : new Error(String(error)),
      )
    }),
    selectBatchId: vi.fn((id: number) => {
      selectedBatchIds.push(id)
    }),
    selectedBatchId: null,
    selectedBatchIds,
    setLoadedBatchDetail: vi.fn((detail: ImportBatchDetail | null) => {
      latestReviewState().loadedBatchDetail = detail
    }),
    supportCopyFeedback: null,
  }
}

function browserProfileFixture(
  overrides: Partial<BrowserProfile> = {},
): BrowserProfile {
  return {
    appDisplayName: 'Google Chrome',
    browserFamily: 'chromium',
    browserName: 'Google Chrome',
    faviconsPath: '/profiles/Default/Favicons',
    historyExists: true,
    historyPath: '/profiles/Default/History',
    historyReadable: true,
    lastVisitedAt: null,
    profileId: 'chrome:Default',
    profileName: 'Default',
    profilePath: '/profiles/Default',
    ...overrides,
  } as BrowserProfile
}

function inspectionFixture(
  overrides: Partial<TakeoutInspection> = {},
): TakeoutInspection {
  return {
    candidateItems: 1,
    detectedLocale: null,
    dryRun: false,
    duplicateItems: 0,
    importBatch: importBatchOverviewFixture(),
    importedItems: 1,
    notes: [],
    previewEntries: [],
    previewRangeEnd: '2026-04-25T12:00:00.000Z',
    previewRangeStart: '2026-04-25T12:00:00.000Z',
    quarantinedFiles: [],
    recognizedFiles: [],
    sourcePath: '/tmp/takeout.zip',
    ...overrides,
  }
}

function importBatchOverviewFixture(
  overrides: Partial<ImportBatchOverview> = {},
): ImportBatchOverview {
  return {
    auditPath: '/tmp/import-audit.json',
    candidateItems: 1,
    createdAt: '2026-04-25T12:00:00.000Z',
    duplicateItems: 0,
    gitCommit: null,
    id: 77,
    importedAt: '2026-04-25T12:01:00.000Z',
    importedItems: 1,
    profileId: 'chrome:Default',
    revertedAt: null,
    sourceKind: 'takeout',
    sourcePath: '/tmp/takeout.zip',
    status: 'imported',
    visibleItems: 1,
    ...overrides,
  }
}

function importBatchDetailFixture(): ImportBatchDetail {
  return {
    batch: importBatchOverviewFixture(),
    detectedLocale: null,
    notes: [],
    previewEntries: [],
    previewRangeEnd: '2026-04-25T12:00:00.000Z',
    previewRangeStart: '2026-04-25T12:00:00.000Z',
    quarantinedFiles: [],
    recognizedFiles: [],
  }
}
