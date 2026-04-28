/**
 * This test file protects the render-only Settings retention section.
 *
 * Why this file exists:
 * - Retention pruning is destructive, so selection, refresh, and prune controls need direct regression coverage.
 * - Focused render tests keep the route hook responsible for state while still proving the section wiring.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on PME review visibility and handler calls instead of styling details.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { SettingsSectionNavItem } from './section-nav-items'
import {
  RetentionSection,
  type RetentionSectionState,
} from './retention-section'

const navItem: SettingsSectionNavItem = {
  id: 'settings-retention',
  icon: 'delete_sweep',
  key: 'retention',
  label: 'Retention',
}

const snapshotWarning =
  'Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.'
const exportWarning =
  'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.'

const state = (
  patch: Partial<RetentionSectionState> = {},
): RetentionSectionState => ({
  action: null,
  error: null,
  needsUnlock: false,
  preview: {
    buckets: [
      {
        id: 'snapshots',
        bytes: 1024,
        itemCount: 2,
        paths: ['/tmp/pathkeep/snapshots'],
      },
      {
        id: 'quarantine',
        bytes: 0,
        itemCount: 0,
        paths: [],
      },
    ],
    warnings: [snapshotWarning],
  },
  result: {
    runId: 42,
    deletedBytes: 512,
    deletedFiles: 3,
    buckets: [],
    warnings: [],
  },
  selectedBytes: 1024,
  selection: { snapshots: true, quarantine: false },
  onBucketSelectionChange: vi.fn(),
  onPrune: vi.fn(),
  onRefresh: vi.fn(),
  ...patch,
})

function renderSection(sectionState: RetentionSectionState) {
  render(
    <I18nProvider>
      <MemoryRouter>
        <RetentionSection navItem={navItem} state={sectionState} />
      </MemoryRouter>
    </I18nProvider>,
  )
}

describe('RetentionSection', () => {
  test('renders preview buckets and forwards selection, refresh, and prune actions', async () => {
    const user = userEvent.setup()
    const sectionState = state()
    renderSection(sectionState)

    expect(screen.getByText(/Saved snapshots/)).toBeVisible()
    expect(
      screen.getByText(
        'Snapshot pruning removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.',
      ),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open prune review' }),
    ).toHaveAttribute('href', '/audit?run=42')

    await user.click(screen.getAllByRole('checkbox')[1])
    await user.click(screen.getByRole('button', { name: 'Refresh preview' }))
    await user.click(screen.getByRole('button', { name: 'Prune selected' }))

    expect(sectionState.onBucketSelectionChange).toHaveBeenCalledWith(
      'quarantine',
      true,
    )
    expect(sectionState.onRefresh).toHaveBeenCalledTimes(1)
    expect(sectionState.onPrune).toHaveBeenCalledTimes(1)
  })

  test('renders unlock, zero-byte, no-audit, and error branches', () => {
    renderSection(
      state({
        error: 'Prune failed',
        needsUnlock: true,
        preview: {
          buckets: [
            {
              id: 'exports',
              bytes: 0,
              itemCount: 0,
              paths: [],
            },
          ],
          warnings: [exportWarning],
        },
        result: {
          runId: null,
          deletedBytes: 0,
          deletedFiles: 0,
          buckets: [],
          warnings: [],
        },
        selectedBytes: 0,
        selection: { exports: true },
      }),
    )

    expect(screen.getByRole('link', { name: 'Security' })).toHaveAttribute(
      'href',
      '/security',
    )
    expect(
      screen.getByText(
        'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.',
      ),
    ).toBeVisible()
    expect(screen.queryByRole('link', { name: 'Open prune review' })).toBeNull()
    expect(screen.getByRole('alert')).toHaveTextContent('Prune failed')
  })
})
