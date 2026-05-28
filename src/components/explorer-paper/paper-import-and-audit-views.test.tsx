/**
 * Tests for the Import + Audit view compositions.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperAuditView,
  PaperImportView,
  type PaperAuditChainEntry,
  type PaperAuditViewCopy,
  type PaperImportMethod,
} from './index'

const METHODS: PaperImportMethod[] = [
  {
    id: 'takeout',
    title: 'Google Takeout',
    description: 'Import an exported Google archive.',
    hint: 'Recommended',
  },
  {
    id: 'browser',
    title: 'Browser direct',
    description: 'Read from a profile DB.',
  },
  {
    id: 'csv',
    title: 'CSV / JSON',
    description: 'From another tool.',
  },
]

describe('PaperImportView', () => {
  test('renders intro, methods, stepper, and body slot', () => {
    render(
      <PaperImportView
        intro="Bring history into the archive."
        methods={METHODS}
        activeMethodId="takeout"
        onSelectMethod={() => {}}
        steps={['Upload', 'Scan', 'Preview', 'Confirm', 'Import']}
        currentStep={2}
        bodySlot={<div data-testid="import-body">Preview body</div>}
        testId="import-view"
      />,
    )

    expect(screen.getByText('Bring history into the archive.')).toBeVisible()
    expect(screen.getByTestId('paper-import-methods')).toBeVisible()
    expect(screen.getByTestId('paper-import-step-2').dataset.step).toBe(
      'active',
    )
    expect(screen.getByTestId('import-body')).toBeVisible()
  })

  test('selecting a method routes onSelectMethod with the id', () => {
    const onSelectMethod = vi.fn()
    render(
      <PaperImportView
        intro="…"
        methods={METHODS}
        activeMethodId="takeout"
        onSelectMethod={onSelectMethod}
        steps={['Upload', 'Preview']}
        currentStep={0}
        bodySlot={null}
        testId="import-select"
      />,
    )

    fireEvent.click(screen.getByTestId('paper-import-method-csv'))
    expect(onSelectMethod).toHaveBeenCalledWith('csv')
  })

  test('marks the active method card via data-active', () => {
    render(
      <PaperImportView
        intro="…"
        methods={METHODS}
        activeMethodId="browser"
        steps={['Upload']}
        currentStep={0}
        bodySlot={null}
        testId="import-active"
      />,
    )

    expect(
      screen.getByTestId('paper-import-method-browser').dataset.active,
    ).toBe('true')
    expect(
      screen.getByTestId('paper-import-method-takeout').dataset.active,
    ).toBeUndefined()
  })
})

const CHAIN: PaperAuditChainEntry[] = [
  {
    id: '#1847',
    hash: '0a4c…ef82',
    type: 'BACKUP',
    when: '2h ago',
    current: true,
  },
  { id: '#1846', hash: '8b71…d3a9', type: 'BACKUP', when: '2h ago' },
  { id: '#1845', hash: '4e29…91c7', type: 'IMPORT', when: '1d ago' },
]

const AUDIT_COPY: PaperAuditViewCopy = {
  manifestTitle: 'Manifest chain',
  manifestBadge: 'Verify integrity →',
  manifestCallout: 'Chain verified. All entries hash-link correctly.',
  earlierBlockLabel: 'earlier ↺',
  recentRunsTitle: 'Recent runs',
  recentRunsBadge: 'Last 7 of 1,847',
  storageTitle: 'Storage breakdown',
  storageBadge: '12.4 GB total',
  exportTitle: 'Take it with you',
  snapshotsTitle: 'Snapshots',
  snapshotsBadge: '4 kept',
  footer: 'Local. Plaintext SQLite by default.',
}

describe('PaperAuditView', () => {
  test('renders the chain blocks and callout', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={<div data-testid="audit-runs">runs</div>}
        storageBreakdownSlot={<div data-testid="audit-storage">storage</div>}
        exportPanelSlot={<div data-testid="audit-export">export</div>}
        snapshotsSlot={<div data-testid="audit-snapshots">snapshots</div>}
        copy={AUDIT_COPY}
        testId="audit-view"
      />,
    )

    const chain = screen.getByTestId('paper-audit-chain')
    expect(within(chain).getByText('#1847')).toBeVisible()
    expect(within(chain).getByText('#1845')).toBeVisible()
    expect(within(chain).getByText('earlier ↺')).toBeVisible()
    expect(
      screen.getByText(/Chain verified\. All entries hash-link/),
    ).toBeVisible()
  })

  test('marks the current chain block', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={AUDIT_COPY}
      />,
    )

    expect(screen.getByTestId('paper-chain-block-#1847').dataset.current).toBe(
      'true',
    )
    expect(
      screen.getByTestId('paper-chain-block-#1846').dataset.current,
    ).toBeUndefined()
  })

  test('clicking a chain block forwards the id', () => {
    const onSelect = vi.fn()
    render(
      <PaperAuditView
        chain={CHAIN}
        onSelectChainBlock={onSelect}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={AUDIT_COPY}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-chain-block-#1846'))
    expect(onSelect).toHaveBeenCalledWith('#1846')
  })

  test('mounts the four data slots into their cards', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={<div data-testid="audit-runs">runs</div>}
        storageBreakdownSlot={<div data-testid="audit-storage">storage</div>}
        exportPanelSlot={<div data-testid="audit-export">export</div>}
        snapshotsSlot={<div data-testid="audit-snapshots">snapshots</div>}
        copy={AUDIT_COPY}
      />,
    )

    expect(
      within(screen.getByTestId('paper-audit-runs')).getByText('runs'),
    ).toBeVisible()
    expect(
      within(screen.getByTestId('paper-audit-storage')).getByText('storage'),
    ).toBeVisible()
    expect(
      within(screen.getByTestId('paper-audit-export')).getByText('export'),
    ).toBeVisible()
    expect(
      within(screen.getByTestId('paper-audit-snapshots')).getByText(
        'snapshots',
      ),
    ).toBeVisible()
  })

  test('renders the quiet footer when copy provides one', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={AUDIT_COPY}
      />,
    )

    expect(
      screen.getByText('Local. Plaintext SQLite by default.'),
    ).toBeVisible()
  })

  test('omits the callout / earlier marker / footer when copy does not include them', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={{
          ...AUDIT_COPY,
          manifestCallout: undefined,
          earlierBlockLabel: undefined,
          footer: undefined,
        }}
      />,
    )

    expect(screen.queryByTestId('paper-audit-callout')).toBeNull()
    expect(screen.queryByTestId('paper-audit-footer')).toBeNull()
    expect(screen.queryByText('earlier ↺')).toBeNull()
  })

  test('omits the recentRuns / storage / snapshots badges when copy leaves them undefined', () => {
    render(
      <PaperAuditView
        chain={CHAIN}
        recentRunsSlot={null}
        storageBreakdownSlot={null}
        exportPanelSlot={null}
        snapshotsSlot={null}
        copy={{
          ...AUDIT_COPY,
          manifestBadge: undefined,
          recentRunsBadge: undefined,
          storageBadge: undefined,
          snapshotsBadge: undefined,
        }}
      />,
    )
    // Each PaperCardHeader's `right` slot conditional (`copy.X ?
    // <PaperCardBadge>X</PaperCardBadge> : undefined`) takes its falsy
    // branch — covers lines 147-148 / 161-162 / equivalent snapshot
    // branch of paper-audit-view.tsx.
    expect(screen.queryByText('Last 7 of 1,847')).toBeNull()
    expect(screen.queryByText('12.4 GB total')).toBeNull()
    expect(screen.queryByText('4 kept')).toBeNull()
  })
})
