/**
 * @file browsing-rhythm-day-detail.test.tsx
 * @description Render coverage for the selected-day browsing-rhythm detail rail.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify loading, error, empty-detail, and linked top-site branches.
 * - Keep the selected-day detail chrome covered independently from the calendar owner.
 *
 * ## Not responsible for
 * - Re-testing async day-insight loading or calendar selection state.
 * - Re-testing the lower-level hour strip and activity proportion visualizers.
 *
 * ## Dependencies
 * - Uses a memory router because the detail rail renders day/domain links.
 *
 * ## Performance notes
 * - Pure render fixtures keep the day-detail contract cheap to exercise.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import type { DayInsights } from '../../lib/core-intelligence'
import { BrowsingRhythmDayDetail } from './browsing-rhythm-day-detail'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('BrowsingRhythmDayDetail', () => {
  test('renders loading and error states', () => {
    const { rerender } = renderDetail({ loading: true })

    expect(screen.getByText('rhythmDetailLoading')).toBeVisible()
    expect(
      screen.getAllByRole('link', { name: 'rhythmViewDetails' })[0],
    ).toHaveAttribute('href', '/intelligence/day/2026-04-25')

    rerender(detailNode({ error: 'day detail unavailable' }))
    expect(screen.getByText('day detail unavailable')).toBeVisible()
  })

  test('renders empty and linked detail branches', () => {
    const empty = dayInsightsFixture()
    const { rerender } = renderDetail({ detail: empty })

    expect(screen.getByText('rhythmDayNoHourlyData')).toBeVisible()
    expect(screen.getByText('rhythmDayNoSites')).toBeVisible()
    expect(screen.getByText('activityMixEmpty')).toBeVisible()

    rerender(
      detailNode({
        dayDomainHref: (domain, date) =>
          `/intelligence/domain/${domain}/day/${date}`,
        detail: dayInsightsFixture({
          activityMix: {
            categories: [
              { domainCategory: 'docs', share: 0.75, visitCount: 9 },
            ],
            changeVsPrevious: [],
          },
          hourlyActivity: [
            { hour: 0, visitCount: 0 },
            { hour: 9, visitCount: 3 },
          ],
          topSites: [
            {
              averageDailyVisits: 8,
              domainCategory: 'docs',
              displayName: null,
              registrableDomain: 'sqlite.org',
              uniqueDays: 1,
              uniqueUrls: 2,
              visitCount: 8,
            },
          ],
        }),
      }),
    )

    expect(screen.getByTestId('rhythm-hour-strip')).toBeVisible()
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org/day/2026-04-25',
    )
    expect(screen.getByText('category_docs')).toBeVisible()

    rerender(
      detailNode({
        detail: dayInsightsFixture({
          topSites: [
            {
              averageDailyVisits: 3,
              domainCategory: 'docs',
              displayName: 'SQLite docs',
              registrableDomain: 'sqlite.org',
              uniqueDays: 1,
              uniqueUrls: 1,
              visitCount: 3,
            },
          ],
        }),
      }),
    )
    expect(screen.getByText('SQLite docs')).toBeVisible()
    expect(screen.queryByRole('link', { name: 'SQLite docs' })).toBeNull()
  })
})

function renderDetail(
  props: Partial<Parameters<typeof BrowsingRhythmDayDetail>[0]> = {},
) {
  return render(detailNode(props))
}

function detailNode(
  props: Partial<Parameters<typeof BrowsingRhythmDayDetail>[0]> = {},
) {
  return (
    <MemoryRouter>
      <BrowsingRhythmDayDetail
        dateKey="2026-04-25"
        dayHref={(date) => `/intelligence/day/${date}`}
        detail={null}
        error={null}
        language="en"
        loading={false}
        t={t}
        {...props}
      />
    </MemoryRouter>
  )
}

function dayInsightsFixture(overrides: Partial<DayInsights> = {}): DayInsights {
  return {
    activityMix: {
      categories: [],
      changeVsPrevious: [],
    },
    date: '2026-04-25',
    digestSummary: {
      dateRange: { start: '2026-04-25', end: '2026-04-25' },
      deepReadPages: { trend: 'flat', value: 1 },
      newDomains: { trend: 'flat', value: 2 },
      refindPages: { trend: 'flat', value: 0 },
      totalSearches: { trend: 'flat', value: 3 },
      totalVisits: { trend: 'flat', value: 12 },
    },
    drilldown: {
      explorerDateRange: { start: '2026-04-25', end: '2026-04-25' },
    },
    hourlyActivity: [],
    queryFamilies: {
      families: [],
      page: 0,
      pageSize: 10,
      total: 0,
    },
    refindPages: [],
    topSites: [],
    ...overrides,
  }
}
