/**
 * Year in Review narrative summary page.
 *
 * ## Responsibilities
 * - Render a Spotify Wrapped-style annual browsing summary at
 *   `/intelligence/year/:year` with scroll-snap narrative flow.
 * - Fetch data for the requested calendar year using existing Core
 *   Intelligence API functions with a year-scoped DateRange.
 * - Show honest empty / loading / partial-year states.
 *
 * ## Not responsible for
 * - Inventing new backend commands — reuses existing deterministic APIs.
 * - Owning date-range computation (delegates to `dateRangeForCalendarYear`).
 * - Owning heatmap cell bucketing (delegates to `buildYearHeatmapCells`).
 *
 * ## Dependencies
 * - Core Intelligence API (`getDigestSummary`, `getTopSites`,
 *   `getQueryFamilies`, `getDiscoveryTrend`, `getActivityMix`,
 *   `getHabitPatterns`, `getRefindPages`).
 * - `YearHeatmap` presentational component.
 * - `PaperCard` / `PaperCardHeader` / `PaperCardBody` for section surfaces.
 * - `year-review-copy.ts` for three-language display copy.
 *
 * ## Performance notes
 * - Seven parallel API fetches on mount; each is a deterministic read that
 *   the backend handles in ~50ms. No full-scan or unbounded loads.
 * - Heatmap renders 365 cells — no virtualization needed.
 * - IntersectionObserver drives fade-in per section; no scroll listener.
 */

