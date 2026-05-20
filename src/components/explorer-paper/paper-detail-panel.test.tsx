/**
 * Tests for PaperDetailPanel — the right-edge slide-over surfacing a single
 * archived visit.
 *
 * The panel concentrates a lot of behaviour: action buttons, debounced
 * notes, tag chip editor, look-further routing, Escape close, and visit-
 * history sparkline. These tests pin each contract so the route can plug
 * the panel in without re-testing layout.
 */

import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  PaperDetailPanel,
  type PaperDetailPanelCopy,
  type PaperDetailPanelEntry,
} from './paper-detail-panel'

const COPY: PaperDetailPanelCopy = {
  recordEyebrow: 'Record',
  closeLabel: 'Close',
  openAction: 'Open',
  copyAction: 'Copy URL',
  refindAction: 'Refind…',
  exportAction: 'Export',
  provenanceHeading: 'Provenance',
  notesHeading: 'Your notes',
  tagsHeading: 'Tags',
  lookFurtherHeading: 'Look further',
  firstVisitLabel: 'First visit',
  lastVisitLabel: 'Last visit',
  totalVisitsLabel: 'Total visits',
  typedCountLabel: 'Typed directly',
  recentVisitsLabel: 'Recent visits',
  sourceLabel: 'Source',
  transitionLabel: 'Transition',
  capturedInRunLabel: 'Captured in run',
  titleHistoryLabel: 'Title history',
  notesPlaceholder: 'Why did this matter?',
  notesEmpty: 'Empty',
  notesSavedLocally: 'Saved · local',
  tagInputPlaceholder: '+ add tag',
  pageLevelInsights: 'Page-level insights',
  allOfDomain: 'All of {domain}',
  threadLabel: 'Thread',
  sessionLabel: 'Session',
  visitCountSuffix: '{count}×',
}

function makeEntry(
  overrides: Partial<PaperDetailPanelEntry> = {},
): PaperDetailPanelEntry {
  return {
    id: 1,
    title: 'tokio-rs/tokio',
    url: 'https://github.com/tokio-rs/tokio',
    domain: 'github.com',
    firstVisitAt: '2025-11-04 09:17',
    lastVisitAt: '2026-05-16 21:42',
    visitCount: 47,
    typedCount: 12,
    source: 'Chrome / Default',
    transition: 'link',
    capturedIn: '#1847 · 2026-05-16 18:30',
    visitHistory: [
      { date: '2026-05-16', count: 5 },
      { date: '2026-05-12', count: 3 },
      { date: '2026-04-22', count: 2 },
    ],
    titleVersions: [
      { date: '2026-03-12', title: 'tokio-rs/tokio: Async runtime' },
      {
        date: '2024-11-08',
        title: 'tokio-rs/tokio: A runtime for reliable async apps',
      },
    ],
    ...overrides,
  }
}

