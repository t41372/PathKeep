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
  modeSmart: 'smart',
  modeHintKeyword: 'Match the exact words.',
  modeHintRegex: 'JavaScript regex.',
  modeHintSmart: 'Meaning, not just words.',
  modeHintSmartUnavailable: 'Turn it on in Settings.',
  modeSmartUnavailableAria: '(unavailable)',
  addFilterDate: '+ Date',
  addFilterSource: '+ Source',
  addFilterDomain: '+ Domain',
  addFilterVisitCount: '+ Visit count',
  addFilterTag: '+ Tag',
  addFilterNote: '+ Note',
  removeChipLabel: 'Remove {label}',
  searchButton: 'Search',
  searchingButton: 'Searching…',
  searchButtonAria: 'Search history',
  searchingButtonAria: 'Searching history…',
  submitHint: 'Press Enter or Search to run',
  staleBanner: 'Showing {mode} results — press Search to update',
  staleModeNames: {
    keyword: 'Keyword',
    regex: 'Regex',
    smart: 'Smart',
  },
  advancedSyntaxHelp: {
    ariaLabel: 'Show advanced keyword syntax',
    title: 'Advanced keyword syntax',
    intro: 'Use these operators in Keyword mode.',
    siteExclude: 'Only github.com results, excluding pathkeep.',
    exactPhrase: 'Require this exact phrase.',
    or: 'Match either side of OR.',
    field: 'Limit terms to title or URL.',
    fileDate: 'Filter by URL extension and visit date.',
    tag: 'Match user-applied tags.',
    note: 'Substring match against your own notes.',
    starred: 'Show only starred pages.',
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
    expect(screen.getByRole('tab', { name: 'smart' })).toBeVisible()
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

  test('a non-Enter, non-Escape key is left to the input (no handler fires)', () => {
    const onSubmit = vi.fn()
    const onQueryChange = vi.fn()
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={onQueryChange}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onSubmit={onSubmit}
        copy={COPY}
      />,
    )

    // ArrowRight is neither Enter nor Escape — the hero must not submit or
    // clear; it falls through so the input handles caret movement normally.
    fireEvent.keyDown(screen.getByTestId('paper-search-input'), {
      key: 'ArrowRight',
    })
    expect(onSubmit).not.toHaveBeenCalled()
    expect(onQueryChange).not.toHaveBeenCalled()
  })

  test('Enter without an onSubmit handler is swallowed (no throw)', () => {
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    // The optional `onSubmit?.()` must short-circuit cleanly when no handler is
    // wired (preview fixtures pass no onSubmit). Pressing Enter is a no-op.
    expect(() =>
      fireEvent.keyDown(screen.getByTestId('paper-search-input'), {
        key: 'Enter',
      }),
    ).not.toThrow()
  })

  test('clicking Search without an onSubmit handler is swallowed (no throw)', () => {
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )

    expect(() =>
      fireEvent.click(screen.getByTestId('paper-search-submit')),
    ).not.toThrow()
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
    const smart = screen.getByRole('tab', { name: 'smart' })
    expect(keyword.getAttribute('aria-selected')).toBe('true')
    expect(smart.getAttribute('aria-selected')).toBe('false')

    fireEvent.click(smart)
    expect(onModeChange).toHaveBeenCalledWith('smart')

    rerender(
      <PaperSearchHero
        query=""
        mode="smart"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onRemoveFilter={() => {}}
        copy={COPY}
      />,
    )
    expect(
      screen.getByRole('tab', { name: 'smart' }).getAttribute('aria-selected'),
    ).toBe('true')
    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'Meaning, not just words.',
    )
  })

  test('Smart tab is disabled and shows the unavailable hint when smartAvailable is false', () => {
    const onModeChange = vi.fn()
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={onModeChange}
        onRemoveFilter={() => {}}
        smartAvailable={false}
        copy={COPY}
      />,
    )

    const smart = screen.getByTestId<HTMLButtonElement>(
      'paper-search-mode-smart',
    )
    // Still visible (REACH-A "available to turn on" discoverability) but disabled.
    expect(smart).toBeVisible()
    expect(smart.disabled).toBe(true)
    expect(smart).toHaveAttribute('aria-label', 'smart (unavailable)')
    // A disabled tab swallows the click so it never flips the active mode.
    fireEvent.click(smart)
    expect(onModeChange).not.toHaveBeenCalled()
    // The keyword-mode hero reads the unavailable hint on its Smart tab.
    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'Match the exact words.',
    )
  })

  test('an active Smart mode that becomes unavailable shows the unavailable hint', () => {
    render(
      <PaperSearchHero
        query=""
        mode="smart"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        smartAvailable={false}
        copy={COPY}
      />,
    )
    expect(screen.getByTestId('paper-search-mode-hint')).toHaveTextContent(
      'Turn it on in Settings.',
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
    const onAddTag = vi.fn()
    const onAddNote = vi.fn()
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
        onAddTagFilter={onAddTag}
        onAddNoteFilter={onAddNote}
        copy={COPY}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-search-add-date'))
    fireEvent.click(screen.getByTestId('paper-search-add-source'))
    fireEvent.click(screen.getByTestId('paper-search-add-domain'))
    fireEvent.click(screen.getByTestId('paper-search-add-visit-count'))
    fireEvent.click(screen.getByTestId('paper-search-add-tag'))
    fireEvent.click(screen.getByTestId('paper-search-add-note'))

    expect(onAddDate).toHaveBeenCalledTimes(1)
    expect(onAddSource).toHaveBeenCalledTimes(1)
    expect(onAddDomain).toHaveBeenCalledTimes(1)
    expect(onAddVisitCount).toHaveBeenCalledTimes(1)
    expect(onAddTag).toHaveBeenCalledTimes(1)
    expect(onAddNote).toHaveBeenCalledTimes(1)
  })

  test('annotation add-chips (+ Tag / + Note) render with the supplied copy labels', () => {
    render(
      <PaperSearchHero
        query=""
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onAddTagFilter={() => {}}
        onAddNoteFilter={() => {}}
        copy={COPY}
      />,
    )

    expect(screen.getByTestId('paper-search-add-tag')).toHaveTextContent(
      '+ Tag',
    )
    expect(screen.getByTestId('paper-search-add-note')).toHaveTextContent(
      '+ Note',
    )
  })

  test('+Tag and +Note chips stay disabled when their handlers are omitted', () => {
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
      screen.getByTestId<HTMLButtonElement>('paper-search-add-tag').disabled,
    ).toBe(true)
    expect(
      screen.getByTestId<HTMLButtonElement>('paper-search-add-note').disabled,
    ).toBe(true)
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
    // Confirms the §3.3 A tag/note operators surface in the popover so
    // users discover them via the same hover affordance.
    expect(panel).toHaveTextContent('tag:rust -tag:archived')
    expect(panel).toHaveTextContent('note:"design doc"')

    fireEvent.mouseLeave(trigger)
    expect(
      screen.queryByTestId('paper-advanced-search-help-panel'),
    ).not.toBeInTheDocument()
  })

  test('advanced-syntax help opens on keyboard focus and closes on blur', () => {
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

    const trigger = screen
      .getByTestId('paper-search-advanced-help')
      .querySelector('button')
    if (!(trigger instanceof HTMLButtonElement)) {
      throw new Error('advanced help button missing')
    }

    fireEvent.focus(trigger)
    expect(screen.getByTestId('paper-advanced-search-help-panel')).toBeVisible()
    expect(trigger).toHaveAttribute('aria-expanded', 'true')

    fireEvent.blur(trigger)
    expect(
      screen.queryByTestId('paper-advanced-search-help-panel'),
    ).not.toBeInTheDocument()
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
  })

  test('Search button submits the current query on click', () => {
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

    const button = screen.getByTestId('paper-search-submit')
    expect(button).toHaveTextContent('Search')
    expect(button).toHaveAttribute('aria-label', 'Search history')
    fireEvent.click(button)
    expect(onSubmit).toHaveBeenCalledWith('rust async')
  })

  test('Search button is disabled when submitDisabled is set', () => {
    const onSubmit = vi.fn()
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onSubmit={onSubmit}
        submitDisabled
        copy={COPY}
      />,
    )

    const button = screen.getByTestId<HTMLButtonElement>('paper-search-submit')
    expect(button.disabled).toBe(true)
    fireEvent.click(button)
    expect(onSubmit).not.toHaveBeenCalled()
  })

  test('Searching state swaps the label + spinner but never locks the button', () => {
    const onSubmit = vi.fn()
    render(
      <PaperSearchHero
        query="rust"
        mode="keyword"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        onSubmit={onSubmit}
        isSearching
        copy={COPY}
      />,
    )

    const button = screen.getByTestId<HTMLButtonElement>('paper-search-submit')
    expect(button).toHaveTextContent('Searching…')
    expect(button).toHaveAttribute('aria-label', 'Searching history…')
    expect(button).toHaveAttribute('aria-busy', 'true')
    expect(
      screen.getByTestId('paper-search-submit-spinner'),
    ).toBeInTheDocument()
    // Not locked: a re-submit while searching is still allowed.
    expect(button.disabled).toBe(false)
    fireEvent.click(button)
    expect(onSubmit).toHaveBeenCalledWith('rust')
  })

  test('Escape on an empty query blurs the input instead of being a no-op', () => {
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

    const input = screen.getByTestId<HTMLInputElement>('paper-search-input')
    input.focus()
    expect(document.activeElement).toBe(input)
    fireEvent.keyDown(input, { key: 'Escape' })
    // Empty draft → Esc does not call onQueryChange, it releases focus.
    expect(onQueryChange).not.toHaveBeenCalled()
    expect(document.activeElement).not.toBe(input)
  })

  test('renders the stale-results banner with the last-submitted mode name', () => {
    render(
      <PaperSearchHero
        query="rust"
        mode="regex"
        activeFilters={[]}
        onQueryChange={() => {}}
        onModeChange={() => {}}
        onRemoveFilter={() => {}}
        staleMode="smart"
        copy={COPY}
      />,
    )

    const banner = screen.getByTestId('paper-search-stale-banner')
    expect(banner).toHaveTextContent(
      'Showing Smart results — press Search to update',
    )
    // The banner replaces the submit hint while it's showing.
    expect(
      screen.queryByTestId('paper-search-submit-hint'),
    ).not.toBeInTheDocument()
  })

  test('shows the submit hint (no stale banner) by default', () => {
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

    expect(screen.getByTestId('paper-search-submit-hint')).toHaveTextContent(
      'Press Enter or Search to run',
    )
    expect(
      screen.queryByTestId('paper-search-stale-banner'),
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
