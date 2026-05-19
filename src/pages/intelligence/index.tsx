/**
 * Intelligence route shell for Core Intelligence.
 *
 * Why this file exists:
 * - `/intelligence` is the deterministic analysis route and should focus on route state, scope honesty, and deep-link grammar.
 * - Section rendering lives in sibling modules so this file stays readable when the page grows.
 *
 * Main declarations:
 * - `IntelligencePage`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/features/core-intelligence-ultimate-design.md` §2.2.
 * - Keep route and scope grammar aligned with `docs/design/screens-and-nav.md`.
 */

import './intelligence.css'

import { useEffect, useMemo, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { peekIntelligencePrimaryOverview } from '../../lib/core-intelligence'
import { PaperIntelligenceView } from '../../components/explorer-paper'
import { getDomainAbbr, getDomainColor } from '../explorer/paper/domain-color'
import { buildPaperIntelligenceCopy } from '../explorer/paper-explorer-copy'
import {
  compareSetInsightsHref,
  dayInsightsHref,
  domainInsightsHref,
  queryFamilyInsightsHref,
  refindInsightsHref,
  trailInsightsHref,
} from '../../lib/core-intelligence/routes'
import { useI18n } from '../../lib/i18n/hooks'
import { IntelligenceSections, IntelligenceSectionsSkeleton } from './sections'
import { useIntelligenceRouteState } from './route-state'
import { IntelligenceRuntimeDigest } from './runtime-digest'
import { useStagedIntelligenceOverview } from './use-staged-intelligence-overview'
import { intelligenceText } from './copy'

/**
 * Renders the `/intelligence` route.
 */
export function IntelligencePage() {
  const { language, t, ns } = useI18n('intelligence')
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const { dashboard, snapshot } = useShellData()
  const {
    dateRange,
    effectiveProfileId,
    preset,
    profileScopeLabel,
    setCustomRange,
    setPreset,
  } = useIntelligenceRouteState()

  const archiveWideBadge = intelligenceText(language, t, 'archiveWideBadge')
  const stagedOverview = useStagedIntelligenceOverview(
    dateRange,
    effectiveProfileId,
  )
  const primaryOverview = peekIntelligencePrimaryOverview(
    dateRange,
    effectiveProfileId,
  )
  const topSiteSuggestions = primaryOverview?.topSites.data ?? []
  const domainHref = (domain: string) =>
    domainInsightsHref({
      domain,
      dateRange,
      preset,
      profileId: effectiveProfileId,
    })
  const focusedDomainHref = (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) =>
    domainInsightsHref({
      domain,
      dateRange,
      preset,
      profileId: effectiveProfileId,
      focus,
    })
  const queryFamilyHref = (
    familyId: string,
    profileIdOverride?: string | null,
  ) =>
    queryFamilyInsightsHref({
      familyId,
      dateRange,
      preset,
      profileId: profileIdOverride ?? effectiveProfileId,
    })
  const refindHref = (canonicalUrl: string) =>
    refindInsightsHref({
      canonicalUrl,
      dateRange,
      preset,
      profileId: effectiveProfileId,
    })
  const trailHref = (trailId: string, profileIdOverride?: string | null) =>
    trailInsightsHref({
      trailId,
      dateRange,
      preset,
      profileId: profileIdOverride ?? effectiveProfileId,
    })
  const compareSetHref = (compareSetId: string) =>
    compareSetInsightsHref({
      compareSetId,
      dateRange,
      preset,
      profileId: effectiveProfileId,
    })
  const dayHref = (date: string) => dayInsightsHref(date, effectiveProfileId)

  return (
    <div className="intelligence-page" data-testid="intelligence-page">
      <div className="intelligence-page__header">
        <TimeRangeSelector
          key={`${preset}:${dateRange.start}:${dateRange.end}`}
          dateRange={dateRange}
          preset={preset}
          onPresetChange={setPreset}
          onCustomRange={setCustomRange}
          t={t}
        />
        {effectiveProfileId ? (
          <p className="intelligence-page__scope-note">
            {t('scopedViewBody', {
              profile: profileScopeLabel ?? effectiveProfileId,
            })}
          </p>
        ) : null}
      </div>

      <InsightAccessStrip
        date={dateRange.end}
        dayHref={dayHref}
        domainHref={domainHref}
        suggestions={topSiteSuggestions.map((site) => ({
          domain: site.registrableDomain,
          label: site.displayName ?? site.registrableDomain,
        }))}
        t={t}
      />

      <IntelligenceRuntimeDigest
        initialized={Boolean(snapshot?.config.initialized)}
        unlocked={Boolean(snapshot?.archiveStatus.unlocked)}
      />

      {searchParams.get('layout') === 'paper' ? (
        <PaperIntelligencePanel
          primaryOverview={primaryOverview}
          dashboard={dashboard}
          onSelectDomain={(domain) => void navigate(domainHref(domain))}
          explorerT={ns('explorer')}
        />
      ) : null}

      {stagedOverview.primaryReady ? (
        <IntelligenceSections
          compareSetHref={compareSetHref}
          dashboard={dashboard}
          dateRange={dateRange}
          dayHref={dayHref}
          domainHref={domainHref}
          focusedDomainHref={focusedDomainHref}
          language={language}
          preset={preset}
          profileId={effectiveProfileId}
          queryFamilyHref={queryFamilyHref}
          refindHref={refindHref}
          scopeLabel={
            effectiveProfileId
              ? (profileScopeLabel ?? effectiveProfileId)
              : archiveWideBadge
          }
          secondaryReady={stagedOverview.secondaryReady}
          trailHref={trailHref}
          t={t}
        />
      ) : (
        <IntelligenceSectionsSkeleton />
      )}
    </div>
  )
}

function InsightAccessStrip({
  date,
  dayHref,
  domainHref,
  suggestions,
  t,
}: {
  date: string
  dayHref: (date: string) => string
  domainHref: (domain: string) => string
  suggestions: Array<{ domain: string; label: string }>
  t: (key: string, vars?: Record<string, string | number>) => string
}) {
  const navigate = useNavigate()
  const [dayValue, setDayValue] = useState(date)
  const [domainValue, setDomainValue] = useState('')

  useEffect(() => {
    setDayValue(date)
  }, [date])

  return (
    <section className="intelligence-access-strip">
      <div className="intelligence-access-strip__copy">
        <span className="mono-kicker">{t('insightAccessEyebrow')}</span>
        <h2 className="intelligence-access-strip__title">
          {t('insightAccessTitle')}
        </h2>
      </div>
      <div className="intelligence-access-strip__controls">
        <label className="intelligence-access-strip__control">
          <span>{t('insightAccessDayLabel')}</span>
          <div className="intelligence-access-strip__action">
            <input
              type="date"
              value={dayValue}
              onChange={(event) => setDayValue(event.target.value)}
            />
            <button
              className="btn-secondary"
              type="button"
              disabled={!dayValue}
              onClick={() => void navigate(dayHref(dayValue))}
            >
              {t('openDayInsights')}
            </button>
          </div>
        </label>
        <label className="intelligence-access-strip__control">
          <span>{t('insightAccessDomainLabel')}</span>
          <div className="intelligence-access-strip__action">
            <input
              list="intelligence-domain-suggestions"
              type="search"
              value={domainValue}
              onChange={(event) => setDomainValue(event.target.value)}
              placeholder={t('topSitesSearch')}
            />
            <datalist id="intelligence-domain-suggestions">
              {suggestions.map((suggestion) => (
                <option key={suggestion.domain} value={suggestion.domain}>
                  {suggestion.label}
                </option>
              ))}
            </datalist>
            <button
              className="btn-secondary"
              type="button"
              disabled={!domainValue.trim()}
              onClick={() => void navigate(domainHref(domainValue.trim()))}
            >
              {t('openDomainInsights')}
            </button>
          </div>
        </label>
      </div>
    </section>
  )
}

export { DayInsightsRoutePage } from './day-insights'
export { DomainDeepDiveRoutePage } from './domain-deep-dive'
export {
  CompareSetInsightsRoutePage,
  QueryFamilyInsightsRoutePage,
  RefindPageInsightsRoutePage,
  SessionInsightsRoutePage,
  TrailInsightsRoutePage,
} from './promoted-entity-routes'

/**
 * Paper-redesign panel that maps the existing primary-overview + dashboard
 * data into PaperIntelligenceView. Mounted alongside the v0.2 sections when
 * the route has `?layout=paper`, so the redesign can be QA'd inline without
 * disturbing the existing layout.
 */
function PaperIntelligencePanel({
  primaryOverview,
  dashboard,
  onSelectDomain,
  explorerT,
}: {
  primaryOverview: ReturnType<typeof peekIntelligencePrimaryOverview>
  dashboard: ReturnType<typeof useShellData>['dashboard']
  onSelectDomain: (domain: string) => void
  explorerT: (key: string, vars?: Record<string, string | number>) => string
}) {
  const copy = useMemo(() => buildPaperIntelligenceCopy(explorerT), [explorerT])

  const domains = useMemo(
    () =>
      (primaryOverview?.topSites.data ?? []).slice(0, 8).map((site) => ({
        domain: site.registrableDomain,
        count: site.visitCount,
      })),
    [primaryOverview?.topSites.data],
  )

  const refindItems = useMemo(
    () =>
      (primaryOverview?.refindPages.data ?? []).slice(0, 6).map((page) => ({
        id: page.canonicalUrl,
        title: page.title ?? page.url,
        domain: page.registrableDomain,
        meta: `${page.crossDayCount} days · ${page.trailCount} sessions`,
      })),
    [primaryOverview?.refindPages.data],
  )

  const kpis = useMemo(
    () => [
      {
        id: 'week',
        label: explorerT('paperIntelligence.kpiThisWeekLabel'),
        value: (dashboard?.totalVisits ?? 0).toLocaleString(),
      },
      {
        id: 'top',
        label: explorerT('paperIntelligence.kpiTopDomainLabel'),
        value: domains[0]?.domain ?? '—',
        monoValue: true,
        sub:
          domains[0]?.count !== undefined
            ? explorerT('paperIntelligence.kpiTopDomainSub', {
                count: domains[0].count,
                pct: Math.round(
                  (domains[0].count /
                    Math.max(
                      1,
                      domains.reduce((acc, row) => acc + row.count, 0),
                    )) *
                    100,
                ),
              })
            : undefined,
      },
      {
        id: 'threads',
        label: explorerT('paperIntelligence.kpiActiveThreadsLabel'),
        value: String(refindItems.length),
      },
      {
        id: 'sources',
        label: 'Sources',
        value: String(dashboard?.recentRuns.length ?? 0),
      },
    ],
    [dashboard, domains, refindItems, explorerT],
  )

  return (
    <section
      data-testid="paper-intelligence-panel"
      className="border-border-light mt-6 border-t pt-6"
    >
      <PaperIntelligenceView
        kpis={kpis}
        topics={[]}
        domains={domains}
        sessions={[]}
        threads={[]}
        refindItems={refindItems}
        resolveDomainColor={getDomainColor}
        resolveDomainAbbr={getDomainAbbr}
        onSelectDomain={onSelectDomain}
        copy={copy}
        testId="paper-intelligence-view"
      />
    </section>
  )
}
