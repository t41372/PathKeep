/**
 * This test file protects the Import route follow-through review panels.
 *
 * Why this file exists:
 * - Import review panels own the visible batch mutation and doctor/repair controls.
 * - Focused tests catch callback drift without exercising the full import wizard.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on visible review actions and route-owned callback payloads.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { ImportBatchDetail, ImportBatchOverview } from '../../lib/types'
import { ImportReviewPanels } from './review-panels'

const batch: ImportBatchOverview = {
  id: 7,
  sourceKind: 'google_takeout',
  sourcePath: '/tmp/takeout.zip',
  profileId: 'chrome:Default',
  createdAt: '2026-04-25T09:00:00.000Z',
  importedAt: '2026-04-25T09:01:00.000Z',
  revertedAt: null,
  status: 'active',
  candidateItems: 3,
  importedItems: 2,
  duplicateItems: 1,
  visibleItems: 2,
  auditPath: '/tmp/import-audit.json',
  gitCommit: null,
}

const batchDetail: ImportBatchDetail = {
  batch,
  previewEntries: [],
  recognizedFiles: [],
  quarantinedFiles: [],
  notes: [],
  detectedLocale: 'en',
  previewRangeStart: '2026-04-01T00:00:00.000Z',
  previewRangeEnd: '2026-04-02T00:00:00.000Z',
}

describe('ImportReviewPanels', () => {
  test('forwards batch, history, doctor, and repair actions', async () => {
    const user = userEvent.setup()
    const onBatchMutation = vi.fn().mockResolvedValue(undefined)
    const onHistoryExpandedChange = vi.fn()
    const onRepairHealth = vi.fn().mockResolvedValue(undefined)
    const onRunDoctor = vi.fn().mockResolvedValue(undefined)
    const onSelectBatch = vi.fn()

    render(
      <I18nProvider>
        <ImportReviewPanels
          activeBatchDetail={batchDetail}
          healthReport={{
            generatedAt: '2026-04-25T09:03:00.000Z',
            checks: [
              {
                name: 'manifest',
                ok: true,
                detail: 'Manifest is readable.',
              },
            ],
          }}
          historyExpanded
          language="en"
          loadingBatch={false}
          recentImportBatches={[batch]}
          repairNotice="Repair complete."
          selectedBatchId={null}
          supportCopyFeedback={null}
          onBatchMutation={onBatchMutation}
          onCopyPath={vi.fn()}
          onHistoryExpandedChange={onHistoryExpandedChange}
          onOpenPath={vi.fn()}
          onRepairHealth={onRepairHealth}
          onRunDoctor={onRunDoctor}
          onSelectBatch={onSelectBatch}
        />
      </I18nProvider>,
    )

    await user.click(screen.getByRole('button', { name: 'Undo import' }))
    await user.click(screen.getByRole('button', { name: /Batch #7/ }))
    await user.click(screen.getByRole('button', { name: 'Run health check' }))
    await user.click(screen.getByRole('button', { name: 'Repair' }))
    await user.click(screen.getByRole('button', { name: 'Hide history' }))

    expect(onBatchMutation).toHaveBeenCalledWith(batch, 'revert')
    expect(onSelectBatch).toHaveBeenCalledWith(7)
    expect(onRunDoctor).toHaveBeenCalledTimes(1)
    expect(onRepairHealth).toHaveBeenCalledTimes(1)
    expect(onHistoryExpandedChange).toHaveBeenCalledWith(false)
  })

  test('forwards restore actions for reverted batches', async () => {
    const user = userEvent.setup()
    const onBatchMutation = vi.fn().mockResolvedValue(undefined)
    const revertedBatch = {
      ...batch,
      status: 'reverted',
      revertedAt: '2026-04-25T09:04:00.000Z',
    }

    render(
      <I18nProvider>
        <ImportReviewPanels
          activeBatchDetail={{ ...batchDetail, batch: revertedBatch }}
          healthReport={null}
          historyExpanded={false}
          language="en"
          loadingBatch={false}
          recentImportBatches={[]}
          repairNotice={null}
          selectedBatchId={7}
          supportCopyFeedback={null}
          onBatchMutation={onBatchMutation}
          onCopyPath={vi.fn()}
          onHistoryExpandedChange={vi.fn()}
          onOpenPath={vi.fn()}
          onRepairHealth={vi.fn()}
          onRunDoctor={vi.fn()}
          onSelectBatch={vi.fn()}
        />
      </I18nProvider>,
    )

    await user.click(screen.getByRole('button', { name: 'Restore import' }))

    expect(onBatchMutation).toHaveBeenCalledWith(revertedBatch, 'restore')
  })

  test('renders empty import history when recent batches are not loaded', () => {
    render(
      <I18nProvider>
        <ImportReviewPanels
          activeBatchDetail={null}
          healthReport={null}
          historyExpanded
          language="en"
          loadingBatch={false}
          recentImportBatches={null}
          repairNotice={null}
          selectedBatchId={null}
          supportCopyFeedback={null}
          onBatchMutation={vi.fn()}
          onCopyPath={vi.fn()}
          onHistoryExpandedChange={vi.fn()}
          onOpenPath={vi.fn()}
          onRepairHealth={vi.fn()}
          onRunDoctor={vi.fn()}
          onSelectBatch={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('No imports yet.')).toBeVisible()
  })
})
