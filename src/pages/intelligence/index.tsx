/**
 * Intelligence page — Core Intelligence dashboard for non-LLM deterministic analysis.
 *
 * Why this file exists:
 * - This is the primary Intelligence route (/intelligence) replacing the old /insights.
 * - Displays digest summary cards, top sites, search activity, refind pages,
 *   browsing rhythm, activity mix, and more — all powered by pre-computed rollups.
 * - All data comes from `derived/history-intelligence.sqlite` via Tauri commands.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §2.2 (Intelligence 主頁 ASCII 佈局)
 * - `docs/design/screens-and-nav.md`
 * - `docs/design/design-tokens.md`
 */

import './intelligence.css'

import { useState } from 'react'
import { useI18n } from '../../lib/i18n/hooks'
import {
  useTimeRange,
  useAsyncData,
  type DateRange,
  type TimeRangePreset,
  type EngineRanking,
  type SearchConcept,
  type KpiMetric,
  type RefindPage,
  type QueryFamily,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'

// ---------------------------------------------------------------------------
// Exported page component
// ---------------------------------------------------------------------------

export function IntelligencePage() {
  const { t } = useI18n('intelligence')
  const { preset, dateRange, setPreset, setCustomRange } = useTimeRange('month')

  return (
    <div className="intelligence-page">
      <TimeRangeBar
        preset={preset}
        dateRange={dateRange}
        onPresetChange={setPreset}
        onCustomRange={setCustomRange}
        t={t}
      />

      <div className="intelligence-grid">
        <DigestSection dateRange={dateRange} t={t} />
        <div className="intelligence-row intelligence-row--two-col">
          <OnThisDaySection t={t} />
          <TopSitesSection dateRange={dateRange} t={t} />
        </div>
        <SearchActivitySection dateRange={dateRange} t={t} />
        <RefindPagesSection dateRange={dateRange} t={t} />
        <div className="intelligence-row intelligence-row--two-col">
          <ActivityMixSection dateRange={dateRange} t={t} />
          <BrowsingRhythmSection dateRange={dateRange} t={t} />
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

type T = (key: string, vars?: Record<string, string | number>) => string

// --- Time Range Bar ---

function TimeRangeBar({
  preset,
  dateRange,
  onPresetChange,
  onCustomRange,
  t,
}: {
  preset: TimeRangePreset
  dateRange: DateRange
  onPresetChange: (p: TimeRangePreset) => void
  onCustomRange: (r: DateRange) => void
  t: T
}) {
  const presets: { key: TimeRangePreset; label: string }[] = [
    { key: 'day', label: t('rangeDay') },
    { key: 'week', label: t('rangeWeek') },
    { key: 'month', label: t('rangeMonth') },
    { key: 'quarter', label: t('rangeQuarter') },
    { key: 'year', label: t('rangeYear') },
    { key: 'custom', label: t('rangeCustom') },
  ]

  const [showCustom, setShowCustom] = useState(false)
  const [customStart, setCustomStart] = useState(dateRange.start)
  const [customEnd, setCustomEnd] = useState(dateRange.end)

  return (
    <div
      className="time-range-bar"
      role="toolbar"
      aria-label={t('timeRangeLabel')}
    >
      <div className="time-range-bar__presets">
        {presets.map(({ key, label }) => (
          <button
            key={key}
            className={`time-range-bar__btn${preset === key ? ' time-range-bar__btn--active' : ''}`}
            onClick={() => {
              if (key === 'custom') {
                setShowCustom(true)
              } else {
                setShowCustom(false)
                onPresetChange(key)
              }
            }}
            aria-pressed={preset === key}
          >
            {label}
          </button>
        ))}
      </div>
      {showCustom && (
        <div className="time-range-bar__custom">
          <input
            type="date"
            value={customStart}
            onChange={(e) => setCustomStart(e.target.value)}
            aria-label={t('customStart')}
          />
          <span className="time-range-bar__separator">–</span>
          <input
            type="date"
            value={customEnd}
            onChange={(e) => setCustomEnd(e.target.value)}
            aria-label={t('customEnd')}
          />
          <button
            className="time-range-bar__apply"
            onClick={() =>
              onCustomRange({ start: customStart, end: customEnd })
            }
          >
            {t('applyRange')}
          </button>
        </div>
      )}
    </div>
  )
}

// --- Digest Summary Cards ---

function DigestSection({ dateRange, t }: { dateRange: DateRange; t: T }) {
  const { data, loading, error } = useAsyncData(
    () => api.getDigestSummary(dateRange),
    [dateRange],
  )

  if (loading) {
    return (
      <section className="intelligence-section digest-section">
        <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
        <div className="digest-cards">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="digest-card digest-card--skeleton">
              <div className="digest-card__value-skeleton" />
              <div className="digest-card__label-skeleton" />
            </div>
          ))}
        </div>
      </section>
    )
  }

  if (error || !data) {
    return (
      <section className="intelligence-section digest-section">
        <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || t('digestUnavailable')}
          </p>
        </div>
      </section>
    )
  }

  const cards: { icon: string; label: string; metric: KpiMetric }[] = [
    { icon: '📊', label: t('digestVisits'), metric: data.totalVisits },
    { icon: '🔍', label: t('digestSearches'), metric: data.totalSearches },
    { icon: '🌐', label: t('digestNewSites'), metric: data.newDomains },
    { icon: '📖', label: t('digestDeepRead'), metric: data.deepReadPages },
    { icon: '🔄', label: t('digestRefind'), metric: data.refindPages },
  ]

  return (
    <section className="intelligence-section digest-section">
      <h2 className="intelligence-section__title">{t('digestTitle')}</h2>
      <div className="digest-cards">
        {cards.map(({ icon, label, metric }) => (
          <div key={label} className="digest-card">
            <span className="digest-card__icon">{icon}</span>
            <span className="digest-card__value">
              {formatNumber(metric.value)}
            </span>
            <span className="digest-card__label">{label}</span>
            <TrendBadge metric={metric} t={t} />
          </div>
        ))}
      </div>
    </section>
  )
}

