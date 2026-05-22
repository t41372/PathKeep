import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { PaperDayInsights } from './paper-day-insights'
import type { DayInsights } from './paper-day-insights-helpers'

const COPY = {
  topDomainsTitle: 'Top domains',
  activityTitle: 'Activity',
  hourlyTitle: '24-hour activity',
  pagesLabel: 'Pages',
  typedLabel: 'Typed',
  linksLabel: 'Links',
  searchesLabel: 'Searches',
  sessionsTemplate: '{count} sessions',
  domainsTemplate: '{count} domains',
  moreDetailsLabel: 'More details',
  firstVisitLabel: 'First visit',
  lastVisitLabel: 'Last visit',
  peakHourLabel: 'Peak hour',
  longestSessionLabel: 'Longest session',
  topUrlsTitle: 'Most revisited',
  visitsCountTemplate: '{count} visits',
}

function makeInsights(overrides: Partial<DayInsights> = {}): DayInsights {
  return {
    totalPages: 12,
    typedCount: 2,
    linkCount: 8,
    searchCount: 1,
    distinctDomains: 4,
    sessionCount: 3,
    topDomains: [
      { domain: 'github.com', visits: 5 },
      { domain: 'docs.rs', visits: 3 },
    ],
    hourBuckets: new Array<number>(24)
      .fill(0)
      .map((_, idx) => (idx === 10 ? 4 : 0)),
    hourPeak: 4,
    firstVisitMs: null,
    lastVisitMs: null,
    peakHour: null,
    longestSessionMs: 0,
    topUrls: [],
    ...overrides,
  }
}

describe('PaperDayInsights', () => {
  test('renders nothing when the day has zero visits', () => {
    const { container } = render(
      <PaperDayInsights
        insights={makeInsights({ totalPages: 0, topDomains: [] })}
        copy={COPY}
      />,
    )
    expect(container.firstChild).toBeNull()
  })

  test('renders top domains, activity tallies, and hourly sparkline', () => {
    render(<PaperDayInsights insights={makeInsights()} copy={COPY} />)
    expect(screen.getByText('Top domains')).toBeVisible()
    expect(screen.getByText('github.com')).toBeVisible()
    expect(screen.getByText('docs.rs')).toBeVisible()
    expect(screen.getByText('Pages')).toBeVisible()
    // The "12" hour-axis label collides with totalPages=12; assert the
    // pages tally via its label sibling so we're unambiguous about which
    // value we're checking.
    const pagesRow = screen.getByText('Pages').parentElement!
    expect(pagesRow.textContent).toContain('12')
    expect(screen.getByText('3 sessions')).toBeVisible()
    expect(screen.getByText('4 domains')).toBeVisible()
    // svg renders with role=img
    expect(
      screen.getByRole('img', { name: '24-hour activity' }),
    ).toBeInTheDocument()
  })

  test('renders an em-dash when no top domains are available', () => {
    render(
      <PaperDayInsights
        insights={makeInsights({ topDomains: [] })}
        copy={COPY}
      />,
    )
    expect(screen.getByText('—')).toBeVisible()
  })
})
