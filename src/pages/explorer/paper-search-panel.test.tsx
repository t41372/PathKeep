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

  test('switching to semantic mode forwards { mode: semantic, regexMode: false }', () => {
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
    const semanticTab = screen
      .getByText('paperSearchView.heroModeSemantic')
      .closest('[role="tab"]')
    if (!(semanticTab instanceof HTMLElement))
      throw new Error('semantic tab missing')
    fireEvent.click(semanticTab)
    expect(onModeChange).toHaveBeenCalledWith({
      mode: 'semantic',
      regexMode: false,
    })
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

  test('+ Tag chip appends `tag:` to the query and focuses the input after the next animation frame', async () => {
    const onQueryChange = vi.fn()
    let rafCallback: FrameRequestCallback | null = null
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((cb: FrameRequestCallback) => {
        rafCallback = cb
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
    expect(rafCallback).not.toBeNull()
    rafCallback?.(0)
    expect(focusSpy).toHaveBeenCalled()
    expect(setSelectionRangeSpy).toHaveBeenCalledWith('rust tag:'.length, 'rust tag:'.length)
    rafSpy.mockRestore()
  })

  test('+ Tag chip is a no-op when appendOperator would return the same query (defensive)', () => {
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
    // appendOperator on empty query returns 'tag:' so this still
    // fires; the early-return branch only triggers if the helper
    // returns the unchanged query, which currently can't happen for
    // valid operator names — kept the guard as a defensive layer.
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
    expect(
      screen.getByTestId('paper-search-active-filter-tag-1'),
    ).toHaveTextContent('tag:rust')
    expect(
      screen.getByTestId('paper-search-active-filter-note-2'),
    ).toHaveTextContent('note:design doc')
    // Clicking the `tag:` chip's × removes that token from the query
    // and leaves the rest intact (including the quoted note phrase).
    const tagChip = screen.getByTestId('paper-search-active-filter-tag-1')
    const removeButton = tagChip.querySelector('button')
    if (!(removeButton instanceof HTMLButtonElement)) {
      throw new Error('expected remove button inside the tag chip')
    }
    fireEvent.click(removeButton)
    expect(onQueryChange).toHaveBeenCalledWith('rust note:"design doc"')
  })
})
