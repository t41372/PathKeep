import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { ShellDataContextValue } from '../../app/shell-data-context'
import type {
  AppSnapshot,
  AuditRunDetail,
  BackupRunOverview,
} from '../../lib/types'
import { AuditPage } from './index'
import type { AuditDetailTab } from './types'

const { auditDataMock, panelPropsMock, shellDataMock } = vi.hoisted(() => ({
  auditDataMock: vi.fn(),
  panelPropsMock: vi.fn(),
  shellDataMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: shellDataMock,
}))

vi.mock('../../lib/i18n', () => ({
  localeTag: () => 'en-US',
  useI18n: () => ({
    language: 'en',
    t: (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}))

vi.mock('./hooks/use-audit-data', () => ({
  useAuditData: auditDataMock,
}))

vi.mock('./panels/run-detail', () => ({
  AuditRunDetailPanel: (props: unknown) => {
    panelPropsMock(props)
    return <div data-testid="audit-detail-panel">detail panel</div>
  },
}))

describe('AuditPage route owner', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    auditDataMock.mockReturnValue(auditDataFixture())
    shellDataMock.mockReturnValue(shellFixture())
  })

  test('renders loading, unavailable, onboarding, and empty-ledger gates', async () => {
    const user = userEvent.setup()
    const runBackup = vi.fn().mockRejectedValue(new Error('backup failed'))
    shellDataMock.mockReturnValue(
      shellFixture({
        loading: true,
        snapshot: null,
      }),
    )

    const { rerender } = renderPage()

    expect(screen.getByText('audit.loadingLedger')).toBeVisible()

    shellDataMock.mockReturnValue(
      shellFixture({
        error: 'archive unavailable',
        snapshot: null,
      }),
    )
    rerender(pageNode())
    expect(screen.getByText('audit.unavailableTitle')).toBeVisible()
    expect(screen.getByText('archive unavailable')).toBeVisible()

    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({
          initialized: false,
          recentRuns: [],
        }),
      }),
    )
    rerender(pageNode())
    expect(screen.getByText('audit.emptyLedgerTitle')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'audit.finishOnboarding' }),
    ).toHaveAttribute('href', '/onboarding')

    shellDataMock.mockReturnValue(
      shellFixture({
        runBackup,
        snapshot: snapshotFixture({
          initialized: true,
          recentRuns: [],
        }),
      }),
    )
    rerender(pageNode())
    expect(screen.getByText('audit.noRunsTitle')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: 'audit.runManualBackup' }),
    )
    expect(runBackup).toHaveBeenCalledTimes(1)
  })

  test('filters runs by route-owned controls and exposes restore-kind labeling to the detail panel', async () => {
    const user = userEvent.setup()
    const runs = [
      runFixture(11, {
        newDownloads: 3,
        newUrls: 7,
        newVisits: 12,
        profileScope: ['takeout::browser-history'],
        runType: 'import',
      }),
      runFixture(10, {
        newDownloads: 2,
        newUrls: 5,
        newVisits: 8,
        profileScope: ['chrome:Default'],
        runType: 'backup',
        trigger: 'schedule',
      }),
      runFixture(9, {
        newDownloads: 1,
        newUrls: 4,
        newVisits: 6,
        profileScope: [],
        runType: 'doctor',
      }),
      runFixture(8, {
        profileScope: ['opera:Daily'],
        runType: 'backup',
      }),
    ]
    const details = {
      11: detailFixture(runs[0], {
        artifacts: [artifactFixture('manifest', '/audit/run-11.json')],
        warnings: ['import warning'],
      }),
      10: detailFixture(runs[1], {
        artifacts: [
          artifactFixture('manifest', '/audit/run-10.json'),
          artifactFixture('snapshot', '/audit/run-10.snapshot'),
        ],
        warnings: ['scheduled warning'],
      }),
      9: detailFixture(runs[2], {
        artifacts: [artifactFixture('snapshot', '/audit/run-9.snapshot')],
      }),
      8: detailFixture(runs[3], {
        artifacts: [artifactFixture('manifest', '/audit/run-8.json')],
      }),
    }

    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({
          recentRuns: runs,
        }),
      }),
    )
    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: details[11],
        detailCache: details,
        detailSeverity: 'warning',
      }),
    )

    renderPage('/audit?run=11')

    await user.selectOptions(screen.getByLabelText('audit.filterSource'), [
      'opera',
    ])
    expect(screen.getByRole('button', { name: /#8/ })).toBeVisible()
    expect(screen.getByText('opera')).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#11/ }),
    ).not.toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('audit.filterSource'), [
      'all',
    ])
    await user.selectOptions(screen.getByLabelText('audit.filterProfile'), [
      'chrome:Default',
    ])
    expect(screen.getByRole('button', { name: /#10/ })).toBeVisible()
    expect(screen.queryByRole('button', { name: /#9/ })).not.toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('audit.filterProfile'), [
      'archive-wide',
    ])
    expect(screen.getByRole('button', { name: /#9/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#10/ }),
    ).not.toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('audit.filterProfile'), [
      'all',
    ])
    await user.selectOptions(
      screen.getByLabelText('audit.filterArtifactType'),
      ['snapshot'],
    )
    await waitFor(() =>
      expect(
        screen.queryByRole('button', { name: /#11/ }),
      ).not.toBeInTheDocument(),
    )
    await user.click(screen.getByRole('button', { name: /#10/ }))
    expect(
      screen.getByText('audit.deltaComparedToRun:{"runId":9}'),
    ).toBeVisible()
    expect(screen.getByText('+6')).toBeVisible()

    await user.selectOptions(
      screen.getByLabelText('audit.filterArtifactType'),
      ['all'],
    )
    await user.selectOptions(screen.getByLabelText('audit.filterSeverity'), [
      'blocked',
    ])
    expect(screen.getByText('audit.noMatchingRuns')).toBeVisible()

    expect(panelPropsMock).toHaveBeenCalled()
    const latestPanelProps = panelPropsMock.mock.calls.at(-1)?.[0] as {
      restoreKindLabel: (kind: string) => string
    }
    expect(latestPanelProps.restoreKindLabel('archive-safety-snapshot')).toBe(
      'audit.restoreKindArchiveSafety',
    )
    expect(latestPanelProps.restoreKindLabel('raw-source')).toBe(
      'audit.restoreKindRawSource',
    )
  })

  test('triages warning and blocked runs from the health summary without dropping other active filters', async () => {
    const user = userEvent.setup()
    const runs = [
      runFixture(21, {
        profileScope: ['chrome:Default'],
      }),
      runFixture(20, {
        profileScope: ['chrome:Default'],
      }),
      runFixture(19, {
        profileScope: ['safari:Personal'],
      }),
    ]
    const details = {
      21: detailFixture(runs[0], {
        errorMessage: 'manifest write failed',
      }),
      20: detailFixture(runs[1], {
        warnings: ['schedule drift'],
      }),
      19: detailFixture(runs[2], {
        warnings: ['safari warning outside active profile'],
      }),
    }

    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({
          recentRuns: runs,
        }),
      }),
    )
    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: details[21],
        detailCache: details,
        detailSeverity: 'blocked',
      }),
    )

    renderPage('/audit')

    await user.selectOptions(screen.getByLabelText('audit.filterProfile'), [
      'chrome:Default',
    ])
    expect(
      screen.getByText('audit.ledgerHealthIssues:{"warning":1,"blocked":1}'),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: 'audit.triageShowBlocked:{"count":1}',
      }),
    )
    expect(screen.getByRole('button', { name: /#21/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#20/ }),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', {
        name: 'audit.triageShowWarning:{"count":1}',
      }),
    )
    expect(screen.getByRole('button', { name: /#20/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#21/ }),
    ).not.toBeInTheDocument()
  })

  test('does not render warning triage when all visible repair runs are blocked', () => {
    const runs = [runFixture(30)]
    const details = {
      30: detailFixture(runs[0], {
        errorMessage: 'manifest write failed',
      }),
    }

    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({
          recentRuns: runs,
        }),
      }),
    )
    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: details[30],
        detailCache: details,
        detailSeverity: 'blocked',
      }),
    )

    renderPage('/audit')

    expect(
      screen.getByRole('button', {
        name: 'audit.triageShowBlocked:{"count":1}',
      }),
    ).toBeVisible()
    expect(
      screen.queryByRole('button', {
        name: /audit\.triageShowWarning/,
      }),
    ).not.toBeInTheDocument()
  })

  test('covers mixed-source fallbacks, missing details, and detail gate states', async () => {
    const user = userEvent.setup()
    const runs = [
      runFixture(15, {
        finishedAt: undefined,
        newDownloads: 0,
        newUrls: 2,
        newVisits: 5,
        profileScope: ['firefox:default-release'],
        runType: undefined,
        trigger: undefined,
      }),
      runFixture(14, {
        newDownloads: 4,
        newUrls: 2,
        newVisits: 5,
        profileScope: ['safari:Personal'],
        runType: 'backup',
        trigger: undefined,
      }),
      runFixture(13, {
        newDownloads: 4,
        newUrls: 8,
        newVisits: 2,
        profileScope: undefined,
        runType: 'import',
        trigger: undefined,
      }),
      runFixture(12, {
        newDownloads: 1,
        newUrls: 3,
        newVisits: 4,
        profileScope: ['takeout::browser-history'],
        runType: 'import',
        trigger: 'manual',
      }),
      runFixture(11, {
        finishedAt: undefined,
        newDownloads: 1,
        newUrls: 3,
        newVisits: 4,
        profileScope: ['chrome:Default'],
        runType: 'doctor',
        startedAt: 'invalid-started-at',
        trigger: 'manual',
      }),
    ]
    const details = {
      15: detailFixture(runs[0], {
        profileScope: ['firefox:default-release'],
        trigger: 'schedule',
      }),
      14: detailFixture(runs[1], {
        profileScope: ['safari:Personal'],
        trigger: 'manual',
      }),
      12: detailFixture(runs[3], {
        profileScope: ['takeout::browser-history'],
      }),
      11: detailFixture(runs[4], {
        profileScope: ['chrome:Default'],
      }),
    }

    shellDataMock.mockReturnValue(
      shellFixture({
        snapshot: snapshotFixture({
          recentRuns: runs,
        }),
      }),
    )
    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: details[15],
        detailCache: details,
        detailSeverity: null,
      }),
    )

    const { rerender } = renderPage('/audit?run=15')

    expect(screen.getByText('audit.filtersLoading')).toBeVisible()
    expect(screen.getByText('audit.sourceChrome')).toBeVisible()
    expect(screen.getByText('audit.sourceFirefox')).toBeVisible()
    expect(screen.getByText('audit.sourceSafari')).toBeVisible()
    expect(screen.getByText('audit.sourceTakeout')).toBeVisible()
    expect(screen.getByText('audit.archiveWide')).toBeVisible()

    expect(
      screen.getByRole('button', {
        name: /#15 · audit.runTypeBackup · audit.scheduledBackup/,
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', {
        name: /#13 · audit.runTypeImport · audit.manualBackup/,
      }),
    ).toBeVisible()
    expect(screen.getByText('invalid-started-at')).toBeVisible()

    await user.click(screen.getByRole('button', { name: /#14/ }))
    expect(
      screen.getByText('audit.deltaComparedToRun:{"runId":13}'),
    ).toBeVisible()
    expect(screen.getAllByText('0').length).toBeGreaterThan(0)
    expect(screen.getByText('-6')).toBeVisible()
    expect(screen.getByText('-4')).toBeVisible()
    expect(screen.getByText('+3')).toBeVisible()

    await user.selectOptions(screen.getByLabelText('audit.filterRunType'), [
      'backup',
    ])
    expect(screen.getByRole('button', { name: /#15/ })).toBeVisible()
    expect(screen.getByRole('button', { name: /#14/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#13/ }),
    ).not.toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('audit.filterRunType'), [
      'all',
    ])
    await user.selectOptions(screen.getByLabelText('audit.filterSeverity'), [
      'warning',
    ])
    expect(screen.getByText('audit.noMatchingRuns')).toBeVisible()

    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: null,
        detailCache: details,
        detailSeverity: null,
        loading: true,
      }),
    )
    rerender(pageNode('/audit?run=15'))
    expect(screen.getByText('audit.loadingRunDetail')).toBeVisible()

    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: null,
        detailCache: details,
        detailSeverity: null,
        error: 'detail transport failed',
      }),
    )
    rerender(pageNode('/audit?run=15'))
    expect(screen.getByText('audit.runDetailUnavailable')).toBeVisible()
    expect(screen.getByText('detail transport failed')).toBeVisible()

    auditDataMock.mockReturnValue(
      auditDataFixture({
        detail: null,
        detailCache: details,
        detailSeverity: null,
      }),
    )
    rerender(pageNode('/audit?run=15'))
    expect(screen.getByText('audit.detailEmptyTitle')).toBeVisible()
  })
})

