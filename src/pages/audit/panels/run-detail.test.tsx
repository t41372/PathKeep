/**
 * @file run-detail.test.tsx
 * @description Interaction coverage for the Audit run detail review panel.
 * @module pages/audit/panels
 *
 * ## Responsibilities
 * - Verify summary, related import-batch actions, artifact copy/open/restore preview, execute restore, and warnings states.
 * - Protect the Audit ledger review surface from silently dropping rollback/restore controls.
 * - Keep panel behavior covered without mounting the full Audit route data hook.
 *
 * ## Not responsible for
 * - Re-testing backend audit or import mutation implementations.
 * - Re-testing the shared ImportBatchReview component in isolation.
 *
 * ## Dependencies
 * - Uses MemoryRouter because the panel exposes route links.
 * - Uses backend-client spies for open-path actions that cross the desktop bridge.
 *
 * ## Performance notes
 * - Fixtures stay small and bounded; this is a render-only panel test.
 */

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../../lib/backend-client'
import type {
  AuditRunDetail,
  ImportBatchDetail,
  ImportBatchOverview,
  SnapshotRestorePreview,
} from '../../../lib/types'
import { AuditRunDetailPanel } from './run-detail'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('AuditRunDetailPanel', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('renders summary state and executes related import batch actions', async () => {
    const user = userEvent.setup()
    const mutateBatch = vi.fn().mockResolvedValue(undefined)
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep/import-audit.json')

    const { rerender } = renderPanel({
      batchActionError: 'restore failed',
      batchActionNotice: 'batch restored',
      handleRelatedBatchMutation: mutateBatch,
      loadingRelatedBatch: false,
      relatedBatchDetail: importBatchDetailFixture({
        status: 'imported',
      }),
      relatedImportBatch: importBatchOverviewFixture({
        status: 'imported',
      }),
    })

    expect(screen.getByText('audit.reviewGuideTitle')).toBeVisible()
    expect(screen.getByText('audit.importBatchLabel:{"id":"77"}')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'audit.openImportReview' }),
    ).toHaveAttribute('href', '/import?batch=77')

    await user.click(
      screen.getByRole('button', { name: 'audit.openImportArtifact' }),
    )
    await user.click(screen.getByRole('button', { name: 'import.revertBatch' }))

    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/import-audit.json')
    expect(mutateBatch).toHaveBeenCalledWith('revert')
    expect(screen.getByRole('status')).toHaveTextContent('batch restored')
    expect(screen.getByRole('alert')).toHaveTextContent('restore failed')
    expect(
      screen.getByRole('button', { name: 'import.restoreBatch' }),
    ).toBeDisabled()

    rerender(
      panelNode({
        handleRelatedBatchMutation: mutateBatch,
        relatedBatchDetail: importBatchDetailFixture({
          status: 'reverted',
        }),
        relatedImportBatch: importBatchOverviewFixture({
          status: 'reverted',
        }),
      }),
    )
    await user.click(
      screen.getByRole('button', { name: 'import.restoreBatch' }),
    )
    expect(mutateBatch).toHaveBeenCalledWith('restore')
  })

  test('renders artifacts, path actions, restore preview, execute restore, and warning tab', async () => {
    const user = userEvent.setup()
    const setDetailTab = vi.fn()
    const copyPath = vi.fn().mockResolvedValue(undefined)
    const previewRestore = vi.fn().mockResolvedValue(undefined)
    const executeRestore = vi.fn().mockResolvedValue(undefined)
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep/snapshot.sqlite')

    const { rerender } = renderPanel({
      copyFeedback: {
        key: '/tmp/pathkeep/manifest.json',
        tone: 'success',
      },
      detailTab: 'artifacts',
      handleCopyPath: copyPath,
      handleExecuteRestore: executeRestore,
      handlePreviewRestore: previewRestore,
      restoreError: 'restore blocked',
      restoreNotice: 'restore complete',
      restorePreview: restorePreviewFixture({ executeSupported: true }),
      setDetailTab,
    })

    expect(screen.getByText('audit.artifacts:{"count":2}')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: 'audit.previewRestore' }),
    )
    expect(previewRestore).toHaveBeenCalledWith('/tmp/pathkeep/snapshot.sqlite')

    const manifestPathRow = screen
      .getAllByText('/tmp/pathkeep/manifest.json')[0]
      .closest('.review-path-action-row')
    expect(manifestPathRow).not.toBeNull()
    if (!(manifestPathRow instanceof HTMLElement)) {
      throw new Error('expected manifest path row')
    }
    await user.click(
      within(manifestPathRow).getByRole('button', {
        name: 'common.copyAction',
      }),
    )
    await user.click(
      within(manifestPathRow).getByRole('button', {
        name: 'common.openAction',
      }),
    )

    expect(copyPath).toHaveBeenCalledWith('/tmp/pathkeep/manifest.json')
    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/manifest.json')
    expect(screen.getByText('audit.restoreReady')).toBeVisible()
    expect(screen.getByText('kind:snapshot')).toBeVisible()
    expect(screen.getByText('snapshot warning')).toBeVisible()

    const restoreOpenButtons = screen.getAllByRole('button', {
      name: 'common.openAction',
    })
    const restoreOpenButton = restoreOpenButtons.at(-1)
    if (!restoreOpenButton) {
      throw new Error('expected restore preview open action')
    }
    await user.click(restoreOpenButton)
    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/snapshot.sqlite')

    const manifestRows = screen.getAllByText('/tmp/pathkeep/manifest.json')
    const manifestFooterRow = manifestRows
      .at(-1)
      ?.closest('.review-path-action-row')
    if (!(manifestFooterRow instanceof HTMLElement)) {
      throw new Error('expected manifest footer path row')
    }
    await user.click(
      within(manifestFooterRow).getByRole('button', {
        name: 'audit.copyPath',
      }),
    )
    await user.click(
      within(manifestFooterRow).getByRole('button', {
        name: 'audit.viewManifest',
      }),
    )
    expect(copyPath).toHaveBeenCalledWith('/tmp/pathkeep/manifest.json')
    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/manifest.json')

    await user.click(
      screen.getByRole('button', { name: 'audit.executeRestore' }),
    )
    expect(executeRestore).toHaveBeenCalledTimes(1)
    expect(screen.getByText('restore complete')).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('restore blocked')

    rerender(
      panelNode({
        detailTab: 'warnings',
        setDetailTab,
      }),
    )
    expect(screen.getByText('source warning')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'audit.summaryTab' }))
    expect(setDetailTab).toHaveBeenCalledWith('summary')
  })

  test('renders loading, unavailable, and manual-only restore fallbacks', () => {
    const { rerender } = renderPanel({
      loadingRelatedBatch: true,
      relatedBatchDetail: null,
      relatedImportBatch: null,
    })

    expect(screen.getByText('common.loading')).toBeVisible()

    rerender(
      panelNode({
        loadingRelatedBatch: false,
        relatedBatchDetail: null,
        relatedBatchError: 'batch unavailable',
        relatedImportBatch: null,
      }),
    )
    expect(screen.getByText('batch unavailable')).toBeVisible()

    rerender(
      panelNode({
        loadingRelatedBatch: false,
        relatedBatchDetail: null,
        relatedBatchError: null,
        relatedImportBatch: null,
      }),
    )
    expect(
      screen.getByText('audit.changePreviewUnavailableTitle'),
    ).toBeVisible()

    rerender(
      panelNode({
        detail: auditRunDetailFixture({ artifacts: [], manifestPath: null }),
        detailTab: 'artifacts',
        restorePreview: restorePreviewFixture({ executeSupported: false }),
      }),
    )
    expect(screen.getByText('common.notAvailable')).toBeVisible()
    expect(screen.getByText('audit.restoreManualOnly')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'audit.executeRestore' }),
    ).toBeDisabled()

    rerender(
      panelNode({
        detail: auditRunDetailFixture({ warnings: [] }),
        detailTab: 'warnings',
      }),
    )
    expect(screen.getByText('audit.noWarnings')).toBeVisible()
  })

  test('renders an error callout for failed runs with an errorMessage', () => {
    const failedRun = {
      ...auditRunDetailFixture().run,
      status: 'failed' as const,
      errorMessage:
        'Full Disk Access required — Safari History.db is not readable yet',
    }
    renderPanel({
      detail: auditRunDetailFixture({
        run: failedRun,
        errorMessage: failedRun.errorMessage,
      }),
      detailSeverity: 'blocked',
    })

    // The StatusCallout rendered for a failed run must show both the failure
    // eyebrow/title key and the raw error message from the backend.
    expect(screen.getByText('shell.backupRunFailed')).toBeVisible()
    expect(screen.getByText('shell.backupRunErrorReason')).toBeVisible()
    expect(
      screen.getByText(
        'Full Disk Access required — Safari History.db is not readable yet',
      ),
    ).toBeVisible()
  })

  test('does not render a failed-run callout for completed runs', () => {
    renderPanel({ detail: auditRunDetailFixture(), detailSeverity: 'clear' })
    expect(screen.queryByText('shell.backupRunFailed')).not.toBeInTheDocument()
  })

  test('renders summary and restore fallback branches for incomplete audit records', () => {
    const detail = auditRunDetailFixture({
      manifestHash: null,
      manifestPath: null,
      profileScope: [],
      run: {
        ...auditRunDetailFixture().run,
        runType: undefined,
        startedAt: 'invalid-started-at',
        trigger: 'schedule',
      },
      trigger: undefined as unknown as string,
    })

    const { rerender } = renderPanel({
      detail,
      detailSeverity: null,
      relatedBatchDetail: importBatchDetailFixture({
        auditPath: null,
        importedItems: 1,
        visibleItems: 3,
      }),
      relatedImportBatch: null,
    })

    expect(screen.getByText('audit.runTypeBackup')).toBeVisible()
    expect(screen.getByText('audit.archiveWide')).toBeVisible()
    expect(screen.getByText('invalid-started-at')).toBeVisible()
    expect(screen.getByText('audit.scheduledBackup')).toBeVisible()
    expect(screen.getAllByText('common.notAvailable').length).toBeGreaterThan(1)
    expect(
      screen.getByText('audit.changePreviewUnavailableShort'),
    ).toBeVisible()
    expect(
      screen.queryByRole('button', { name: 'audit.openImportArtifact' }),
    ).not.toBeInTheDocument()
    expect(screen.getByText('0')).toBeVisible()

    rerender(
      panelNode({
        detail: auditRunDetailFixture(),
        detailTab: 'artifacts',
        restoreBusy: true,
        restorePreview: restorePreviewFixture({
          createdAt: null,
          executeSupported: true,
          sourceProfileId: null,
          warnings: [],
        }),
      }),
    )

    expect(screen.getByText('audit.restoreReady')).toBeVisible()
    expect(screen.getByText('audit.archiveWide')).toBeVisible()
    expect(screen.getByText('common.notAvailable')).toBeVisible()
    const loadingButtons = screen.getAllByRole('button', {
      name: 'common.loading',
    })
    expect(loadingButtons).toHaveLength(2)
    expect(
      loadingButtons.some((button) => button.hasAttribute('disabled')),
    ).toBe(true)

    rerender(
      panelNode({
        detail: auditRunDetailFixture(),
        detailTab: 'artifacts',
        restorePreview: restorePreviewFixture({
          createdAt: 'invalid-restore-date',
          warnings: [],
        }),
      }),
    )
    expect(screen.getByText('invalid-restore-date')).toBeVisible()
  })
})