function TrendBadge({ metric, t }: { metric: KpiMetric; t: T }) {
  if (metric.changePercent == null) return null
  const arrow =
    metric.trend === 'up' ? '↑' : metric.trend === 'down' ? '↓' : '='
  const sign = metric.changePercent > 0 ? '+' : ''
  return (
    <span
      className={`trend-badge trend-badge--${metric.trend}`}
      aria-label={t('trendLabel', {
        direction: metric.trend,
        percent: Math.abs(metric.changePercent),
      })}
    >
      {sign}
      {Math.round(metric.changePercent)}% {arrow}
    </span>
  )
}

// --- On This Day ---

function OnThisDaySection({ t }: { t: T }) {
  const { data, loading } = useAsyncData(() => api.getOnThisDay(), [])

  return (
    <section className="intelligence-section on-this-day-section">
      <h2 className="intelligence-section__title">{t('onThisDayTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__eyebrow">{t('onThisDayEyebrow')}</p>
          <p className="intelligence-empty__text">{t('onThisDayEmpty')}</p>
        </div>
      ) : (
        <div className="on-this-day-list">
          {data.slice(0, 3).map((entry) => (
            <div key={entry.year} className="on-this-day-entry">
              <span className="on-this-day-entry__date">📅 {entry.date}</span>
              <span className="on-this-day-entry__summary">
                {entry.summary ??
                  t('onThisDayVisits', { count: entry.totalVisits })}
              </span>
            </div>
          ))}
          {data.length > 3 && (
            <button className="intelligence-link">{t('onThisDayMore')}</button>
          )}
        </div>
      )}
    </section>
  )
}

// --- Top Sites ---

