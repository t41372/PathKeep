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
})