function renderPanel(
  overrides: Partial<Parameters<typeof AuditRunDetailPanel>[0]> = {},
) {
  return render(panelNode(overrides))
}

function panelNode(
  overrides: Partial<Parameters<typeof AuditRunDetailPanel>[0]> = {},
) {
  return (
    <MemoryRouter>
      <AuditRunDetailPanel
        batchActionError={null}
        batchActionNotice={null}
        copyFeedback={null}
        detail={auditRunDetailFixture()}
        detailSeverity="warning"
        detailTab="summary"
        handleCopyPath={vi.fn().mockResolvedValue(undefined)}
        handleExecuteRestore={vi.fn().mockResolvedValue(undefined)}
        handlePreviewRestore={vi.fn().mockResolvedValue(undefined)}
        handleRelatedBatchMutation={vi.fn().mockResolvedValue(undefined)}
        language="en"
        loadingRelatedBatch={false}
        relatedBatchDetail={importBatchDetailFixture()}
        relatedBatchError={null}
        relatedImportBatch={importBatchOverviewFixture()}
        restoreBusy={false}
        restoreError={null}
        restoreKindLabel={(kind) => `kind:${kind}`}
        restoreNotice={null}
        restorePreview={null}
        setDetailTab={vi.fn()}
        t={t}
        {...overrides}
      />
    </MemoryRouter>
  )
}

