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
  visitedLabel: 'Visited',
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
  notesSaveError: 'Not saved · retry',
  notesCharSingular: '1 char',
  notesCharPlural: '{count} chars',
  tagInputPlaceholder: '+ add tag',
  tagRemoveAriaLabel: 'Remove tag {tag}',
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
    // Default summary shows the First/Last pair, not the single-visit field.
    expect(screen.getByText('First visit')).toBeVisible()
    expect(screen.getByText('Last visit')).toBeVisible()
    expect(screen.getByText('2025-11-04 09:17')).toBeVisible()
    expect(screen.getByText('2026-05-16 21:42')).toBeVisible()
    expect(screen.queryByText('Visited')).toBeNull()
  })

  test('collapses to a single "Visited" field when only the opened visit is known', () => {
    // The Explorer mount has just one visit row, so it sets `visitedAt`
    // instead of fabricating identical First/Last dates. The panel must then
    // show one honest field and drop the First/Last pair entirely.
    render(
      <PaperDetailPanel
        entry={makeEntry({
          visitedAt: '2026-05-17 10:30',
          firstVisitAt: undefined,
          lastVisitAt: undefined,
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="panel-visited"
      />,
    )

    expect(screen.getByText('Visited')).toBeVisible()
    expect(screen.getByText('2026-05-17 10:30')).toBeVisible()
    // The misleading First/Last pair must not render in single-visit mode.
    expect(screen.queryByText('First visit')).toBeNull()
    expect(screen.queryByText('Last visit')).toBeNull()
  })

  test('shows the single "Visited" field even when its formatted value is empty', () => {
    // `visitedAt: ''` is a present-but-empty value (e.g. a degraded
    // timestamp the caller already collapsed to a dash). Presence — not
    // truthiness — selects the single-visit layout, so the First/Last pair
    // must still be suppressed.
    render(
      <PaperDetailPanel
        entry={makeEntry({
          visitedAt: '',
          firstVisitAt: '2025-11-04 09:17',
          lastVisitAt: '2026-05-16 21:42',
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="panel-visited-empty"
      />,
    )

    expect(screen.getByText('Visited')).toBeVisible()
    expect(screen.queryByText('First visit')).toBeNull()
    expect(screen.queryByText('Last visit')).toBeNull()
  })

  test('surfaces a not-saved alert instead of the saved hint when a write fails', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes="kept this for later"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        annotationError="notes: archive is locked"
        copy={COPY}
        testId="panel-save-error"
      />,
    )

    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent('Not saved · retry')
    // The misleading "Saved · local" hint must not appear when the write failed.
    expect(screen.queryByText('Saved · local')).toBeNull()
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

  test('pressing Enter on a whitespace-only input does not commit a tag', () => {
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
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    // handleAddTag at line 221 short-circuits with `if (!candidate) return`
    // because trim() yields the empty string. No tag is committed.
    expect(onUpdateTags).not.toHaveBeenCalled()
  })

  test('Backspace on an empty input with no tags is a no-op', () => {
    const onUpdateTags = vi.fn()
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={onUpdateTags}
        copy={COPY}
      />,
    )

    const input = screen.getByTestId('paper-detail-tag-input')
    fireEvent.change(input, { target: { value: '' } })
    fireEvent.keyDown(input, { key: 'Backspace' })
    // Branch at line 244-247 of paper-detail-panel.tsx requires tags.length > 0;
    // with no tags the handler does not fire onUpdateTags.
    expect(onUpdateTags).not.toHaveBeenCalled()
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

  test('renders the singular "char" copy when notes is exactly one character', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes="x"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.getByText('1 char')).toBeVisible()
  })

  test('falls back to URL for the title heading when title is empty', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry({
          title: '',
          url: 'https://example.com/no-title',
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )
    // Title heading falls back to the URL (sanitized).
    const heading = screen.getByRole('heading', { level: 2 })
    expect(heading.textContent).toContain('example.com/no-title')
  })

  test('debounced save clears a pending timer when a new keystroke arrives', () => {
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
    // First keystroke schedules a save at +400ms.
    fireEvent.change(textarea, { target: { value: 'first draft' } })
    act(() => {
      vi.advanceTimersByTime(200)
    })
    // Second keystroke arrives within the debounce — must clear the prior timer.
    fireEvent.change(textarea, { target: { value: 'final copy' } })
    act(() => {
      vi.advanceTimersByTime(200)
    })
    // First half of the window doesn't fire yet — proves the first timer was cleared.
    expect(onUpdateNotes).not.toHaveBeenCalled()
    act(() => {
      vi.advanceTimersByTime(200)
    })
    expect(onUpdateNotes).toHaveBeenCalledTimes(1)
    expect(onUpdateNotes).toHaveBeenCalledWith('final copy')
  })

  test('unmount flushes a pending debounced save before tearing down', () => {
    // The panel has no explicit save button, so the user trusts auto-save.
    // Closing / navigating away should commit pending text, not drop it.
    const onUpdateNotes = vi.fn()
    const { unmount } = render(
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
    fireEvent.change(screen.getByTestId('paper-detail-notes'), {
      target: { value: 'unsent thoughts' },
    })
    unmount()
    expect(onUpdateNotes).toHaveBeenCalledTimes(1)
    expect(onUpdateNotes).toHaveBeenCalledWith('unsent thoughts')
    // The timer should have been cancelled — advancing time afterwards
    // must not double-fire the save.
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(onUpdateNotes).toHaveBeenCalledTimes(1)
  })

  test('entry switch flushes the pending edit to the previous record', () => {
    // A user types a note for record A, then clicks on record B before the
    // debounce fires. The pending text belongs to A — the swap must not
    // lose it (would be silent data loss) or carry it onto B (would write
    // A's text to B's url).
    const onUpdateNotesA = vi.fn()
    const onUpdateNotesB = vi.fn()
    const { rerender } = render(
      <PaperDetailPanel
        entry={makeEntry({ id: 1 })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotesA}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={400}
      />,
    )
    fireEvent.change(screen.getByTestId('paper-detail-notes'), {
      target: { value: 'thought about record A' },
    })
    expect(onUpdateNotesA).not.toHaveBeenCalled()
    rerender(
      <PaperDetailPanel
        entry={makeEntry({ id: 2 })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotesB}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={400}
      />,
    )
    expect(onUpdateNotesA).toHaveBeenCalledTimes(1)
    expect(onUpdateNotesA).toHaveBeenCalledWith('thought about record A')
    expect(onUpdateNotesB).not.toHaveBeenCalled()
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

  test('external notes refreshes replace the textarea when no local edit is pending', () => {
    const entry = makeEntry()
    const { rerender } = render(
      <PaperDetailPanel
        entry={entry}
        notes="cached note"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    rerender(
      <PaperDetailPanel
        entry={entry}
        notes="backend refresh"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-detail-notes').value,
    ).toBe('backend refresh')
  })

  test('pending local notes are not overwritten by a stale external refresh', () => {
    const entry = makeEntry()
    const onUpdateNotes = vi.fn()
    const { rerender } = render(
      <PaperDetailPanel
        entry={entry}
        notes="cached note"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotes}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={400}
      />,
    )

    fireEvent.change(screen.getByTestId('paper-detail-notes'), {
      target: { value: 'local draft' },
    })
    rerender(
      <PaperDetailPanel
        entry={entry}
        notes="stale backend refresh"
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={onUpdateNotes}
        onUpdateTags={() => {}}
        copy={COPY}
        notesDebounceMs={400}
      />,
    )

    expect(
      screen.getByTestId<HTMLTextAreaElement>('paper-detail-notes').value,
    ).toBe('local draft')
    act(() => {
      vi.advanceTimersByTime(400)
    })
    expect(onUpdateNotes).toHaveBeenCalledWith('local draft')
  })

  test('look-further section is suppressed when route handlers are not wired', () => {
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

    expect(screen.queryByText('Look further')).toBeNull()
    expect(screen.queryByText('Page-level insights')).toBeNull()
  })

  test('look-further rows remain clickable when count hints are omitted', () => {
    const onOpenDomain = vi.fn()
    const entry = makeEntry()
    render(
      <PaperDetailPanel
        entry={entry}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        onOpenDomain={onOpenDomain}
        copy={COPY}
      />,
    )

    const row = screen.getByRole('button', { name: 'All of github.com' })
    expect(row).toHaveTextContent('All of github.com')
    fireEvent.click(row)
    expect(onOpenDomain).toHaveBeenCalledWith(entry)
  })

  test('renders the favicon chip and og:image hero when both are provided', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry({
          faviconDataUrl: 'data:image/png;base64,FAVICON',
          ogImageDataUrl: 'data:image/png;base64,OGHERO',
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="paper-detail"
      />,
    )
    const favicon = screen.getByTestId<HTMLImageElement>('paper-detail-favicon')
    expect(favicon.src).toContain('FAVICON')
    const hero = screen.getByTestId<HTMLImageElement>('paper-detail-og-hero')
    expect(hero.src).toContain('OGHERO')
  })

  test('uses default media test ids when the caller does not supply a panel test id', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry({
          faviconDataUrl: 'data:image/png;base64,FAVICON',
          ogImageDataUrl: 'data:image/png;base64,OGHERO',
        })}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
      />,
    )

    expect(
      screen.getByTestId<HTMLImageElement>('paper-detail-favicon').src,
    ).toContain('FAVICON')
    expect(
      screen.getByTestId<HTMLImageElement>('paper-detail-og-hero').src,
    ).toContain('OGHERO')
  })

  test('omits the favicon chip and og:image hero when neither is provided', () => {
    render(
      <PaperDetailPanel
        entry={makeEntry()}
        notes=""
        tags={[]}
        onClose={() => {}}
        onUpdateNotes={() => {}}
        onUpdateTags={() => {}}
        copy={COPY}
        testId="paper-detail"
      />,
    )
    expect(screen.queryByTestId('paper-detail-favicon')).toBeNull()
    expect(screen.queryByTestId('paper-detail-og-hero')).toBeNull()
  })
})
