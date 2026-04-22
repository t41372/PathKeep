/**
 * @file import-batch-review.test.tsx
 * @description Protects the shared import-batch review body used by Import and Audit follow-through surfaces.
 * @module components/review
 *
 * ## Responsibilities
 * - Verify the shared manifest grid, preview-entry list, and optional audit-path action row.
 * - Keep the cross-route review body stable as Import and Audit continue to compose their own actions around it.
 *
 * ## Not responsible for
 * - Re-testing ImportPage or Audit route loading/error state.
 * - Verifying route-specific revert, restore, or navigation handlers.
 *
 * ## Dependencies
 * - Depends on the shared review component export surface.
 * - Uses lightweight callback spies instead of route harnesses because the component is render-only.
 *
 * ## Performance notes
 * - Focused render test so this shared primitive can be validated without booting route-level data loaders.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import type { ImportBatchDetail } from '../../lib/types'
import { ImportBatchReview } from '.'

const batchDetail: ImportBatchDetail = {
  batch: {
    id: 7,
    sourceKind: 'takeout',
    sourcePath: '/tmp/takeout',
    profileId: 'takeout::browser-history',
    createdAt: '2026-04-21T10:00:00.000Z',
    importedAt: '2026-04-21T10:01:00.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 12,
    importedItems: 10,
    duplicateItems: 2,
    visibleItems: 9,
    auditPath: '/tmp/import-audit.json',
    gitCommit: null,
  },
  previewEntries: [
    {
      sourcePath: '/tmp/takeout/BrowserHistory.json',
      sourceVisitId: 1,
      status: 'imported',
      title: 'Shared review entry',
      url: 'https://example.com/shared-review',
      visitedAt: '2026-04-21T10:00:00.000Z',
    },
  ],
  recognizedFiles: [],
  quarantinedFiles: [],
  notes: [],
}

describe('ImportBatchReview', () => {
  test('renders manifest metrics, preview rows, and audit-path actions', async () => {
    const user = userEvent.setup()
    const onCopy = vi.fn()
    const onOpenPath = vi.fn()

    render(
      <ImportBatchReview
        auditPathActions={{
          copyFeedback: { key: 'import:audit:7', tone: 'success' },
          copyKey: 'import:audit:7',
          copyLabel: 'Copy',
          errorMessage: 'Copy failed',
          label: 'Manifest path',
          onCopy,
          onOpenPath,
          openPathLabel: 'Open',
          successMessage: 'Copied',
        }}
        batchDetail={batchDetail}
        language="en"
        metricLabels={{
          candidateRows: 'Candidate rows',
          duplicateRows: 'Duplicate rows',
          importedRows: 'Imported rows',
          visibleRows: 'Visible rows',
        }}
        noPreviewEntriesLabel="No preview rows"
        previewStatusLabel={(status) => status.toUpperCase()}
      />,
    )

    expect(screen.getByText('Candidate rows')).toBeVisible()
    expect(screen.getByText('12')).toBeVisible()
    expect(screen.getByText('Shared review entry')).toBeVisible()
    expect(screen.getByText('IMPORTED')).toBeVisible()
    expect(screen.getByText('Manifest path')).toBeVisible()
    expect(screen.getByText('Copied')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Open' }))
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/import-audit.json')

    await user.click(screen.getByRole('button', { name: 'Copy' }))
    expect(onCopy).toHaveBeenCalledWith(
      'import:audit:7',
      '/tmp/import-audit.json',
    )
  })

  test('renders the empty-preview copy when the batch has no preview rows', () => {
    render(
      <ImportBatchReview
        batchDetail={{
          ...batchDetail,
          batch: {
            ...batchDetail.batch,
            auditPath: null,
          },
          previewEntries: [],
        }}
        language="en"
        metricLabels={{
          candidateRows: 'Candidate rows',
          duplicateRows: 'Duplicate rows',
          importedRows: 'Imported rows',
          visibleRows: 'Visible rows',
        }}
        noPreviewEntriesLabel="No preview rows"
      />,
    )

    expect(screen.getByText('No preview rows')).toBeVisible()
    expect(screen.queryByText('Manifest path')).not.toBeInTheDocument()
  })
})