function auditRunDetailFixture(
  overrides: Partial<AuditRunDetail> = {},
): AuditRunDetail {
  return {
    run: {
      id: 42,
      startedAt: '2026-04-25T10:00:00Z',
      finishedAt: '2026-04-25T10:05:00Z',
      status: 'completed',
      runType: 'import',
      trigger: 'manual',
      profileScope: ['chrome:Default'],
      manifestHash: 'sha256:manifest',
      profilesProcessed: 1,
      newVisits: 12,
      newUrls: 8,
      newDownloads: 1,
    },
    trigger: 'manual',
    timezone: 'UTC',
    dueOnly: false,
    profileScope: ['chrome:Default'],
    warnings: ['source warning'],
    errorMessage: null,
    stats: {},
    manifestPath: '/tmp/pathkeep/manifest.json',
    manifestHash: 'sha256:manifest',
    artifacts: [
      {
        kind: 'snapshot',
        path: '/tmp/pathkeep/snapshot.sqlite',
        checksum: 'sha256:snapshot',
        sizeBytes: 2048,
        createdAt: '2026-04-25T10:01:00Z',
        reason: 'before import',
      },
      {
        kind: 'manifest',
        path: '/tmp/pathkeep/manifest.json',
        checksum: 'sha256:manifest',
        sizeBytes: 512,
        createdAt: '2026-04-25T10:02:00Z',
        reason: 'audit manifest',
      },
    ],
    ...overrides,
  }
}

