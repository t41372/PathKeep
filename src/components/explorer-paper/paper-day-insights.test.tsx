import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { PaperDayInsights } from './paper-day-insights'
import type { DayInsights } from './paper-day-insights-helpers'

function openDisclosure() {
  // Tests render a closed <details>; flip it open so the assertions
  // that walk descendants of the disclosure can observe them.
  for (const element of document.querySelectorAll('details')) {
    element.open = true
  }
}

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

  test('renders the More details disclosure with formatted first/last/peak/longest cells', () => {
    const fixedFirst = new Date('2026-05-21T09:14:00').getTime()
    const fixedLast = new Date('2026-05-21T22:43:00').getTime()
    render(
      <PaperDayInsights
        insights={makeInsights({
          firstVisitMs: fixedFirst,
          lastVisitMs: fixedLast,
          peakHour: 15,
          longestSessionMs: 75 * 60_000, // 1h 15m
          topUrls: [
            { url: 'https://docs.rs/sqlx', title: 'sqlx docs', visits: 7 },
            { url: 'http://example.com', title: null, visits: 4 },
          ],
        })}
        copy={COPY}
        language="en"
      />,
    )
    expect(screen.getByText('More details')).toBeVisible()
    openDisclosure()
    expect(screen.getByText('First visit')).toBeVisible()
    expect(screen.getByText('Last visit')).toBeVisible()
    expect(screen.getByText('Peak hour')).toBeVisible()
    expect(screen.getByText('Longest session')).toBeVisible()
    expect(screen.getByText('Most revisited')).toBeVisible()
    // URL renders without scheme so the row reads as host+path; the title
    // attribute carries the friendly title for screen readers.
    expect(screen.getByText('docs.rs/sqlx')).toBeVisible()
    expect(screen.getByText('example.com')).toBeVisible()
    // Plural-aware visit count template substitutes {count} per row.
    expect(screen.getByText('7 visits')).toBeVisible()
    expect(screen.getByText('4 visits')).toBeVisible()
  })

  test('does not render the More details disclosure when no extras are available', () => {
    render(
      <PaperDayInsights
        insights={makeInsights({
          firstVisitMs: null,
          lastVisitMs: null,
          peakHour: null,
          longestSessionMs: 0,
          topUrls: [],
        })}
        copy={COPY}
      />,
    )
    expect(screen.queryByText('More details')).toBeNull()
  })

  test('respects hour12 false for the disclosure time cells', () => {
    const fixedFirst = new Date('2026-05-21T13:14:00').getTime()
    render(
      <PaperDayInsights
        insights={makeInsights({
          firstVisitMs: fixedFirst,
          lastVisitMs: fixedFirst,
          peakHour: 13,
          longestSessionMs: 0,
          topUrls: [],
        })}
        copy={COPY}
        hour12={false}
        language="en"
      />,
    )
    openDisclosure()
    // 24-hour rendering keeps the 13 hour prefix without an AM/PM suffix.
    expect(screen.getAllByText(/13:14/).length).toBeGreaterThan(0)
  })

  test('formatDuration spans hour + minute units for sessions over 60 minutes', () => {
    render(
      <PaperDayInsights
        insights={makeInsights({
          firstVisitMs: 1,
          lastVisitMs: 1,
          peakHour: 0,
          longestSessionMs: 2 * 60 * 60_000 + 30 * 60_000, // 2h 30m
          topUrls: [],
        })}
        copy={COPY}
        language="en"
      />,
    )
    openDisclosure()
    // Locale-aware unit display joins hour + minute fragments.
    const longest = screen.getByText('Longest session').nextElementSibling
    expect(longest?.textContent).toMatch(/2.*hr.*30.*min|2h.*30m/i)
  })

  test('handles very short sessions by rounding up to a single minute', () => {
    render(
      <PaperDayInsights
        insights={makeInsights({
          firstVisitMs: 1,
          lastVisitMs: 1,
          peakHour: 0,
          longestSessionMs: 1000, // 1 second → 1 minute (rounded)
          topUrls: [],
        })}
        copy={COPY}
        language="en"
      />,
    )
    openDisclosure()
    const longest = screen.getByText('Longest session').nextElementSibling
    expect(longest?.textContent).toMatch(/1.*min|1m/i)
  })
})
