/**
 * Tests for PaperSearchHero — the literary search input + mode toggle +
 * filter chips.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { PaperSearchHero, type PaperSearchHeroCopy } from './paper-search-hero'

const COPY: PaperSearchHeroCopy = {
  prompt: 'What would you like to find again?',
  inputPlaceholder: 'A page, a phrase, a feeling…',
  modesLabel: 'Mode',
  filtersLabel: 'Filters',
  modeKeyword: 'keyword',
  modeRegex: 'regex',
  modeSemantic: 'semantic',
  modeHintKeyword: 'Match the exact words.',
  modeHintRegex: 'JavaScript regex.',
  modeHintSemantic: 'Meaning, not just words.',
  addFilterDate: '+ Date',
  addFilterSource: '+ Source',
  addFilterDomain: '+ Domain',
  addFilterVisitCount: '+ Visit count',
  removeChipLabel: 'Remove {label}',
  advancedSyntaxHelp: {
    ariaLabel: 'Show advanced keyword syntax',
    title: 'Advanced keyword syntax',
    intro: 'Use these operators in Keyword mode.',
    siteExclude: 'Only github.com results, excluding pathkeep.',
    exactPhrase: 'Require this exact phrase.',
    or: 'Match either side of OR.',
    field: 'Limit terms to title or URL.',
    fileDate: 'Filter by URL extension and visit date.',
    regexNote: 'Regex mode uses Rust regex.',
  },
}

describe('PaperSearchHero', () => {
  test('renders prompt, input, modes, mode hint, and filter labels', () => {
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
        testId="hero"
      />,
    )

    expect(screen.getByText('What would you like to find again?')).toBeVisible()
    expect(screen.getByTestId('paper-search-input')).toBeVisible()
    expect(screen.getByRole('tab', { name: 'keyword' })).toBeVisible()
    expect(screen.getByRole('tab', { name: 'regex' })).toBeVisible()
    expect(screen.getByRole('tab', { name: 'semantic' })).toBeVisible()
    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'Match the exact words.',
    )
  })

  test('input change forwards to onQueryChange', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    fireEvent.change(screen.getByTestId('paper-search-input'), {
      target: { value: 'rust' },
    })
    expect(onQueryChange).toHaveBeenCalledWith('rust')
  })

  test('Enter calls onSubmit with the current query', () => {
    const onSubmit = vi.fn()
    render(
      <PaperSearchHero
        query="rust async"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onSubmit={onSubmit}
        copy={COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-search-input'), {
      key: 'Enter',
    })
    expect(onSubmit).toHaveBeenCalledWith('rust async')
  })

  test('Escape with a non-empty query clears the input', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-search-input'), {
      key: 'Escape',
    })
    expect(onQueryChange).toHaveBeenCalledWith('')
  })

  test('Escape with an empty query is a no-op', () => {
    const onQueryChange = vi.fn()
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    fireEvent.keyDown(screen.getByTestId('paper-search-input'), {
      key: 'Escape',
    })
    expect(onQueryChange).not.toHaveBeenCalled()
  })

  test('mode toggle reflects aria-selected and calls onModeChange', () => {
    const onModeChange = vi.fn()
    const { rerender } = render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    const keyword = screen.getByRole('tab', { name: 'keyword' })
    const semantic = screen.getByRole('tab', { name: 'semantic' })
    expect(keyword.getAttribute('aria-selected')).toBe('true')
    expect(semantic.getAttribute('aria-selected')).toBe('false')

    fireEvent.click(semantic)
    expect(onModeChange).toHaveBeenCalledWith('semantic')

    rerender(
      <PaperSearchHero
        query=""
        mode="semantic"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )
    expect(
      screen
        .getByRole('tab', { name: 'semantic' })
        .getAttribute('aria-selected'),
    ).toBe('true')
    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'Meaning, not just words.',
    )
  })

  test('regex mode shows its hint', () => {
    render(
      <PaperSearchHero
        query=""
        mode="regex"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'JavaScript regex.',
    )
  })

  test('active filters render with remove buttons', () => {
    const onRemoveFilter = vi.fn()
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[
          { id: 'date-30', label: 'Last 30 days' },
          { id: 'source-chrome', label: 'Chrome' },
        ]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={onRemoveFilter}
        copy={COPY}
      />,
    )

    expect(screen.getByText('Last 30 days')).toBeVisible()
    expect(screen.getByText('Chrome')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: 'Remove Last 30 days' }))
    expect(onRemoveFilter).toHaveBeenCalledWith('date-30')
  })

  test('add-filter chips are disabled when their handler is omitted', () => {
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    expect(
      screen.getByTestId<HTMLButtonElement>('paper-search-add-date').disabled,
    ).toBe(true)
    expect(
      screen.getByTestId<HTMLButtonElement>('paper-search-add-source').disabled,
    ).toBe(true)
  })

  test('add-filter chips fire their handlers when enabled', () => {
    const onAddDate = vi.fn()
    const onAddSource = vi.fn()
    const onAddDomain = vi.fn()
    const onAddVisitCount = vi.fn()
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onAddDateFilter={onAddDate}
        onAddSourceFilter={onAddSource}
        onAddDomainFilter={onAddDomain}
        onAddVisitCountFilter={onAddVisitCount}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-search-add-date'))
    fireEvent.click(screen.getByTestId('paper-search-add-source'))
    fireEvent.click(screen.getByTestId('paper-search-add-domain'))
    fireEvent.click(screen.getByTestId('paper-search-add-visit-count'))

    expect(onAddDate).toHaveBeenCalledTimes(1)
    expect(onAddSource).toHaveBeenCalledTimes(1)
    expect(onAddDomain).toHaveBeenCalledTimes(1)
    expect(onAddVisitCount).toHaveBeenCalledTimes(1)
  })

  test('renders the advanced-syntax help trigger and reveals the popover on hover', () => {
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    const trigger = screen.getByTestId('paper-search-advanced-help')
    expect(trigger).toBeVisible()
    // Popover hidden until hover/focus opens it.
    expect(
      screen.queryByTestId('paper-advanced-search-help-panel'),
    ).not.toBeInTheDocument()

    fireEvent.mouseEnter(trigger)
    const panel = screen.getByTestId('paper-advanced-search-help-panel')
    expect(panel).toBeVisible()
    expect(panel).toHaveTextContent('site:github.com -pathkeep')
    expect(panel).toHaveTextContent('Advanced keyword syntax')

    fireEvent.mouseLeave(trigger)
    expect(
      screen.queryByTestId('paper-advanced-search-help-panel'),
    ).not.toBeInTheDocument()
  })

  test('forwards ref to the input for parent-managed focus', () => {
    let input: HTMLInputElement | null = null
    render(
      <PaperSearchHero
        ref={(node) => {
          input = node
        }}
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    expect(input).toBe(screen.getByTestId('paper-search-input'))
  })
})
