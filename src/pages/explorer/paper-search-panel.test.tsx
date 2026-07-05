import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type { HistoryEntry } from '@/lib/types/archive'
import { PaperSearchPanel } from './paper-search-panel'

function explorerT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

function makeEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 1,
    profileId: 'chrome:Default',
    url: 'https://example.com/',
    title: 'Example',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-17T10:30:00',
    visitTime: 1747488600,
    durationMs: null,
    transition: 0,
    sourceVisitId: 0,
    appId: null,
    ...over,
  }
}

describe('PaperSearchPanel', () => {
  test('renders the search hero + results layout', () => {
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[makeEntry({ id: 1, title: 'rustlang docs' })]}
        totalResults={1}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    expect(screen.getByTestId('explorer-paper-search-view')).toBeInTheDocument()
    expect(screen.getByTestId('paper-search-results')).toBeInTheDocument()
  })

  test('clicking the Search button submits the current query', () => {
    const onSubmit = vi.fn()
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={onSubmit}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-submit'))
    expect(onSubmit).toHaveBeenCalledWith('rust')
  })

  test('forwards isSearching / searchSubmitDisabled to the Search button', () => {
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        isSearching
        searchSubmitDisabled
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    const button = screen.getByTestId<HTMLButtonElement>('paper-search-submit')
    expect(button.disabled).toBe(true)
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(
      screen.getByTestId('paper-search-submit-spinner'),
    ).toBeInTheDocument()
  })

  test('forwards staleResultsMode so the hero renders the stale banner', () => {
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[makeEntry({ id: 1, title: 'rust' })]}
        totalResults={1}
        language="en"
        explorerT={explorerT}
        staleResultsMode="smart"
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // The banner copy comes from the explorerT stub key — its presence proves
    // the panel threaded staleResultsMode → hero → banner.
    expect(screen.getByTestId('paper-search-stale-banner')).toBeInTheDocument()
  })

  test('switching to regex mode forwards { mode: keyword, regexMode: true }', () => {
    const onModeChange = vi.fn()
    render(
      <PaperSearchPanel
        query="x"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // The mode toggle exposes three tabs; the second (Regex) lights up
    const regexTab = screen
      .getByText('paperSearchView.heroModeRegex')
      .closest('[role="tab"]')
    if (!(regexTab instanceof HTMLElement)) throw new Error('regex tab missing')
    fireEvent.click(regexTab)
    expect(onModeChange).toHaveBeenCalledWith({
      mode: 'keyword',
      regexMode: true,
    })
  })

  test('switching to Smart mode forwards { mode: hybrid, regexMode: false }', () => {
    const onModeChange = vi.fn()
    render(
      <PaperSearchPanel
        query="x"
        mode="keyword"
        regexMode={true}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    const smartTab = screen
      .getByText('paperSearchView.heroModeSmart')
      .closest('[role="tab"]')
    if (!(smartTab instanceof HTMLElement)) throw new Error('smart tab missing')
    fireEvent.click(smartTab)
    // REACH-B: the single Smart tab maps to the honest `hybrid` URL mode.
    expect(onModeChange).toHaveBeenCalledWith({
      mode: 'hybrid',
      regexMode: false,
    })
  })

  test('renders the relevance layout for the legacy ?mode=semantic alias', () => {
    render(
      <PaperSearchPanel
        query="async runtime"
        mode="semantic"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        rankedEntries={[
          {
            id: 7,
            title: 'tokio internals',
            url: 'https://tokio.rs/x',
            domain: 'tokio.rs',
            time: '10:00',
            matchReason: 'Semantic match',
            relevanceBand: { label: 'High confidence', tone: 'success' },
            dayKey: '2026-05-17',
          },
        ]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // The legacy semantic alias still resolves to the Smart relevance layout.
    expect(screen.getByTestId('paper-search-relevance')).toBeInTheDocument()
    expect(screen.getByText('tokio internals')).toBeVisible()
  })

  test('Smart mode renders ranked entries + pagination and forwards Ask assistant', () => {
    const onAskAssistant = vi.fn()
    const onPrev = vi.fn()
    const onNext = vi.fn()
    const ranked = [
      {
        id: 11,
        title: 'rust async book',
        url: 'https://rust-lang.github.io/async-book',
        domain: 'rust-lang.github.io',
        time: '09:00',
        matchReason: 'Lexical + semantic match',
        relevanceBand: { label: 'Relevant', tone: 'warning' as const },
        dayKey: '2026-05-16',
      },
    ]
    render(
      <PaperSearchPanel
        query="scheduler"
        mode="hybrid"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        rankedEntries={ranked}
        aiNotes={['Lexical-only fallback while AI warms up.']}
        pagination={{
          prevDisabled: false,
          nextDisabled: false,
          onPrev,
          onNext,
          page: 2,
        }}
        onAskAssistant={onAskAssistant}
        relevanceHeaderSlot={<div data-testid="panel-build-cta">build</div>}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // Ranked row + its caption + band render through the shared row component.
    // (Query "scheduler" does not appear in the title, so the <mark> highlighter
    // leaves the heading text intact and `getByText` matches it whole.)
    expect(screen.getByText('rust async book')).toBeVisible()
    expect(screen.getByText('Lexical + semantic match')).toBeVisible()
    // Backend notes, build CTA slot, and pagination all show.
    expect(
      screen.getByText('Lexical-only fallback while AI warms up.'),
    ).toBeVisible()
    expect(screen.getByTestId('panel-build-cta')).toBeVisible()
    expect(
      screen.getByTestId('paper-search-relevance-pagination'),
    ).toBeVisible()
    fireEvent.click(screen.getByTestId('paper-search-relevance-next'))
    expect(onNext).toHaveBeenCalledTimes(1)
    // Ask assistant on the ranked row forwards the entry.
    fireEvent.click(screen.getByTestId('paper-search-result-ask-assistant'))
    expect(onAskAssistant).toHaveBeenCalledWith(ranked[0])
  })

  test('Smart loading flag renders the in-place skeleton, not the day-grouped list', () => {
    render(
      <PaperSearchPanel
        query="async"
        mode="hybrid"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        rankedEntries={[]}
        aiLoading
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    expect(screen.getByTestId('paper-search-relevance-loading')).toBeVisible()
    expect(screen.queryByTestId('paper-search-results')).toBeNull()
  })

  test('an is:starred query stays on the keyword day-grouped layout even in Smart mode', () => {
    render(
      <PaperSearchPanel
        query="is:starred"
        mode="hybrid"
        regexMode={false}
        entries={[makeEntry({ id: 1, title: 'starred page' })]}
        totalResults={1}
        language="en"
        explorerT={explorerT}
        rankedEntries={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // `is:starred` forces the keyword layout (day-grouped), never the ranked view.
    expect(screen.getByTestId('paper-search-results')).toBeInTheDocument()
    expect(screen.queryByTestId('paper-search-relevance')).toBeNull()
  })

  test('aboveResultsCallout renders a StatusCallout below the hero', () => {
    render(
      <PaperSearchPanel
        query="boom"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        aboveResultsCallout={{
          tone: 'blocked',
          eyebrow: 'NO RESULTS',
          title: 'Query failed',
          body: 'sqlite returned an error',
        }}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    const callout = screen.getByTestId('paper-search-above-results-callout')
    expect(callout).toBeInTheDocument()
    expect(callout).toHaveTextContent('Query failed')
    expect(callout).toHaveTextContent('sqlite returned an error')
  })

  test('omitting aboveResultsCallout does not render the slot', () => {
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[makeEntry({ id: 1, title: 'rust lang' })]}
        totalResults={1}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    expect(
      screen.queryByTestId('paper-search-above-results-callout'),
    ).not.toBeInTheDocument()
  })

  test('selecting a result coerces the entry id to a number', () => {
    const onSelectEntry = vi.fn()
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[makeEntry({ id: 42, title: 'tokio' })]}
        totalResults={1}
        language="en"
        explorerT={explorerT}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={onSelectEntry}
        onSeeInContext={() => {}}
      />,
    )
    // PaperSearchResult attaches the click handler on the row; use the
    // data-entry-id selector to find it.
    const row = document.querySelector('[data-entry-id="42"]')
    if (!(row instanceof HTMLElement)) throw new Error('row missing')
    fireEvent.click(row)
    expect(onSelectEntry).toHaveBeenCalledWith(42)
  })

  test('+ Tag chip appends `tag:` to the query and focuses the input after the next animation frame', () => {
    const onQueryChange = vi.fn()
    // Boxed reference so the rAF callback survives TS's control-flow
    // narrowing of `let rafCallback = null` to `never` outside the
    // spy closure.
    const rafBox: { cb: FrameRequestCallback | null } = { cb: null }
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        rafBox.cb = cb
        return 1
      })
    render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-add-tag'))
    expect(onQueryChange).toHaveBeenCalledWith('rust tag:')
    // Flush the rAF callback: the panel uses it to defer the
    // input.focus() + setSelectionRange to the next paint so the
    // browser commits the appended `tag:` text before placing the
    // caret. Without flushing we'd never exercise that branch.
    const input = screen.getByTestId<HTMLInputElement>('paper-search-input')
    const focusSpy = vi.spyOn(input, 'focus')
    const setSelectionRangeSpy = vi.spyOn(input, 'setSelectionRange')
    expect(rafBox.cb).not.toBeNull()
    rafBox.cb?.(0)
    expect(focusSpy).toHaveBeenCalled()
    expect(setSelectionRangeSpy).toHaveBeenCalledWith(
      'rust tag:'.length,
      'rust tag:'.length,
    )
    rafSpy.mockRestore()
  })

  test('back-to-back +Tag and +Note clicks within one frame cancel the earlier focus rAF (review §6)', () => {
    // Pre-fix behaviour: rAF1 fires first and writes setSelectionRange
    // for the SHORTER query ("rust tag:" length 9) against an input
    // whose committed value is the LONGER second query ("rust tag:
    // note:" length 15) — caret lands at position 9 instead of 15.
    // The fix tracks the latest rAF handle in a ref and cancels the
    // previous before scheduling the new one.
    const onQueryChange = vi.fn().mockImplementationOnce(() => {
      // Simulate the parent committing the new value so the input
      // node's `.value` reflects it before rAF2 measures.
    })
    const cancelled: number[] = []
    let nextHandle = 1
    const rafCallbacks: Array<{ handle: number; cb: FrameRequestCallback }> = []
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        const handle = nextHandle++
        rafCallbacks.push({ handle, cb })
        return handle
      })
    const cancelSpy = vi
      .spyOn(window, 'cancelAnimationFrame')
      .mockImplementation((handle: number) => {
        cancelled.push(handle)
      })

    const { rerender } = render(
      <PaperSearchPanel
        query="rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-add-tag'))
    rerender(
      <PaperSearchPanel
        query="rust tag:"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-add-note'))
    // Two clicks scheduled two rAFs; the second click must have
    // cancelled the first (handle === 1).
    expect(cancelled).toContain(1)
    rafSpy.mockRestore()
    cancelSpy.mockRestore()
  })

  test('+ Tag chip emits the appended operator even when the current query is empty', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchPanel
        query=""
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-add-tag'))
    expect(onQueryChange).toHaveBeenCalledWith('tag:')
  })

  test('+ Note chip appends `note:` and works on an empty query without a leading space', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchPanel
        query=""
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-add-note'))
    expect(onQueryChange).toHaveBeenCalledWith('note:')
  })

  test('active tag / note operators surface as removable chips that strip the matching token from the query', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchPanel
        query='rust tag:rust note:"design doc"'
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    // Two active chips render: `tag:rust` and `note:design doc`.
    // Chip ids are identity-based (kind::pos|neg::value::occurrence)
    // so they stay stable even if unrelated tokens are added.
    expect(
      screen.getByTestId('paper-search-active-filter-tag::pos::rust::0'),
    ).toHaveTextContent('tag:rust')
    expect(
      screen.getByTestId('paper-search-active-filter-note::pos::design doc::0'),
    ).toHaveTextContent('note:design doc')
    // Clicking the `tag:` chip's × removes that token from the query
    // and leaves the rest intact (including the quoted note phrase).
    const tagChip = screen.getByTestId(
      'paper-search-active-filter-tag::pos::rust::0',
    )
    const removeButton = tagChip.querySelector('button')
    if (!(removeButton instanceof HTMLButtonElement)) {
      throw new Error('expected remove button inside the tag chip')
    }
    fireEvent.click(removeButton)
    expect(onQueryChange).toHaveBeenCalledWith('rust note:"design doc"')
  })

  test('chip click resolves the right operator after the query was extended between render and click (review §8 id-stability)', () => {
    // Render with one tag chip → its identity-based id is
    // `tag::pos::rust::0`. Then SIMULATE a query update that prepends
    // a new operator before the click commits (e.g. another component
    // wrote to URL state). Re-render with the new query, then click
    // the old chip — the click must still remove the `tag:rust`
    // token, not the prepended new one.
    const onQueryChange = vi.fn()
    const { rerender } = render(
      <PaperSearchPanel
        query="rust tag:rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    expect(
      screen.getByTestId('paper-search-active-filter-tag::pos::rust::0'),
    ).toBeInTheDocument()
    // Query extends with an unrelated `note:hi` token prepended.
    rerender(
      <PaperSearchPanel
        query="note:hi rust tag:rust"
        mode="keyword"
        regexMode={false}
        entries={[]}
        totalResults={0}
        language="en"
        explorerT={explorerT}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onSubmit={() => {}}
        onSelectEntry={() => {}}
        onSeeInContext={() => {}}
      />,
    )
    const tagChip = screen.getByTestId(
      'paper-search-active-filter-tag::pos::rust::0',
    )
    const removeButton = tagChip.querySelector('button')
    if (!(removeButton instanceof HTMLButtonElement)) {
      throw new Error('expected remove button inside the tag chip')
    }
    fireEvent.click(removeButton)
    // Result preserves the note operator and the bare `rust` keyword,
    // drops the `tag:rust` operator.
    expect(onQueryChange).toHaveBeenCalledWith('note:hi rust')
  })
})