function renderPage(route = '/audit') {
  return render(pageNode(route))
}

function pageNode(route = '/audit') {
  return (
    <MemoryRouter initialEntries={[route]}>
      <AuditPage />
    </MemoryRouter>
  )
}

function shellFixture(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
  const snapshot =
    'snapshot' in overrides ? overrides.snapshot : snapshotFixture()

  return {
    appLockStatus: null,
    buildInfo: null,
    busyAction: null,
    busyOverlay: null,
    dashboard: null,
    error: null,
    loading: false,
    notice: null,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshKey: 1,
    refreshRuntimeStatus: vi.fn().mockResolvedValue({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    }),
    snapshot,
    initializeArchive: vi.fn(),
    runBackup: vi.fn(),
    saveConfig: vi.fn(),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    clearNotice: vi.fn(),
    lockAppSession: vi.fn(),
    unlockAppSession: vi.fn(),
    ...overrides,
  } as ShellDataContextValue
}

function snapshotFixture({
  initialized = true,
  recentRuns = [runFixture(11), runFixture(10)],
}: {
  initialized?: boolean
  recentRuns?: BackupRunOverview[]
} = {}): AppSnapshot {
  return {
    config: {
      initialized,
    },
    recentImportBatches: [],
    recentRuns,
  } as unknown as AppSnapshot
}