describe('PaperDetailPanel', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  test('returns null when entry is missing', () => {
    const { container } = render(
      <PaperDetailPanel
        entry={null}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )
    expect(container).toBeEmptyDOMElement()
  })

  test('renders title, URL, visit summary, and provenance', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="panel"
      />,
    )

    expect(screen.getByText('tokio-rs/tokio')).toBeVisible()
    expect(screen.getByText('github.com/tokio-rs/tokio')).toBeVisible()
    expect(screen.getByText('47')).toBeVisible() // visit count
    expect(screen.getByText('12')).toBeVisible() // typed count
    expect(screen.getByText('Chrome / Default')).toBeVisible()
    expect(screen.getByText('link')).toBeVisible()
    expect(screen.getByText('#1847 · 2026-05-16 18:30')).toBeVisible()
  })

  test('renders the visit-history sparkline rows', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="panel-history"
      />,
    )

    const history = screen.getByTestId('paper-detail-visit-history')
    expect(within(history).getByText('2026-05-16')).toBeVisible()
    expect(within(history).getByText('5×')).toBeVisible()
    expect(within(history).getByText('2×')).toBeVisible()
  })

  test('falls back gracefully when visit-history and title-versions are absent', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry({
          visitHistory: undefined,
          titleVersions: undefined,
          firstVisitAt: undefined,
          lastVisitAt: undefined,
          visitCount: undefined,
          typedCount: undefined,
          source: undefined,
          transition: undefined,
          capturedIn: undefined,
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    expect(screen.queryByTestId('paper-detail-visit-history')).toBeNull()
    // Total-visits cell collapses to —
    const dashes = screen.getAllByText('—')
    expect(dashes.length).toBeGreaterThanOrEqual(2)
  })

  test('action buttons fire their handlers with the entry', () => {
    const onOpen = vi.fn()
    const onCopyUrl = vi.fn()
    const onRefind = vi.fn()
    const onExport = vi.fn()
    const entry = makeEntry()

    render(
      <PaperDetailPanel
        entry={entry}
        notes=""
        tags={[]}
        onClose={() => {}}
        onOpen={onOpen}
        onCopyUrl={onCopyUrl}
        onRefind={onRefind}
        onExport={onExport}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Open' }))
    fireEvent.click(screen.getByRole('button', { name: 'Copy URL' }))
    fireEvent.click(screen.getByRole('button', { name: 'Refind…' }))
    fireEvent.click(screen.getByRole('button', { name: 'Export' }))

    expect(onOpen).toHaveBeenCalledWith(entry)
    expect(onCopyUrl).toHaveBeenCalledWith(entry)
    expect(onRefind).toHaveBeenCalledWith(entry)
    expect(onExport).toHaveBeenCalledWith(entry)
  })

  test('action buttons without handlers are disabled', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    expect(
      screen.getByRole<HTMLButtonElement>('button', { name: 'Open' }).disabled,
    ).toBe(true)
  })

  test('close button + backdrop + Escape all dismiss the panel', () => {
    const onClose = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={onClose}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="panel-close"
      />,
    )

    // Close icon button
    fireEvent.click(screen.getAllByRole('button', { name: 'Close' })[0])
    // Backdrop (also has aria-label "Close")
    fireEvent.click(screen.getAllByRole('button', { name: 'Close' })[1])
    // Escape
    fireEvent.keyDown(window, { key: 'Escape' })

    expect(onClose).toHaveBeenCalledTimes(3)
  })

  test('notes textarea debounces writes and saves after the timeout', () => {
    const onUpdateNotes = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotes}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={400}
      />,
    )

    const textarea = screen.getByTestId('paper-detail-notes')
    fireEvent.change(textarea, {
      target: { value: 'Why I kept reading this' },
    })
    expect(onUpdateNotes).not.toHaveBeenCalled()

    act(() => {
      vi.advanceTimersByTime(400)
    })
    expect(onUpdateNotes).toHaveBeenCalledWith('Why I kept reading this')
  })

  test('notesDebounceMs of zero writes synchronously', () => {
    const onUpdateNotes = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotes}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={0}
      />,
    )

    fireEvent.change(screen.getByTestId('paper-detail-notes'), {
      target: { value: 'instant save' },
    })
    expect(onUpdateNotes).toHaveBeenCalledWith('instant save')
  })

  test('Enter key adds a tag and Backspace on empty input removes the last tag', () => {
    const onUpdateTags = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={['rust', 'async']}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={onUpdateTags}
        copy={COPY}
      />,
    )

    const input = screen.getByTestId('paper-detail-tag-input')
    fireEvent.change(input, { target: { value: 'tokio' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onUpdateTags).toHaveBeenLastCalledWith(['rust', 'async', 'tokio'])

    // Backspace on empty input pops the last tag.
    fireEvent.change(input, { target: { value: '' } })
    fireEvent.keyDown(input, { key: 'Backspace' })
    expect(onUpdateTags).toHaveBeenLastCalledWith(['rust'])
  })

  test('duplicate tag is rejected silently and clears the input', () => {
    const onUpdateTags = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={['rust']}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={onUpdateTags}
        copy={COPY}
      />,
    )

    const input = screen.getByTestId('paper-detail-tag-input')
    fireEvent.change(input, { target: { value: 'RUST' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onUpdateTags).not.toHaveBeenCalled()
    expect((input as HTMLInputElement).value).toBe('')
  })

  test('chip "✕" buttons remove their tag', () => {
    const onUpdateTags = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={['rust', 'async']}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={onUpdateTags}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Remove tag rust' }))
    expect(onUpdateTags).toHaveBeenCalledWith(['async'])
  })

  test('look-further rows fire their handlers with the entry', () => {
    const onOpenInsights = vi.fn()
    const onOpenDomain = vi.fn()
    const onOpenThread = vi.fn()
    const onOpenSession = vi.fn()
    const entry = makeEntry()

    render(
      <PaperDetailPanel
        entry={entry}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        onOpenInsights={onOpenInsights}
        onOpenDomain={onOpenDomain}
        onOpenThread={onOpenThread}
        onOpenSession={onOpenSession}
        lookFurtherCounts={{
          visitsLabel: '47 visits',
          domainPagesLabel: '2,341 pages',
          threadLabel: '89 pages · active',
          sessionLabel: '14 pages',
        }}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: /Page-level insights/ }))
    fireEvent.click(screen.getByRole('button', { name: /All of github.com/ }))
    fireEvent.click(screen.getByRole('button', { name: /Thread/ }))
    fireEvent.click(screen.getByRole('button', { name: /Session/ }))

    expect(onOpenInsights).toHaveBeenCalledWith(entry)
    expect(onOpenDomain).toHaveBeenCalledWith(entry)
    expect(onOpenThread).toHaveBeenCalledWith(entry)
    expect(onOpenSession).toHaveBeenCalledWith(entry)
  })

  test('resets the local notes buffer when entry id changes', () => {
    const { rerender } = render(
      <PaperDetailPanel
        entry={makeEntry({ id: 1 })}
        notes="first record notes"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )
    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-detail-notes').value,
    ).toBe('first record notes')

    rerender(
      <PaperDetailPanel
        entry={makeEntry({ id: 2 })}
        notes="second record notes"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-detail-notes').value,
    ).toBe('second record notes')
  })
})
