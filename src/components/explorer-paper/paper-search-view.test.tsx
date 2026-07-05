/**
 * Tests for the PaperSearchView composition.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperSearchView,
  type PaperSearchResultEntry,
  type PaperSearchViewCopy,
  type PaperSearchViewDayGroup,
} from './index'

const COPY: PaperSearchViewCopy = {
  hero: {
    prompt: 'What would you like to find again?',
    inputPlaceholder: 'A page, a phrase…',
    modesLabel: 'Mode',
    filtersLabel: 'Filters',
    modeKeyword: 'keyword',
    modeRegex: 'regex',
    modeSmart: 'smart',
    modeHintKeyword: 'Exact words.',
    modeHintRegex: 'JS regex.',
    modeHintSmart: 'Meaning.',
    modeHintSmartUnavailable: 'Turn on in Settings.',
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
      siteExclude: 'site:github.com -pathkeep example',
      exactPhrase: 'Require this exact phrase.',
      or: 'Match either side of OR.',
      field: 'Limit terms to title or URL.',
      fileDate: 'Filter by URL extension and visit date.',
      tag: 'Match user-applied tags.',
      note: 'Substring match against your own notes.',
      starred: 'Show only starred pages.',
      regexNote: 'Regex mode uses Rust regex.',
    },
  },
  empty: {
    tryAskingHeading: 'Try asking',
    recentHeading: 'Recent',
    recentMeta: '{mode} · {count} · {when}',
    footer: 'Search is local.',
    smartPrompt: 'Ask in plain language — that article about Rust async.',
  },
  resultsCount: '{noun} found',
  resultsRange: '{first} — {last} · {mode}',
  pageSuffixSingular: 'page',
  pageSuffixPlural: 'pages',
  noMatchesTitle: 'Nothing here yet. Memory is patient.',
  noMatchesBody: '— try a broader phrase',
  seeInContextLabel: 'See in context',
  dayCountTemplate: '{count} {noun}',
  relevance: {
    rankedCount: 'ranked by relevance',
    askAssistantLabel: 'Ask assistant',
    loadingLabel: 'Finding related pages…',
    prevPageLabel: 'Previous',
    nextPageLabel: 'Next',
    pageSummary: 'Page {page}',
    pageSummaryRanked: 'Page {page} · {total} ranked',
    moreAvailable: 'more available',
    endOfResults: 'end of results',
  },
}

const RANKED: PaperSearchResultEntry[] = [
  {
    id: 7,
    title: 'tokio runtime internals',
    url: 'https://tokio.rs/blog/internals',
    domain: 'tokio.rs',
    time: '14:02',
    matchReason: 'Lexical + semantic match',
    relevanceBand: { label: 'High confidence', tone: 'success' },
    dayKey: '2026-05-16',
  },
  {
    id: 8,
    title: 'async-std vs tokio',
    url: 'https://blog.dev/async',
    domain: 'blog.dev',
    time: '11:20',
    matchReason: 'Semantic match',
    relevanceBand: { label: 'Relevant', tone: 'warning' },
    dayKey: '2026-05-15',
  },
]

const GROUPS: PaperSearchViewDayGroup[] = [
  {
    date: '2026-05-16',
    label: 'Yesterday',
    entries: [
      {
        id: 'r1',
        title: 'tokio-rs/tokio',
        url: 'https://github.com/tokio-rs/tokio',
        domain: 'github.com',
        time: '21:42',
      },
      {
        id: 'r2',
        title: 'Attention Is All You Need',
        url: 'https://arxiv.org/abs/1706.03762',
        domain: 'arxiv.org',
        time: '20:05',
      },
    ],
  },
  {
    date: '2026-05-15',
    label: 'Thursday',
    entries: [
      {
        id: 'r3',
        title: 'docs.rs / sqlx',
        url: 'https://docs.rs/sqlx',
        domain: 'docs.rs',
        time: '09:30',
      },
    ],
  },
]

function renderView(
  overrides: Partial<Parameters<typeof PaperSearchView>[0]> = {},
) {
  return render(
    <PaperSearchView
      query="rust"
      mode="keyword"
      activeFilters={[]}
      groups={GROUPS}
      totalResults={3}
      resolveDomainColor={() => '#888'}
      resolveDomainAbbr={(domain) => domain.slice(0, 3).toUpperCase()}
      onQueryChange={() => {}}
      onModeChange={() => {}}
      onRemoveFilter={() => {}}
      copy={COPY}
      testId="view"
      {...overrides}
    />,
  )
}

describe('PaperSearchView', () => {
  test('renders the hero + result groups when query has matches', () => {
    renderView()

    expect(screen.getByTestId('paper-search-input')).toBeVisible()
    expect(screen.getByTestId('paper-search-results')).toBeVisible()
    expect(screen.getByTestId('paper-search-day-2026-05-16')).toBeVisible()
    expect(screen.getByTestId('paper-search-day-2026-05-15')).toBeVisible()
  })

  test('renders the empty state when query is blank', () => {
    renderView({
      query: '',
      groups: [],
      totalResults: 0,
      suggestions: [{ id: 'p1', cue: 'Just ask', text: 'A question' }],
      recent: [
        { id: 'rs1', q: 'tokio', mode: 'keyword', count: 89, when: 'today' },
      ],
    })

    expect(screen.getByTestId('paper-search-empty')).toBeVisible()
    expect(screen.queryByTestId('paper-search-results')).toBeNull()
    expect(screen.queryByTestId('paper-search-no-matches')).toBeNull()
    // 8c: keyword empty state never shows the Smart prompt.
    expect(screen.queryByTestId('paper-search-empty-smart-prompt')).toBeNull()
  })

  test('8c: the Smart-mode empty state surfaces the natural-language prompt', () => {
    renderView({
      query: '',
      mode: 'smart',
      groups: [],
      totalResults: 0,
    })

    expect(screen.getByTestId('paper-search-empty')).toBeVisible()
    expect(
      screen.getByTestId('paper-search-empty-smart-prompt'),
    ).toHaveTextContent(COPY.empty.smartPrompt)
  })

  test('renders the no-matches branch when query has no results', () => {
    renderView({ query: 'no-such-thing', groups: [], totalResults: 0 })
    expect(screen.getByTestId('paper-search-no-matches')).toBeVisible()
    expect(
      screen.getByText('Nothing here yet. Memory is patient.'),
    ).toBeVisible()
  })

  test('renders the results count + range header', () => {
    renderView()
    expect(screen.getByText('3')).toBeVisible() // total count
    expect(screen.getByText(/2026-05-15 — 2026-05-16/)).toBeVisible()
  })

  test('interpolates singular vs plural noun in the day-count', () => {
    renderView()
    const oldGroup = screen.getByTestId('paper-search-day-2026-05-15')
    expect(within(oldGroup).getByText('1 page')).toBeVisible()
    const newGroup = screen.getByTestId('paper-search-day-2026-05-16')
    expect(within(newGroup).getByText('2 pages')).toBeVisible()
  })

  test('selecting an entry forwards via onSelectEntry', () => {
    const onSelect = vi.fn()
    renderView({ onSelectEntry: onSelect })
    fireEvent.click(screen.getByText('tokio-rs/tokio'))
    expect(onSelect).toHaveBeenCalledWith(GROUPS[0].entries[0])
  })

  test('"See in context" forwards the entry plus the day key', () => {
    const onSeeInContext = vi.fn()
    renderView({ onSeeInContext })
    const dayGroup = screen.getByTestId('paper-search-day-2026-05-15')
    fireEvent.click(
      within(dayGroup).getByTestId('paper-search-result-see-in-context'),
    )
    expect(onSeeInContext).toHaveBeenCalledWith(
      GROUPS[1].entries[0],
      '2026-05-15',
    )
  })

  test('renders the belowHero slot when supplied', () => {
    renderView({
      belowHeroSlot: <div data-testid="below-hero">callout</div>,
    })
    expect(screen.getByTestId('below-hero')).toBeVisible()
  })

  test('forwards an entryStar provider to each result row', () => {
    const onToggle = vi.fn()
    renderView({
      entryStar: {
        isStarred: (url) => url === 'https://github.com/tokio-rs/tokio',
        onToggle,
        starLabel: 'Star',
        unstarLabel: 'Unstar',
      },
    })
    // One result is starred (Unstar label), the rest are not (Star label).
    expect(screen.getByRole('button', { name: 'Unstar' })).toBeVisible()
    const starButtons = screen.getAllByRole('button', { name: 'Star' })
    fireEvent.click(starButtons[0])
    expect(onToggle).toHaveBeenCalled()
  })

  test('skips the star for a result row that has no URL', () => {
    renderView({
      groups: [
        {
          date: '2026-05-16',
          label: 'Yesterday',
          entries: [
            {
              id: 'no-url',
              title: 'No URL row',
              url: '',
              domain: 'x.test',
              time: '01:00',
            },
          ],
        },
      ],
      entryStar: {
        isStarred: () => false,
        onToggle: () => {},
        starLabel: 'Star',
        unstarLabel: 'Unstar',
      },
    })
    // No star button renders for a URL-less row (the `entry.url` guard).
    expect(screen.queryByRole('button', { name: 'Star' })).toBeNull()
  })

  describe('relevance layout (Smart search)', () => {
    test('renders a flat ranked list with match-reason captions + relevance bands', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
      })
      // The flat ranked region renders, NOT the day-grouped one.
      expect(screen.getByTestId('paper-search-relevance')).toBeVisible()
      expect(screen.queryByTestId('paper-search-day-2026-05-16')).toBeNull()
      // Both ranked rows render with their match reason + band.
      expect(screen.getByText('tokio runtime internals')).toBeVisible()
      expect(screen.getByText('Lexical + semantic match')).toBeVisible()
      expect(screen.getByText('Semantic match')).toBeVisible()
      const bands = screen.getAllByTestId('paper-search-result-band')
      expect(bands).toHaveLength(2)
      expect(bands[0]).toHaveTextContent('High confidence')
      expect(bands[0]).toHaveAttribute('data-tone', 'success')
    })

    test('REACH-C3: a Smart row frames its enrichment excerpt by source (no match-claim), and suppresses it when absent', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [
          {
            ...RANKED[0],
            // The adapter stamps both fields together on an enriched row.
            enrichmentExcerpt: 'Reusable workflow runner for CI pipelines',
            enrichmentSourceLabel: 'Page summary',
          },
          RANKED[1], // no excerpt → the affordance must stay suppressed on this row
        ],
        groups: [],
        totalResults: 0,
      })
      // The enriched row surfaces its excerpt framed by the honest source pill —
      // NEVER a "matched in" caption that would lie on a pure-semantic hit.
      expect(screen.getByText('Page summary')).toBeVisible()
      expect(screen.queryByText('Matched in enriched content')).toBeNull()
      expect(
        screen.getByText(/Reusable workflow runner for CI pipelines/),
      ).toBeVisible()
      // Exactly ONE enrichment block renders — the non-enriched ranked row stays suppressed.
      expect(
        screen.getAllByTestId('paper-search-result-enrichment'),
      ).toHaveLength(1)
    })

    test('preserves the backend ranking order (no day re-sort)', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
      })
      const region = screen.getByTestId('paper-search-results')
      const rows = region.querySelectorAll('[data-entry-id]')
      // Order matches `RANKED` (id 7 then 8), not a day-grouped reordering.
      expect(rows[0].getAttribute('data-entry-id')).toBe('7')
      expect(rows[1].getAttribute('data-entry-id')).toBe('8')
    })

    test('shows the in-place loading skeleton while Smart results stream in', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [],
        groups: [],
        totalResults: 0,
        aiLoading: true,
      })
      // Composer stays mounted; only the results region shows the skeleton.
      expect(screen.getByTestId('paper-search-input')).toBeVisible()
      expect(screen.getByTestId('paper-search-relevance-loading')).toBeVisible()
      expect(screen.queryByTestId('paper-search-results')).toBeNull()
    })

    test('renders the no-matches state for an empty ranked set with a query', () => {
      renderView({
        query: 'nothing matches',
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [],
        groups: [],
        totalResults: 0,
      })
      expect(screen.getByTestId('paper-search-no-matches')).toBeVisible()
    })

    test('holds the region empty (composer mounted) while an AI error is showing', () => {
      renderView({
        query: 'boom',
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [],
        groups: [],
        totalResults: 0,
        aiError: 'provider down',
      })
      // The error itself renders in the route's belowHeroSlot, so the region is
      // intentionally bare; the composer must stay mounted for retry.
      expect(screen.getByTestId('paper-search-input')).toBeVisible()
      expect(
        screen.getByTestId('paper-search-relevance-error-region'),
      ).toBeInTheDocument()
      expect(screen.queryByTestId('paper-search-no-matches')).toBeNull()
    })

    test('renders backend notes above the ranked list', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        aiNotes: ['Lexical-only fallback: AI is warming up.'],
      })
      const notes = screen.getByTestId('paper-search-relevance-notes')
      expect(notes).toHaveTextContent(
        'Lexical-only fallback: AI is warming up.',
      )
    })

    test('renders the relevance header slot (build CTA) when supplied', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        relevanceHeaderSlot: <div data-testid="build-cta">Build index</div>,
      })
      expect(screen.getByTestId('paper-search-relevance-header')).toBeVisible()
      expect(screen.getByTestId('build-cta')).toBeVisible()
    })

    test('fires the per-row Ask assistant handler only for ranked rows that carry a match reason', () => {
      const onAskAssistant = vi.fn()
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        onAskAssistant,
      })
      const askButtons = screen.getAllByTestId(
        'paper-search-result-ask-assistant',
      )
      expect(askButtons).toHaveLength(2)
      fireEvent.click(askButtons[0])
      expect(onAskAssistant).toHaveBeenCalledWith(RANKED[0])
    })

    test('selecting a ranked row forwards the entry (real historyId) to onSelectEntry', () => {
      const onSelectEntry = vi.fn()
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        onSelectEntry,
      })
      fireEvent.click(screen.getByText('tokio runtime internals'))
      expect(onSelectEntry).toHaveBeenCalledWith(RANKED[0])
    })

    test('see-in-context on a ranked row jumps to that row dayKey', () => {
      const onSeeInContext = vi.fn()
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        onSeeInContext,
      })
      const buttons = screen.getAllByTestId(
        'paper-search-result-see-in-context',
      )
      fireEvent.click(buttons[1])
      expect(onSeeInContext).toHaveBeenCalledWith(RANKED[1], '2026-05-15')
    })

    test('renders prev/next pagination and forwards both controls', () => {
      const onPrev = vi.fn()
      const onNext = vi.fn()
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: false,
          nextDisabled: false,
          onPrev,
          onNext,
          page: 2,
        },
      })
      const pager = screen.getByTestId('paper-search-relevance-pagination')
      expect(pager).toHaveTextContent('Page 2')
      fireEvent.click(screen.getByTestId('paper-search-relevance-prev'))
      fireEvent.click(screen.getByTestId('paper-search-relevance-next'))
      expect(onPrev).toHaveBeenCalledTimes(1)
      expect(onNext).toHaveBeenCalledTimes(1)
    })

    test('disables the pagination controls per the pagination flags', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: true,
          nextDisabled: true,
          onPrev: () => {},
          onNext: () => {},
          page: 1,
        },
      })
      expect(
        screen.getByTestId<HTMLButtonElement>('paper-search-relevance-prev')
          .disabled,
      ).toBe(true)
      expect(
        screen.getByTestId<HTMLButtonElement>('paper-search-relevance-next')
          .disabled,
      ).toBe(true)
    })

    test('I2: bounds the page within the ranked set when a total is known', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: false,
          nextDisabled: false,
          onPrev: () => {},
          onNext: () => {},
          page: 2,
          total: 137,
        },
      })
      const summary = screen.getByTestId('paper-search-relevance-page-summary')
      // "Page 2 · 137 ranked" — not a bare ordinal.
      expect(summary).toHaveTextContent('Page 2 · 137 ranked')
      // An enabled Next means there is more to see.
      expect(
        screen.getByTestId('paper-search-relevance-position'),
      ).toHaveTextContent('more available')
    })

    test('I2: shows "end of results" on the last page (no next cursor)', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: false,
          nextDisabled: true,
          onPrev: () => {},
          onNext: () => {},
          page: 3,
          total: 137,
        },
      })
      expect(
        screen.getByTestId('paper-search-relevance-position'),
      ).toHaveTextContent('end of results')
    })

    test('I2: falls back to the bare ordinal when the total is unknown', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: false,
          nextDisabled: false,
          onPrev: () => {},
          onNext: () => {},
          page: 2,
          total: null,
        },
      })
      const summary = screen.getByTestId('paper-search-relevance-page-summary')
      expect(summary).toHaveTextContent('Page 2')
      expect(summary).not.toHaveTextContent('ranked')
    })

    test('I3: renders the scope/freshness micro-line when provided', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        relevanceScopeLine: '1,240 pages indexed · updated May 17, 2026',
      })
      expect(
        screen.getByTestId('paper-search-relevance-scope'),
      ).toHaveTextContent('1,240 pages indexed · updated May 17, 2026')
    })

    test('I3: omits the scope micro-line when there is nothing honest to say', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        relevanceScopeLine: null,
      })
      expect(screen.queryByTestId('paper-search-relevance-scope')).toBeNull()
    })

    test('shows the patient prompt when the relevance query is blank', () => {
      renderView({
        query: '',
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [],
        groups: [],
        totalResults: 0,
      })
      expect(screen.getByTestId('paper-search-relevance-prompt')).toBeVisible()
    })

    test('forwards an entryStar provider to each ranked row', () => {
      const onToggle = vi.fn()
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        entryStar: {
          isStarred: (url) => url === RANKED[0].url,
          onToggle,
          starLabel: 'Star',
          unstarLabel: 'Unstar',
        },
      })
      // Row 0 is starred (Unstar), row 1 is not (Star).
      expect(screen.getByRole('button', { name: 'Unstar' })).toBeVisible()
      const stars = screen.getAllByRole('button', { name: 'Star' })
      fireEvent.click(stars[0])
      expect(onToggle).toHaveBeenCalledWith(RANKED[1].url)
    })

    test('suppresses see-in-context on a ranked row that has no dayKey', () => {
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [{ ...RANKED[0], dayKey: undefined }],
        groups: [],
        totalResults: 0,
        onSeeInContext: vi.fn(),
      })
      // Without a dayKey there is no Browse day to jump to, so the affordance
      // is withheld rather than firing a no-op jump.
      expect(
        screen.queryByTestId('paper-search-result-see-in-context'),
      ).toBeNull()
    })

    test('renders the relevance layout when the copy bundle omits the relevance keys', () => {
      // `copy.relevance` is optional; a caller that has not wired the bundle yet
      // must still render the ranked rows (the optional-chained labels collapse
      // to empty strings) without throwing.
      const copyWithoutRelevance: PaperSearchViewCopy = {
        ...COPY,
        relevance: undefined,
      }
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        aiLoading: false,
        pagination: {
          prevDisabled: false,
          // A known total + a disabled Next exercise BOTH the `pageSummaryRanked`
          // and `endOfResults` `?? ''` fallbacks when the relevance copy is
          // omitted — the labels collapse to empty strings without throwing.
          nextDisabled: true,
          onPrev: () => {},
          onNext: () => {},
          page: 1,
          total: 12,
        },
        copy: copyWithoutRelevance,
      })
      expect(screen.getByTestId('paper-search-results')).toBeVisible()
      expect(screen.getByText('tokio runtime internals')).toBeVisible()
      // The pagination chrome still renders even without the summary label.
      expect(
        screen.getByTestId('paper-search-relevance-pagination'),
      ).toBeVisible()
      // The summary + position render as empty strings (no copy), not a crash.
      expect(
        screen.getByTestId('paper-search-relevance-page-summary'),
      ).toHaveTextContent('')
    })

    test('pagination without relevance copy: bare-ordinal + more-available fallbacks collapse to empty', () => {
      // Complements the test above: no `total` (bare-ordinal `pageSummary ?? ''`
      // branch) + an enabled Next (`moreAvailable ?? ''` branch), both with the
      // relevance copy omitted, so every `?? ''` fallback is exercised.
      const copyWithoutRelevance: PaperSearchViewCopy = {
        ...COPY,
        relevance: undefined,
      }
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: RANKED,
        groups: [],
        totalResults: 0,
        pagination: {
          prevDisabled: false,
          nextDisabled: false,
          onPrev: () => {},
          onNext: () => {},
          page: 1,
          total: null,
        },
        copy: copyWithoutRelevance,
      })
      expect(
        screen.getByTestId('paper-search-relevance-page-summary'),
      ).toHaveTextContent('')
    })

    test('renders the loading skeleton even without the relevance copy bundle', () => {
      const copyWithoutRelevance: PaperSearchViewCopy = {
        ...COPY,
        relevance: undefined,
      }
      renderView({
        mode: 'smart',
        resultLayout: 'relevance',
        rankedEntries: [],
        groups: [],
        totalResults: 0,
        aiLoading: true,
        copy: copyWithoutRelevance,
      })
      expect(screen.getByTestId('paper-search-relevance-loading')).toBeVisible()
    })
  })
})