import './styles/year-review.css'

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import {
  YearHeatmap,
  type YearHeatmapCopy,
} from '@/components/heatmap/year-heatmap'
import {
  buildYearHeatmapCells,
  type DailyVisitPoint,
} from '@/components/heatmap/year-heatmap-helpers'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { dateRangeForCalendarYear } from '@/lib/core-intelligence/hooks'
import type {
  ActivityMix,
  DigestSummary,
  DiscoveryTrend,
  HabitPattern,
  QueryFamilyResult,
  RefindPage,
  TopSite,
} from '@/lib/core-intelligence/types'
import { dayInsightsHref } from '@/lib/core-intelligence/routes'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n/hooks'
import { useProfileScope } from '@/lib/profile-scope-context'
import { yearReviewText } from './year-review-copy'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface YearReviewData {
  digest: DigestSummary | null
  topSites: TopSite[]
  queryFamilies: QueryFamilyResult | null
  discoveryTrend: DiscoveryTrend | null
  activityMix: ActivityMix | null
  habits: HabitPattern[]
  refindPages: RefindPage[]
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CATEGORY_COLORS = [
  'var(--accent)',
  '#6366f1',
  '#ec4899',
  '#f59e0b',
  '#10b981',
  '#8b5cf6',
  '#ef4444',
  '#06b6d4',
  '#84cc16',
  '#f97316',
]

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function YearReviewPage() {
  const params = useParams<{ year: string }>()
  const navigate = useNavigate()
  const { language } = useI18n()
  const { activeProfileId } = useProfileScope()

  const currentYear = new Date().getFullYear()
  const year = Number(params.year) || currentYear
  const isCurrentYear = year === currentYear
  const dateRange = useMemo(() => dateRangeForCalendarYear(year), [year])

  const [data, setData] = useState<YearReviewData | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const text = useCallback(
    (
      key: Parameters<typeof yearReviewText>[1],
      vars?: Record<string, string | number>,
    ) => yearReviewText(language, key, vars),
    [language],
  )

  // ---- Data fetching ----
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)

    void (async () => {
      try {
        const [
          digestResult,
          topSitesResult,
          queryFamiliesResult,
          discoveryResult,
          activityMixResult,
          habitsResult,
          refindResult,
        ] = await Promise.all([
          coreIntelligenceApi.getDigestSummary(dateRange, activeProfileId),
          coreIntelligenceApi.getTopSites(
            dateRange,
            activeProfileId,
            undefined,
            5,
          ),
          coreIntelligenceApi.getQueryFamilies(dateRange, activeProfileId, {
            page: 0,
            pageSize: 3,
          }),
          coreIntelligenceApi.getDiscoveryTrend(
            dateRange,
            activeProfileId,
            'day',
          ),
          coreIntelligenceApi.getActivityMix(dateRange, activeProfileId),
          coreIntelligenceApi.getHabitPatterns(dateRange, activeProfileId),
          coreIntelligenceApi.getRefindPages(dateRange, activeProfileId, 5),
        ])
        if (cancelled) return
        setData({
          digest: digestResult.data,
          topSites: topSitesResult.data,
          queryFamilies: queryFamiliesResult.data,
          discoveryTrend: discoveryResult.data,
          activityMix: activityMixResult.data,
          habits: habitsResult.data,
          refindPages: refindResult.data,
        })
      } catch (err) {
        if (!cancelled) {
          setError(describeError(err, 'year_review_fetch'))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [dateRange, activeProfileId])

  // ---- Year pager ----
  const goToYear = (targetYear: number) => {
    void navigate(`/intelligence/year/${targetYear}`)
  }

  // ---- Loading state ----
  if (loading) {
    return (
      <div className="year-review__loading" data-testid="year-review-loading">
        <p className="font-serif text-[14px] italic text-ink-muted">
          {text('loading')}
        </p>
      </div>
    )
  }

  // ---- Error state ----
  if (error) {
    return (
      <div className="year-review__empty" data-testid="year-review-error">
        <h2 className="year-review__empty-title">
          {text('emptyTitle', { year })}
        </h2>
        <p className="year-review__empty-body">{error}</p>
      </div>
    )
  }

  // ---- Empty state ----
  if (!data || !data.digest || data.digest.totalVisits.value <= 0) {
    return (
      <div className="year-review__empty" data-testid="year-review-empty">
        <YearPager
          year={year}
          currentYear={currentYear}
          onGoToYear={goToYear}
          text={text}
        />
        <h2 className="year-review__empty-title">
          {text('emptyTitle', { year })}
        </h2>
        <p className="year-review__empty-body">{text('emptyBody')}</p>
      </div>
    )
  }

  return (
    <div className="year-review" data-testid="year-review">
      <HeroSection
        year={year}
        isCurrentYear={isCurrentYear}
        currentYear={currentYear}
        digest={data.digest}
        onGoToYear={goToYear}
        text={text}
      />
      <VolumeSection
        year={year}
        discoveryTrend={data.discoveryTrend}
        language={language}
        text={text}
      />
      <PodiumSection topSites={data.topSites} text={text} />
      <ResearchSection queryFamilies={data.queryFamilies} text={text} />
      <DiscoverySection
        discoveryTrend={data.discoveryTrend}
        language={language}
        text={text}
      />
      <ContentMixSection activityMix={data.activityMix} text={text} />
      <HabitsSection habits={data.habits} text={text} />
      <RefindSection refindPages={data.refindPages} text={text} />
      <footer className="year-review__footer">
        <button
          type="button"
          className="year-review__footer-link"
          onClick={() =>
            void navigate(
              `/intelligence?range=custom&start=${dateRange.start}&end=${dateRange.end}`,
            )
          }
          data-testid="year-review-footer-cta"
        >
          {text('footerCta')} →
        </button>
      </footer>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

type TextFn = (
  key: Parameters<typeof yearReviewText>[1],
  vars?: Record<string, string | number>,
) => string

function YearPager({
  year,
  currentYear,
  onGoToYear,
  text,
}: {
  year: number
  currentYear: number
  onGoToYear: (y: number) => void
  text: TextFn
}) {
  return (
    <nav className="year-review__year-pager" data-testid="year-review-pager">
      <button
        type="button"
        onClick={() => onGoToYear(year - 1)}
        aria-label={text('yearPagerPrev')}
      >
        ←
      </button>
      <span>{year}</span>
      <button
        type="button"
        onClick={() => onGoToYear(year + 1)}
        disabled={year >= currentYear}
        aria-label={text('yearPagerNext')}
      >
        →
      </button>
    </nav>
  )
}

function HeroSection({
  year,
  isCurrentYear,
  currentYear,
  digest,
  onGoToYear,
  text,
}: {
  year: number
  isCurrentYear: boolean
  currentYear: number
  digest: DigestSummary
  onGoToYear: (y: number) => void
  text: TextFn
}) {
  const titleKey = isCurrentYear ? 'heroTitleSoFar' : 'heroTitle'
  return (
    <section
      className="year-review__section year-review__section--hero"
      data-testid="year-review-hero"
    >
      <YearPager
        year={year}
        currentYear={currentYear}
        onGoToYear={onGoToYear}
        text={text}
      />
      <h1 className="year-review__hero-title">{text(titleKey, { year })}</h1>
      <div className="year-review__hero-stats">
        <div className="year-review__hero-stat">
          <span
            className="year-review__hero-stat-value"
            data-testid="yr-stat-visits"
          >
            {compactNumber(digest.totalVisits.value)}
          </span>
          <span className="year-review__hero-stat-label">
            {text('statTotalVisits')}
          </span>
        </div>
        <div className="year-review__hero-stat">
          <span
            className="year-review__hero-stat-value"
            data-testid="yr-stat-new-domains"
          >
            {compactNumber(digest.newDomains.value)}
          </span>
          <span className="year-review__hero-stat-label">
            {text('statNewDomains')}
          </span>
        </div>
        <div className="year-review__hero-stat">
          <span
            className="year-review__hero-stat-value"
            data-testid="yr-stat-deep-reads"
          >
            {compactNumber(digest.deepReadPages.value)}
          </span>
          <span className="year-review__hero-stat-label">
            {text('statDeepReads')}
          </span>
        </div>
      </div>
    </section>
  )
}

function VolumeSection({
  year,
  discoveryTrend,
  language,
  text,
}: {
  year: number
  discoveryTrend: DiscoveryTrend | null
  language: string
  text: TextFn
}) {
  const navigate = useNavigate()

  const points: DailyVisitPoint[] = useMemo(
    () =>
      (discoveryTrend?.points ?? []).map((p) => ({
        dateKey: p.dateKey,
        totalVisits: p.totalVisits,
      })),
    [discoveryTrend],
  )

  const startDate = useMemo(() => new Date(year, 0, 1), [year])
  const daysInYear =
    year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 366 : 365
  const cells = useMemo(
    () => buildYearHeatmapCells(points, startDate, daysInYear),
    [points, startDate, daysInYear],
  )

  const activeDays = useMemo(
    () => cells.filter((c) => c.count > 0).length,
    [cells],
  )
  const busiestDay = useMemo(() => {
    /* v8 ignore next -- buildYearHeatmapCells always emits a full calendar year, so cells is never empty; the guard is defensive. */
    if (cells.length === 0) return null
    let best = cells[0]
    for (const cell of cells) {
      if (cell.count > best.count) best = cell
    }
    return best
  }, [cells])

  const heatmapCopy = useMemo<YearHeatmapCopy>(() => {
    const locale = language === 'en' ? 'en-US' : language
    const monthFmt = new Intl.DateTimeFormat(locale, { month: 'short' })
    const dayFmt = new Intl.DateTimeFormat(locale, { weekday: 'short' })
    const monthLabels = Array.from({ length: 12 }, (_, i) =>
      monthFmt.format(new Date(year, i, 1)),
    ) as YearHeatmapCopy['monthLabels']
    const dayLabels = Array.from({ length: 7 }, (_, i) =>
      dayFmt.format(new Date(2025, 11, 28 + i)),
    ) as YearHeatmapCopy['dayLabels']
    return {
      legendLess: text('heatmapLess'),
      legendMore: text('heatmapMore'),
      monthLabels,
      dayLabels,
      cellTooltip: (date, count) => `${date}: ${count}`,
    }
  }, [language, year, text])

  return (
    <FadeInSection testId="year-review-volume">
      <PaperCard>
        <PaperCardHeader title={text('volumeHeading')} compact />
        <PaperCardBody className="px-[18px] pb-[16px] pt-[12px]">
          <div className="mb-5">
            <YearHeatmap
              cells={cells}
              copy={heatmapCopy}
              onSelectDate={(date) => void navigate(dayInsightsHref(date))}
              testId="year-review-heatmap"
            />
          </div>
          {busiestDay && busiestDay.count > 0 ? (
            <p className="year-review__section-body">
              {text('volumeBusiestDay', {
                date: busiestDay.date,
                count: busiestDay.count,
              })}
            </p>
          ) : null}
          <p className="year-review__section-body" style={{ marginBottom: 0 }}>
            {text('volumeActiveDays', {
              count: activeDays,
              total: daysInYear,
            })}
          </p>
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function PodiumSection({
  topSites,
  text,
}: {
  topSites: TopSite[]
  text: TextFn
}) {
  if (topSites.length === 0) return null
  return (
    <FadeInSection testId="year-review-podium">
      <PaperCard>
        <PaperCardHeader title={text('podiumHeading')} compact />
        <PaperCardBody>
          <div className="year-review__podium">
            {topSites.slice(0, 5).map((site, index) => (
              <div
                key={site.registrableDomain}
                className="year-review__podium-entry"
              >
                <span className="year-review__podium-rank">{index + 1}</span>
                <span className="year-review__podium-name">
                  {site.displayName ?? site.registrableDomain}
                </span>
                <span className="year-review__podium-count">
                  {text('podiumVisits', { count: site.visitCount })}
                </span>
              </div>
            ))}
          </div>
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function ResearchSection({
  queryFamilies,
  text,
}: {
  queryFamilies: QueryFamilyResult | null
  text: TextFn
}) {
  if (!queryFamilies || queryFamilies.total === 0) return null
  return (
    <FadeInSection testId="year-review-research">
      <PaperCard>
        <PaperCardHeader title={text('researchHeading')} compact />
        <PaperCardBody>
          <p className="year-review__section-body">
            {text('researchJourneys', { count: queryFamilies.total })}
          </p>
          {queryFamilies.families.slice(0, 3).map((family) => (
            <div key={family.familyId} className="year-review__query-chain">
              <span className="year-review__query-chain-anchor">
                {family.anchorQuery}
              </span>
              {family.queries
                .filter((q) => q !== family.anchorQuery)
                .slice(0, 3)
                .map((q) => (
                  <span key={q} className="year-review__query-chain-member">
                    → {q}
                  </span>
                ))}
            </div>
          ))}
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function DiscoverySection({
  discoveryTrend,
  language,
  text,
}: {
  discoveryTrend: DiscoveryTrend | null
  language: string
  text: TextFn
}) {
  const points = discoveryTrend?.points ?? []
  if (points.length === 0) return null

  const totalNewDomains = points.reduce((sum, p) => sum + p.newDomainCount, 0)

  // Find the month with the most new domains
  const monthBuckets = new Map<string, number>()
  for (const point of points) {
    const month = point.dateKey.slice(0, 7) // YYYY-MM
    monthBuckets.set(
      month,
      (monthBuckets.get(month) ?? 0) + point.newDomainCount,
    )
  }
  let bestMonth = ''
  let bestCount = 0
  for (const [month, count] of monthBuckets) {
    if (count > bestCount) {
      bestMonth = month
      bestCount = count
    }
  }

  const locale = language === 'en' ? 'en-US' : language
  const monthFmt = new Intl.DateTimeFormat(locale, {
    month: 'long',
    year: 'numeric',
  })
  const bestMonthLabel = bestMonth
    ? monthFmt.format(
        new Date(
          Number(bestMonth.slice(0, 4)),
          Number(bestMonth.slice(5, 7)) - 1,
          1,
        ),
      )
    : ''

  // Top 5 newly discovered domains from the discovery trend points
  // (the discovery trend gives us per-day new domain counts, but not the
  // domain names themselves — show just the aggregate count).

  return (
    <FadeInSection testId="year-review-discovery">
      <PaperCard>
        <PaperCardHeader title={text('discoveryHeading')} compact />
        <PaperCardBody>
          <p className="year-review__section-body">
            {text('discoveryNewSites', { count: totalNewDomains })}
          </p>
          {bestMonthLabel ? (
            <p className="year-review__section-body">
              {text('discoveryExploratory', { month: bestMonthLabel })}
            </p>
          ) : null}
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function ContentMixSection({
  activityMix,
  text,
}: {
  activityMix: ActivityMix | null
  text: TextFn
}) {
  if (!activityMix || activityMix.categories.length === 0) return null
  const sorted = [...activityMix.categories].sort(
    (a, b) => b.visitCount - a.visitCount,
  )
  return (
    <FadeInSection testId="year-review-mix">
      <PaperCard>
        <PaperCardHeader title={text('mixHeading')} compact />
        <PaperCardBody>
          <div className="year-review__mix-bar">
            {sorted.map((cat, index) => (
              <div
                key={cat.domainCategory}
                className="year-review__mix-segment"
                style={{
                  flex: cat.share,
                  backgroundColor:
                    CATEGORY_COLORS[index % CATEGORY_COLORS.length],
                }}
                title={`${cat.domainCategory}: ${(cat.share * 100).toFixed(1)}%`}
                role="img"
                aria-label={`${cat.domainCategory}: ${(cat.share * 100).toFixed(1)}%`}
              />
            ))}
          </div>
          <div className="year-review__mix-legend">
            {sorted.slice(0, 8).map((cat, index) => (
              <span
                key={cat.domainCategory}
                className="year-review__mix-legend-item"
              >
                <span
                  className="year-review__mix-legend-dot"
                  style={{
                    backgroundColor:
                      CATEGORY_COLORS[index % CATEGORY_COLORS.length],
                  }}
                />
                {cat.domainCategory} ({(cat.share * 100).toFixed(0)}%)
              </span>
            ))}
          </div>
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function HabitsSection({
  habits,
  text,
}: {
  habits: HabitPattern[]
  text: TextFn
}) {
  if (habits.length === 0) return null
  const habitTypeLabel = (type: HabitPattern['habitType']) => {
    switch (type) {
      case 'daily_habit':
        return text('habitsDaily')
      case 'weekly_habit':
        return text('habitsWeekly')
      case 'periodic_reference':
        return text('habitsPeriodic')
    }
  }
  return (
    <FadeInSection testId="year-review-habits">
      <PaperCard>
        <PaperCardHeader title={text('habitsHeading')} compact />
        <PaperCardBody>
          <div className="year-review__compact-list">
            {habits.slice(0, 8).map((habit) => (
              <div
                key={habit.registrableDomain}
                className="year-review__compact-item"
              >
                <span className="year-review__compact-item-name">
                  {habit.displayName ?? habit.registrableDomain}
                </span>
                <span className="year-review__compact-item-detail">
                  {habitTypeLabel(habit.habitType)}
                </span>
              </div>
            ))}
          </div>
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

function RefindSection({
  refindPages,
  text,
}: {
  refindPages: RefindPage[]
  text: TextFn
}) {
  if (refindPages.length === 0) return null
  return (
    <FadeInSection testId="year-review-refind">
      <PaperCard>
        <PaperCardHeader title={text('refindHeading')} compact />
        <PaperCardBody>
          <div className="year-review__compact-list">
            {refindPages.slice(0, 5).map((page) => (
              <div
                key={page.canonicalUrl}
                className="year-review__compact-item"
              >
                <span className="year-review__compact-item-name">
                  {page.title ?? page.url}
                </span>
                <span className="year-review__compact-item-detail">
                  {text('refindRevisits', { count: page.crossDayCount })}
                </span>
              </div>
            ))}
          </div>
        </PaperCardBody>
      </PaperCard>
    </FadeInSection>
  )
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/**
 * Wraps a section in an IntersectionObserver-driven fade-in.
 */
function FadeInSection({
  children,
  testId,
}: {
  children: React.ReactNode
  testId: string
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [visible, setVisible] = useState(false)

  useEffect(() => {
    const el = ref.current
    /* v8 ignore next -- defensive: the ref is bound to the always-rendered <section>, so ref.current is populated whenever this effect runs; the null-ref early return is unreachable. */
    if (!el) return
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true)
          observer.unobserve(el)
        }
      },
      { threshold: 0.15 },
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  return (
    <section
      ref={ref}
      className={`year-review__section${visible ? ' year-review__section--visible' : ''}`}
      data-testid={testId}
    >
      {children}
    </section>
  )
}

function compactNumber(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}