function importBatchOverviewFixture(
  overrides: Partial<ImportBatchOverview> = {},
): ImportBatchOverview {
  return {
    id: 77,
    sourceKind: 'browser-direct',
    sourcePath: '/Users/test/Chrome/History',
    profileId: 'chrome:Default',
    createdAt: '2026-04-25T09:55:00Z',
    importedAt: '2026-04-25T10:05:00Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 3,
    importedItems: 2,
    duplicateItems: 1,
    visibleItems: 2,
    auditPath: '/tmp/pathkeep/import-audit.json',
    gitCommit: 'abc123',
    ...overrides,
  }
}

function importBatchDetailFixture(
  overviewOverrides: Partial<ImportBatchOverview> = {},
): ImportBatchDetail {
  return {
    batch: importBatchOverviewFixture(overviewOverrides),
    previewEntries: [
      {
        sourcePath: '/Users/test/Chrome/History',
        url: 'https://example.com/docs',
        title: 'Example docs',
        visitedAt: '2026-04-24T12:00:00Z',
        sourceVisitId: 100,
        status: 'imported',
      },
    ],
    recognizedFiles: [],
    quarantinedFiles: [],
    notes: ['batch note'],
    detectedLocale: 'en-US',
    previewRangeStart: '2026-04-24T12:00:00Z',
    previewRangeEnd: '2026-04-24T12:00:00Z',
  }
}

function restorePreviewFixture(
  overrides: Partial<SnapshotRestorePreview> = {},
): SnapshotRestorePreview {
  return {
    snapshotPath: '/tmp/pathkeep/snapshot.sqlite',
    snapshotKind: 'snapshot',
    sourceRunId: 42,
    sourceProfileId: 'chrome:Default',
    sourceBrowserName: 'Google Chrome',
    createdAt: '2026-04-25T10:01:00Z',
    reason: 'before import',
    executeSupported: true,
    estimatedVisits: 12,
    estimatedUrls: 8,
    estimatedDownloads: 1,
    warnings: ['snapshot warning'],
    ...overrides,
  }
}