function TopSitesSection({ dateRange, t }: { dateRange: DateRange; t: T }) {
  const [sortBy, setSortBy] = useState('visit_count')
  const [search, setSearch] = useState('')

  const { data, loading } = useAsyncData(
    () => api.getTopSites(dateRange, null, sortBy, 20),
    [dateRange, sortBy],
  )

  const filteredData = data?.filter((site) => {
    if (!search.trim()) return true
    const needle = search.toLowerCase()
    return (
      site.registrableDomain.toLowerCase().includes(needle) ||
      (site.displayName?.toLowerCase().includes(needle) ?? false)
    )
  })

  return (
    <section className="intelligence-section top-sites-section">
      <h2 className="intelligence-section__title">{t('topSitesTitle')}</h2>
      <div className="top-sites-controls">
        <input
          className="top-sites-controls__search"
          type="search"
          placeholder={t('topSitesSearch')}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label={t('topSitesSearch')}
        />
        <select
          className="top-sites-controls__sort"
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value)}
          aria-label={t('topSitesSort')}
        >
          <option value="visit_count">{t('topSitesSortVisits')}</option>
          <option value="unique_days">{t('topSitesSortDays')}</option>
          <option value="avg_daily">{t('topSitesSortAvg')}</option>
        </select>
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !filteredData || filteredData.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('topSitesEmpty')}</p>
        </div>
      ) : (
        <div className="top-sites-list">
          {filteredData.map((site, i) => {
            const maxVal =
              sortBy === 'unique_days'
                ? filteredData[0].uniqueDays
                : sortBy === 'avg_daily'
                  ? filteredData[0].averageDailyVisits
                  : filteredData[0].visitCount
            const val =
              sortBy === 'unique_days'
                ? site.uniqueDays
                : sortBy === 'avg_daily'
                  ? site.averageDailyVisits
                  : site.visitCount
            const displayVal =
              sortBy === 'avg_daily' ? val.toFixed(1) : formatNumber(val)
            const suffix =
              sortBy === 'unique_days'
                ? t('topSitesDays')
                : sortBy === 'avg_daily'
                  ? t('topSitesAvgSuffix')
                  : t('visits')
            return (
              <div key={site.registrableDomain} className="top-site-row">
                <span className="top-site-row__rank">{i + 1}.</span>
                <span className="top-site-row__domain">
                  {site.displayName ?? site.registrableDomain}
                </span>
                <span className="top-site-row__bar">
                  <span
                    className="top-site-row__bar-fill"
                    style={{
                      width: `${maxVal > 0 ? Math.round((val / maxVal) * 100) : 0}%`,
                    }}
                  />
                </span>
                <span className="top-site-row__count">
                  {displayVal} {suffix}
                </span>
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}

// --- Search Activity ---

function SearchActivitySection({
  dateRange,
  t,
}: {
  dateRange: DateRange
  t: T
}) {
  const [tab, setTab] = useState<'engines' | 'concepts' | 'families'>('engines')

  const engines = useAsyncData(
    () => api.getSearchEngineRanking(dateRange),
    [dateRange],
  )
  const concepts = useAsyncData(
    () => api.getTopSearchConcepts(dateRange, null, 50),
    [dateRange],
  )

  return (
    <section className="intelligence-section search-activity-section">
      <h2 className="intelligence-section__title">
        {t('searchActivityTitle')}
      </h2>
      <div className="intelligence-tabs" role="tablist">
        {(['engines', 'concepts', 'families'] as const).map((key) => (
          <button
            key={key}
            role="tab"
            aria-selected={tab === key}
            className={`intelligence-tab${tab === key ? ' intelligence-tab--active' : ''}`}
            onClick={() => setTab(key)}
          >
            {t(`searchTab_${key}`)}
          </button>
        ))}
      </div>

      <div className="intelligence-tab-content">
        {tab === 'engines' && (
          <EngineRankingPanel
            data={engines.data}
            loading={engines.loading}
            t={t}
          />
        )}
        {tab === 'concepts' && (
          <ConceptCloudPanel
            data={concepts.data}
            loading={concepts.loading}
            t={t}
          />
        )}
        {tab === 'families' && (
          <QueryFamiliesPanel dateRange={dateRange} t={t} />
        )}
      </div>
    </section>
  )
}

function EngineRankingPanel({
  data,
  loading,
  t,
}: {
  data: EngineRanking[] | null
  loading: boolean
  t: T
}) {
  if (loading)
    return <div className="intelligence-skeleton intelligence-skeleton--bar" />
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('engineRankingEmpty')}</p>
      </div>
    )
  }

  const max = data[0].searchCount
  return (
    <div className="engine-ranking">
      {data.map((engine) => (
        <div key={engine.searchEngine} className="engine-ranking__row">
          <span className="engine-ranking__name">
            {engine.displayName ?? engine.searchEngine}
          </span>
          <span className="engine-ranking__bar">
            <span
              className="engine-ranking__bar-fill"
              style={{
                width: `${Math.round((engine.searchCount / max) * 100)}%`,
              }}
            />
          </span>
          <span className="engine-ranking__count">
            {formatNumber(engine.searchCount)}
          </span>
        </div>
      ))}
    </div>
  )
}

function ConceptCloudPanel({
  data,
  loading,
  t,
}: {
  data: SearchConcept[] | null
  loading: boolean
  t: T
}) {
  if (loading)
    return (
      <div className="intelligence-skeleton intelligence-skeleton--cloud" />
    )
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('conceptCloudEmpty')}</p>
      </div>
    )
  }

  const maxFreq = Math.max(...data.map((c) => c.frequency))
  return (
    <div
      className="concept-cloud"
      role="img"
      aria-label={t('conceptCloudLabel')}
    >
      {data.map((concept) => {
        const scale = 0.6 + (concept.frequency / maxFreq) * 1.4
        return (
          <span
            key={concept.term}
            className="concept-cloud__term"
            style={{ fontSize: `${scale}em` }}
            title={t('conceptTooltip', {
              term: concept.term,
              count: concept.frequency,
            })}
          >
            {concept.term}
          </span>
        )
      })}
    </div>
  )
}

