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

import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { peekIntelligencePrimaryOverview } from '../../lib/core-intelligence'
import { useI18n } from '../../lib/i18n/hooks'
import { dayInsightsHref, domainInsightsHref } from '../../lib/intelligence'
import { IntelligenceSections, IntelligenceSectionsSkeleton } from './sections'
import { useIntelligenceRouteState } from './route-state'
import { IntelligenceRuntimeDigest } from './runtime-digest'
import { useStagedIntelligenceOverview } from './use-staged-intelligence-overview'
import { intelligenceText } from './copy'

/**
 * Renders the `/intelligence` route.
 */
export function IntelligencePage() {
  const { language, t } = useI18n('intelligence')
  const { snapshot } = useShellData()
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

      {stagedOverview.primaryReady ? (
        <IntelligenceSections
          dateRange={dateRange}
          dayHref={dayHref}
          domainHref={domainHref}
          language={language}
          profileId={effectiveProfileId}
          scopeLabel={
            effectiveProfileId
              ? (profileScopeLabel ?? effectiveProfileId)
              : archiveWideBadge
          }
          secondaryReady={stagedOverview.secondaryReady}
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