interface AuditDataMockValue {
  batchActionError: string | null
  batchActionNotice: string | null
  copyFeedback: unknown
  detail: AuditRunDetail | null
  detailCache: Record<number, AuditRunDetail>
  detailSeverity: 'clear' | 'warning' | 'blocked' | null
  detailTab: AuditDetailTab
  error: string | null
  handleCopyPath: ReturnType<typeof vi.fn>
  handleExecuteRestore: ReturnType<typeof vi.fn>
  handlePreviewRestore: ReturnType<typeof vi.fn>
  handleRelatedBatchMutation: ReturnType<typeof vi.fn>
  loading: boolean
  loadingRelatedBatch: boolean
  relatedBatchDetail: unknown
  relatedBatchError: string | null
  relatedImportBatch: unknown
  restoreBusy: boolean
  restoreError: string | null
  restoreNotice: string | null
  restorePreview: unknown
  setDetailTab: ReturnType<typeof vi.fn>
}

function auditDataFixture(
  overrides: Partial<AuditDataMockValue> = {},
): AuditDataMockValue {
  return {
    ...auditDataFixtureBase(),
    ...overrides,
  }
}

function auditDataFixtureBase(): AuditDataMockValue {
  const run = runFixture(11)
  const detail = detailFixture(run)

  return {
    batchActionError: null,
    batchActionNotice: null,
    copyFeedback: null,
    detail,
    detailCache: {
      [run.id]: detail,
      10: detailFixture(runFixture(10)),
    },
    detailSeverity: 'clear',
    detailTab: 'summary',
    error: null,
    handleCopyPath: vi.fn(),
    handleExecuteRestore: vi.fn(),
    handlePreviewRestore: vi.fn(),
    handleRelatedBatchMutation: vi.fn(),
    loading: false,
    loadingRelatedBatch: false,
    relatedBatchDetail: null,
    relatedBatchError: null,
    relatedImportBatch: null,
    restoreBusy: false,
    restoreError: null,
    restoreNotice: null,
    restorePreview: null,
    setDetailTab: vi.fn(),
  }
}