// --- Refind Pages ---

function RefindPagesSection({ dateRange, t }: { dateRange: DateRange; t: T }) {
  const { data, loading } = useAsyncData(
    () => api.getRefindPages(dateRange, null, 5),
    [dateRange],
  )

  return (
    <section className="intelligence-section refind-section">
      <h2 className="intelligence-section__title">{t('refindTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : !data || data.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('refindEmpty')}</p>
        </div>
      ) : (
        <div className="refind-list">
          {data.map((page) => (
            <RefindCard key={page.canonicalUrl} page={page} t={t} />
          ))}
        </div>
      )}
    </section>
  )
}

function RefindCard({ page, t }: { page: RefindPage; t: T }) {
  const [showFactors, setShowFactors] = useState(false)

  const factors = [
    { label: t('refindFactorDays'), value: page.crossDayCount, weight: 3 },
    { label: t('refindFactorTrails'), value: page.trailCount, weight: 3 },
    {
      label: t('refindFactorSearch'),
      value: page.searchArrivalCount,
      weight: 2,
    },
    { label: t('refindFactorTyped'), value: page.typedRevisitCount, weight: 1 },
  ]
  const maxContribution = Math.max(...factors.map((f) => f.value * f.weight), 1)

  return (
    <div className="refind-card">
      <div className="refind-card__header">
        <span className="refind-card__icon">📄</span>
        <span className="refind-card__title">{page.title ?? page.url}</span>
      </div>
      <p className="refind-card__description">
        {t('refindDescription', {
          days: page.crossDayCount,
          searches: page.searchArrivalCount,
        })}
      </p>
      <button
        className="refind-card__expand-toggle"
        type="button"
        onClick={() => setShowFactors(!showFactors)}
      >
        <span>{showFactors ? '▾' : '▸'}</span>
        <span>{t('refindShowFactors')}</span>
        <span style={{ marginLeft: 'auto', fontFamily: 'var(--font-code)' }}>
          {t('refindScore')}: {page.refindScore.toFixed(1)}
        </span>
      </button>
      {showFactors && (
        <div className="refind-card__factors">
          {factors.map((f) => (
            <div key={f.label} className="refind-card__factor">
              <span className="refind-card__factor-label">{f.label}</span>
              <span
                className="refind-card__factor-bar"
                style={{
                  width: `${Math.round(((f.value * f.weight) / maxContribution) * 80)}px`,
                }}
              />
              <span className="refind-card__factor-value">
                {f.value} ×{f.weight}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// --- Activity Mix ---

function ActivityMixSection({ dateRange, t }: { dateRange: DateRange; t: T }) {
  const { data, loading } = useAsyncData(
    () => api.getActivityMix(dateRange),
    [dateRange],
  )

  return (
    <section className="intelligence-section activity-mix-section">
      <h2 className="intelligence-section__title">{t('activityMixTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !data || data.categories.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('activityMixEmpty')}</p>
        </div>
      ) : (
        <div className="activity-mix">
          {data.categories.map((cat) => {
            const change = data.changeVsPrevious.find(
              (c) => c.domainCategory === cat.domainCategory,
            )
            const changePts = change?.changePoints ?? 0
            return (
              <div key={cat.domainCategory} className="activity-mix__row">
                <span className="activity-mix__category">
                  {t(`category_${cat.domainCategory}`) || cat.domainCategory}
                </span>
                <span className="activity-mix__bar">
                  <span
                    className="activity-mix__bar-fill"
                    style={{ width: `${Math.round(cat.share * 100)}%` }}
                    data-category={cat.domainCategory}
                  />
                </span>
                <span className="activity-mix__share">
                  {Math.round(cat.share * 100)}%
                </span>
                {changePts !== 0 && (
                  <span
                    className={`activity-mix__change activity-mix__change--${changePts > 0 ? 'positive' : 'negative'}`}
                  >
                    {changePts > 0 ? '+' : ''}
                    {Math.round(changePts * 100)}%
                  </span>
                )}
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}

// --- Browsing Rhythm Heatmap ---

function BrowsingRhythmSection({
  dateRange,
  t,
}: {
  dateRange: DateRange
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getBrowsingRhythm(dateRange),
    [dateRange],
  )

  const days = [
    t('dow_sun'),
    t('dow_mon'),
    t('dow_tue'),
    t('dow_wed'),
    t('dow_thu'),
    t('dow_fri'),
    t('dow_sat'),
  ]

  return (
    <section className="intelligence-section rhythm-section">
      <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--heatmap" />
      ) : !data || data.cells.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('rhythmEmpty')}</p>
        </div>
      ) : (
        <div
          className="rhythm-heatmap"
          role="img"
          aria-label={t('rhythmLabel')}
        >
          <div className="rhythm-heatmap__header">
            <span className="rhythm-heatmap__corner" />
            {Array.from({ length: 24 }).map((_, h) => (
              <span key={h} className="rhythm-heatmap__hour">
                {h}
              </span>
            ))}
          </div>
          {days.map((dayLabel, dow) => (
            <div key={dow} className="rhythm-heatmap__row">
              <span className="rhythm-heatmap__day">{dayLabel}</span>
              {Array.from({ length: 24 }).map((_, hour) => {
                const cell = data.cells.find(
                  (c) => c.dow === dow && c.hour === hour,
                )
                const intensity = cell
                  ? Math.min(1, cell.visitCount / data.maxCount)
                  : 0
                return (
                  <span
                    key={hour}
                    className="rhythm-heatmap__cell"
                    style={{
                      opacity: intensity > 0 ? 0.2 + intensity * 0.8 : 0.05,
                      backgroundColor:
                        intensity > 0 ? 'var(--accent)' : 'var(--text-faint)',
                    }}
                    title={
                      cell
                        ? t('rhythmCellTooltip', {
                            day: dayLabel,
                            hour,
                            count: cell.visitCount,
                          })
                        : ''
                    }
                  />
                )
              })}
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

// --- Query Families Panel ---

function QueryFamiliesPanel({ dateRange, t }: { dateRange: DateRange; t: T }) {
  const { data, loading, error } = useAsyncData(
    () => api.getQueryFamilies(dateRange, null, { page: 1, pageSize: 10 }),
    [dateRange],
  )

  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--list" />
  }

  if (error || !data || data.families.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">
          {error || t('queryFamiliesPlaceholder')}
        </p>
      </div>
    )
  }

  return (
    <div className="query-families">
      {data.families.map((family) => (
        <QueryFamilyCard key={family.familyId} family={family} t={t} />
      ))}
    </div>
  )
}

function QueryFamilyCard({ family, t }: { family: QueryFamily; t: T }) {
  const [expanded, setExpanded] = useState(false)
  const visibleQueries = expanded ? family.queries : family.queries.slice(0, 3)

  return (
    <div className="query-family-card">
      <div className="query-family-card__header">
        <span className="query-family-card__anchor">
          "{family.anchorQuery}"
        </span>
        <span className="query-family-card__engine">{family.searchEngine}</span>
        <span className="query-family-card__count">
          {family.memberCount} {t('queryFamilyMemberCount')}
        </span>
      </div>
      <div className="query-family-card__members">
        {visibleQueries.map((q, i) => (
          <span key={i} className="query-family-card__member">
            "{q}"
          </span>
        ))}
        {family.queries.length > 3 && !expanded && (
          <button
            className="intelligence-link"
            onClick={() => setExpanded(true)}
          >
            +{family.queries.length - 3} {t('queryFamilyMore')}
          </button>
        )}
      </div>
      <span className="query-family-card__dates">
        {family.firstSeenAt} — {family.lastSeenAt}
      </span>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}
