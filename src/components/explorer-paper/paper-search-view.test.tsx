/**
 * Tests for the PaperSearchView composition.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperSearchView,
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
    modeSemantic: 'semantic',
    modeHintKeyword: 'Exact words.',
    modeHintRegex: 'JS regex.',
    modeHintSemantic: 'Meaning.',
    addFilterDate: '+ Date',
    addFilterSource: '+ Source',
    addFilterDomain: '+ Domain',
    addFilterVisitCount: '+ Visit count',
    removeChipLabel: 'Remove {label}',
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
      regexNote: 'Regex mode uses Rust regex.',
    },
  },
  empty: {
    tryAskingHeading: 'Try asking',
    recentHeading: 'Recent',
    recentMeta: '{mode} · {count} · {when}',
    footer: 'Search is local.',
  },
  resultsCount: '{noun} found',
  resultsRange: '{first} — {last} · {mode}',
  pageSuffixSingular: 'page',
  pageSuffixPlural: 'pages',
  noMatchesTitle: 'Nothing here yet. Memory is patient.',
  noMatchesBody: '— try a broader phrase',
  seeInContextLabel: 'See in context',
  dayCountTemplate: '{count} {noun}',
}

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
})
