/**
 * @file discovery-trend-section.test.tsx
 * @description Render and unit tests for the discovery-trend card and its sparkline.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Verify loading, empty, ready, and sparkline rendering states.
 * - Verify the sparkline renders through the shared `Sparkline` primitive
 *   with its mean reference line + description, and that the percent bar
 *   is `aria-hidden` (its data is read from sibling text).
 * - Keep the discovery-trend card covered without mounting the full route.
 *
 * ## Not responsible for
 * - Re-testing backend discovery-trend calculations.
 * - Re-testing shared section metadata rendering.
 *
 * ## Dependencies
 * - Mocks `useAsyncData` so the card can be exercised deterministically.
 */

import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../../../lib/core-intelligence'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  DiscoveryTrend,
} from '../../../../lib/core-intelligence'
import type { DiscoveryTrendPoint } from '../../../../lib/core-intelligence/types-analysis'
import { DiscoveryTrendSection } from './discovery-trend-section'

const { useAsyncDataMock } = vi.hoisted(() => ({
  useAsyncDataMock: vi.fn(),
}))

vi.mock('../../../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()
  return {
    ...actual,
    useAsyncData: useAsyncDataMock,
  }
})

vi.mock('../../../../components/intelligence/section-meta', () => ({
  IntelligenceSectionMeta: ({ scopeLabel }: { scopeLabel: string }) => (
    <span>{scopeLabel}</span>
  ),
}))

vi.mock('./heuristics', () => ({
  humanizeDiscoveryWeekLabel: (dateKey: string) => `week:${dateKey}`,
}))

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

function testT(key: string, vars?: Record<string, string | number>) {
  if (vars) {
    return Object.entries(vars).reduce(
      (result, [k, v]) => result.replace(`{${k}}`, String(v)),
      key,
    )
  }
  return key
}

function meta(sectionId: string): CoreIntelligenceSectionMeta {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: ['test'],
    notes: [],
    sectionId,
    sourceTables: ['test_table'],
    state: 'ready',
    stateReason: null,
    window: { dateRange, kind: 'date-range' },
  }
}

function trendResult(
  data: DiscoveryTrend | null,
): CoreIntelligenceSectionResult<DiscoveryTrend | null> {
  return { data, meta: meta('discovery-trend') }
}

function makePoints(count: number): DiscoveryTrendPoint[] {
  return Array.from({ length: count }, (_, i) => ({
    dateKey: `2026-W${String(i + 10).padStart(2, '0')}`,
    discoveryRate: 0.05 + i * 0.03,
    newDomainCount: 10 + i * 5,
    totalVisits: 100 + i * 20,
  }))
}

function sectionNode() {
  return (
    <DiscoveryTrendSection
      dateRange={dateRange}
      profileId="chrome:Default"
      scopeLabel="Chrome"
      t={testT}
    />
  )
}

function renderSection() {
  return render(sectionNode())
}

describe('DiscoveryTrendSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders loading skeleton', () => {
    useAsyncDataMock.mockReturnValue({ data: null, loading: true })
    renderSection()

    expect(document.querySelector('.intelligence-skeleton')).toBeInTheDocument()
  })

  test('returns null when ready payload has no points', () => {
    useAsyncDataMock.mockReturnValue({
      data: trendResult({ points: [], availableYears: [] }),
      loading: false,
    })
    const { container } = renderSection()

    expect(container.innerHTML).toBe('')
  })

  test('renders empty state when data is not ready and has no points', () => {
    const pendingMeta = meta('discovery-trend')
    pendingMeta.state = 'stale'
    useAsyncDataMock.mockReturnValue({
      data: {
        data: { points: [], availableYears: [] },
        meta: pendingMeta,
      },
      loading: false,
    })
    renderSection()

    expect(screen.getByText('discoveryTrendEmpty')).toBeInTheDocument()
  })

  test('renders weekly rows without sparkline when fewer than 2 points', () => {
    useAsyncDataMock.mockReturnValue({
      data: trendResult({ points: makePoints(1), availableYears: [2026] }),
      loading: false,
    })
    renderSection()

    expect(screen.queryByTestId('discovery-sparkline')).toBeNull()
    expect(screen.getByText('week:2026-W10')).toBeInTheDocument()
  })

  test('renders sparkline SVG through the shared Sparkline primitive when 2+ points are available', () => {
    useAsyncDataMock.mockReturnValue({
      data: trendResult({ points: makePoints(4), availableYears: [2026] }),
      loading: false,
    })
    renderSection()

    const sparkline = screen.getByTestId('discovery-sparkline')
    expect(sparkline).toBeInTheDocument()
    expect(sparkline.tagName.toLowerCase()).toBe('svg')
    expect(sparkline.getAttribute('viewBox')).toBe('0 0 200 48')
    expect(
      screen.getByRole('img', {
        name: 'discoveryTrendSparklineLabel',
      }),
    ).toBe(sparkline)

    const polyline = sparkline.querySelector('polyline')
    expect(polyline).not.toBeNull()
    expect(polyline!.getAttribute('class')).toContain('stroke-accent')
    expect(polyline!.getAttribute('stroke-width')).toBe('1.5')

    const areaPath = sparkline.querySelector('path')
    expect(areaPath).not.toBeNull()
    expect(areaPath!.getAttribute('class')).toContain('opacity-10')

    // The dashed mean-rate reference line + its <title> description drive
    // through the same shared adornment props every Sparkline consumer uses.
    const meanLine = sparkline.querySelector('line')
    expect(meanLine).not.toBeNull()
    expect(meanLine!.getAttribute('stroke-dasharray')).toBe('4 3')
    // The test double `t()` doesn't interpolate `{percent}` into the raw
    // key (only real catalog templates do), so this simply confirms the
    // mean-rate `<title>` is wired to the same translator as the rest of
    // the card's copy.
    expect(sparkline.querySelector('title')?.textContent).toBe(
      'discoveryTrendMeanLabel',
    )
  })

  test('renders correct weekly rows alongside sparkline', () => {
    useAsyncDataMock.mockReturnValue({
      data: trendResult({ points: makePoints(3), availableYears: [2026] }),
      loading: false,
    })
    renderSection()

    expect(screen.getByText('week:2026-W12')).toBeInTheDocument()
    expect(screen.getByText('week:2026-W11')).toBeInTheDocument()
    expect(screen.getByText('week:2026-W10')).toBeInTheDocument()
  })

  test('marks the decorative discovery-rate percent bar aria-hidden', () => {
    useAsyncDataMock.mockReturnValue({
      data: trendResult({ points: makePoints(3), availableYears: [2026] }),
      loading: false,
    })
    const { container } = renderSection()

    const bars = container.querySelectorAll('.discovery-trend__bar')
    expect(bars.length).toBeGreaterThan(0)
    for (const bar of bars) {
      expect(bar).toHaveAttribute('aria-hidden', 'true')
    }
  })
})
