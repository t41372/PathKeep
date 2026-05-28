import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperSearchPanel } from './paper-search-panel'

vi.mock('@/components/explorer-paper', () => ({
  PaperSearchView: (props: {
    onAddTagFilter: () => void
    onRemoveFilter: (id: string) => void
  }) => (
    <div data-testid="mock-paper-search-view">
      <button
        type="button"
        data-testid="mock-add-tag"
        onClick={props.onAddTagFilter}
      >
        add tag
      </button>
      <button
        type="button"
        data-testid="mock-remove-missing"
        onClick={() => props.onRemoveFilter('missing-filter-id')}
      >
        remove missing
      </button>
    </div>
  ),
}))

function explorerT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

describe('PaperSearchPanel child contract defenses', () => {
  test('focus scheduling tolerates a child search view that does not attach the input ref', () => {
    const onQueryChange = vi.fn()
    let scheduled: FrameRequestCallback | null = null
    const rafSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((callback: FrameRequestCallback) => {
        scheduled = callback
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

    fireEvent.click(screen.getByTestId('mock-add-tag'))
    expect(onQueryChange).toHaveBeenCalledWith('rust tag:')
    expect(scheduled).not.toBeNull()
    expect(() => scheduled?.(0)).not.toThrow()

    rafSpy.mockRestore()
  })

  test('stale filter removal ids are ignored without rewriting the query', () => {
    const onQueryChange = vi.fn()
    render(
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

    fireEvent.click(screen.getByTestId('mock-remove-missing'))
    expect(onQueryChange).not.toHaveBeenCalled()
  })
})
