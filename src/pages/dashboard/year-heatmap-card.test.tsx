import { describe, expect, test, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nProvider } from '@/lib/i18n'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import { DashboardYearHeatmapCard } from './year-heatmap-card'

function makeTrendResult(
  points: Array<{ dateKey: string; totalVisits: number }>,
) {
  return {
    data: {
      points: points.map((point) => ({
        ...point,
        discoveryRate: 0,
        newDomainCount: 0,
      })),
      availableYears: [],
    },
    meta: { state: 'ready' as const },
  } as unknown as Awaited<
    ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>
  >
}

function renderCard(
  props: Partial<{
    archiveReady: boolean
    onOpenInsights: () => void
    onSelectDate: (date: string) => void
  }> = {},
) {
  return render(
    <ProfileScopeProvider>
      <I18nProvider>
        <DashboardYearHeatmapCard
          archiveReady
          onOpenInsights={vi.fn()}
          onSelectDate={vi.fn()}
          {...props}
        />
      </I18nProvider>
    </ProfileScopeProvider>,
  )
}

describe('DashboardYearHeatmapCard', () => {
  test('renders the honest empty copy when archive is not ready', () => {
    const spy = vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend')
    renderCard({ archiveReady: false })
    expect(spy).not.toHaveBeenCalled()
    expect(screen.getByTestId('dashboard-year-empty')).toBeInTheDocument()
  })

  test('renders the loading skeleton while the fetch is pending', () => {
    let resolveTrend: (
      value: Awaited<ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>>,
    ) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTrend = resolve
        }),
    )
    renderCard()
    expect(screen.getByTestId('dashboard-year-loading')).toBeInTheDocument()
    // Resolve to release the in-flight promise (avoids unhandled rejection).
    act(() => resolveTrend(makeTrendResult([])))
  })

  test('renders the error fallback when the fetch rejects', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockRejectedValue(
      new Error('boom'),
    )
    renderCard()
    expect(await screen.findByTestId('dashboard-year-error')).toHaveTextContent(
      'boom',
    )
  })

  test('renders the heatmap grid + streak label on a populated result', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      makeTrendResult([
        { dateKey: '2026-05-19', totalVisits: 12 },
        { dateKey: '2026-05-20', totalVisits: 18 },
      ]),
    )
    renderCard()
    expect(
      await screen.findByTestId('dashboard-year-heatmap-grid'),
    ).toBeInTheDocument()
    expect(screen.getByTestId('dashboard-year-streak')).toBeInTheDocument()
  })

  test('forwards a clicked non-zero day to onSelectDate', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      makeTrendResult([{ dateKey: '2026-05-19', totalVisits: 12 }]),
    )
    const onSelectDate = vi.fn()
    renderCard({ onSelectDate })
    const grid = await screen.findByTestId('dashboard-year-heatmap-grid')
    const cells = grid.querySelectorAll('button:not(:disabled)')
    expect(cells.length).toBeGreaterThan(0)
    const user = userEvent.setup()
    await user.click(cells[0] as HTMLButtonElement)
    expect(onSelectDate).toHaveBeenCalled()
  })

  test('falls back to the translated error key when the rejection is not an Error', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockRejectedValue(
      'string-rejection',
    )
    renderCard()
    expect(await screen.findByTestId('dashboard-year-error')).toHaveTextContent(
      'Could not load the year heatmap.',
    )
  })

  test('renders the "no streak" copy when the rolling window has no consecutive days', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      makeTrendResult([]),
    )
    renderCard()
    const streak = await screen.findByTestId('dashboard-year-streak')
    expect(streak).toHaveTextContent('No streak yet')
  })

  test('unmounting mid-flight with a rejected fetch skips the error setters', async () => {
    let rejectTrend: (reason: unknown) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectTrend = reject
        }),
    )
    const { unmount } = renderCard()
    unmount()
    // Flushing a rejection after unmount drives the
    // `cancelled === true` branch inside the catch at line 93.
    act(() => rejectTrend(new Error('boom')))
    await new Promise((resolve) => window.setTimeout(resolve, 10))
  })

  test('unmounting mid-flight skips the post-fetch setters (cancelled branch)', async () => {
    let resolveTrend: (
      value: Awaited<ReturnType<typeof coreIntelligenceApi.getDiscoveryTrend>>,
    ) => void = () => {}
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveTrend = resolve
        }),
    )
    const { unmount } = renderCard()
    unmount()
    // Flushing the promise after unmount drives the `cancelled === true`
    // branches at lines 83 / 93 / 102 of year-heatmap-card.tsx — they
    // bail out without touching React state.
    act(() => resolveTrend(makeTrendResult([])))
    await new Promise((resolve) => window.setTimeout(resolve, 10))
  })

  test('tolerates a malformed response with no points field (?? fallback)', async () => {
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue({
      // Intentionally missing `points` so the route's `result.data?.points ?? []`
      // (line 85 of year-heatmap-card.tsx) takes its falsy branch.
      data: {},
      meta: { state: 'ready' as const },
    } as never)
    renderCard()
    expect(
      await screen.findByTestId('dashboard-year-heatmap-grid'),
    ).toBeInTheDocument()
  })

  test('renders the heatmap in a zh-CN locale (covers the language ternary)', async () => {
    window.localStorage.setItem('pathkeep-language-preference', 'zh-CN')
    try {
      vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
        makeTrendResult([{ dateKey: '2026-05-19', totalVisits: 4 }]),
      )
      renderCard()
      // buildHeatmapCopy's `language === 'en' ? 'en-US' : language` ternary
      // (lines 191 + 195) takes the else branch when the resolved locale is
      // anything other than English.
      expect(
        await screen.findByTestId('dashboard-year-heatmap-grid'),
      ).toBeInTheDocument()
    } finally {
      window.localStorage.removeItem('pathkeep-language-preference')
    }
  })
})