function runFixture(
  id: number,
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    id,
    startedAt: `2026-04-${String(id).padStart(2, '0')}T10:00:00.000Z`,
    finishedAt: `2026-04-${String(id).padStart(2, '0')}T10:05:00.000Z`,
    status: 'success',
    runType: 'backup',
    trigger: 'manual',
    profileScope: ['chrome:Default'],
    manifestHash: `hash-${id}`,
    profilesProcessed: 1,
    newVisits: 2,
    newUrls: 1,
    newDownloads: 0,
    ...overrides,
  }
}

function detailFixture(
  run: BackupRunOverview,
  overrides: Partial<AuditRunDetail> = {},
): AuditRunDetail {
  return {
    run,
    artifacts: [artifactFixture('manifest', `/audit/run-${run.id}.json`)],
    dueOnly: false,
    errorMessage: null,
    manifestHash: run.manifestHash,
    manifestPath: `/audit/run-${run.id}.json`,
    profileScope: run.profileScope ?? [],
    stats: {},
    timezone: 'UTC',
    trigger: run.trigger ?? 'manual',
    warnings: [],
    ...overrides,
  }
}

function artifactFixture(kind: string, path: string) {
  return {
    kind,
    path,
    createdAt: '2026-04-25T12:00:00.000Z',
  }
}
