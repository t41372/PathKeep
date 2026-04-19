/**
 * Search and activity-mix sections for `/intelligence`.
 *
 * Why this file exists:
 * - Search activity and activity mix are sibling half-width cards in the
 *   current IA, so keeping them together makes layout-level iteration easier.
 * - Pulling these interactive sections out of the route aggregator keeps the
 *   main coordinator readable.
 *
 * Main declarations:
 * - `SearchActivitySection`
 * - `ActivityMixSection`
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type EngineRanking,
  type QueryFamily,
  type SearchConcept,
  type TopSite,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { intelligenceCategoryLabel } from '../copy'
import { IntelligenceSectionBody } from './section-body'
import { firstSectionMeta, formatNumber, type T } from './shared'

export function SearchActivitySection({
  dateRange,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const [tab, setTab] = useState<'engines' | 'concepts' | 'families'>('engines')
  const engines = useAsyncData(
    () => api.getSearchEngineRanking(dateRange, profileId),
    [dateRange, profileId],
  )
  const concepts = useAsyncData(
    () => api.getTopSearchConcepts(dateRange, profileId, 50),
    [dateRange, profileId],
  )
  const meta = firstSectionMeta(engines.data, concepts.data)

  return (
    <section className="intelligence-section search-activity-section">
      <h2 className="intelligence-section__title">
        {t('searchActivityTitle')}
      </h2>
      {meta ? (
        <IntelligenceSectionMeta meta={meta} scopeLabel={scopeLabel} />
      ) : null}
      <div className="intelligence-tabs" role="tablist">
        {(['engines', 'concepts', 'families'] as const).map((key) => (
          <button
            key={key}
            role="tab"
            type="button"
            aria-selected={tab === key}
            className={`intelligence-tab${tab === key ? ' intelligence-tab--active' : ''}`}
            onClick={() => setTab(key)}
          >
            {t(`searchTab_${key}`)}
          </button>
        ))}
      </div>
      <IntelligenceSectionBody className="intelligence-tab-content">
        {tab === 'engines' ? (
          <EngineRankingPanel
            data={engines.data?.data ?? null}
            loading={engines.loading}
            t={t}
          />
        ) : null}
        {tab === 'concepts' ? (
          <ConceptCloudPanel
            data={concepts.data?.data ?? null}
            loading={concepts.loading}
            t={t}
          />
        ) : null}
        {tab === 'families' ? (
          <QueryFamiliesPanel
            dateRange={dateRange}
            profileId={profileId}
            t={t}
          />
        ) : null}
      </IntelligenceSectionBody>
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
  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--bar" />
  }
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('engineRankingEmpty')}</p>
      </div>
    )
  }

  const max = Math.max(data[0]?.searchCount ?? 0, 1)
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
  if (loading) {
    return (
      <div className="intelligence-skeleton intelligence-skeleton--cloud" />
    )
  }
  if (!data || data.length === 0) {
    return (
      <div className="intelligence-empty">
        <p className="intelligence-empty__text">{t('conceptCloudEmpty')}</p>
      </div>
    )
  }

  const maxFrequency = Math.max(...data.map((concept) => concept.frequency), 1)
  return (
    <div
      className="concept-cloud"
      role="img"
      aria-label={t('conceptCloudLabel')}
    >
      {data.map((concept) => {
        const scale = 0.6 + (concept.frequency / maxFrequency) * 1.4
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

function QueryFamiliesPanel({
  dateRange,
  profileId,
  t,
}: {
  dateRange: DateRange
  profileId: string | null
  t: T
}) {
  const { data, loading, error } = useAsyncData(
    () => api.getQueryFamilies(dateRange, profileId, { page: 0, pageSize: 10 }),
    [dateRange, profileId],
  )
  const families = data?.data.families ?? []

  if (loading) {
    return <div className="intelligence-skeleton intelligence-skeleton--list" />
  }

  if (error || families.length === 0) {
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
      {families.map((family) => (
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
        {visibleQueries.map((query, index) => (
          <span key={index} className="query-family-card__member">
            "{query}"
          </span>
        ))}
        {family.queries.length > 3 && !expanded ? (
          <button
            className="intelligence-link"
            type="button"
            onClick={() => setExpanded(true)}
          >
            +{family.queries.length - 3} {t('queryFamilyMore')}
          </button>
        ) : null}
      </div>
      <span className="query-family-card__dates">
        {family.firstSeenAt} - {family.lastSeenAt}
      </span>
      <ExplainabilityPanel
        entityType="query_family"
        entityId={family.familyId}
        t={t}
      />
    </div>
  )
}

export function ActivityMixSection({
  dateRange,
  domainHref,
  language,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const mixResult = useAsyncData(
    () => api.getActivityMix(dateRange, profileId),
    [dateRange, profileId],
  )
  const topSitesResult = useAsyncData(
    () => api.getTopSites(dateRange, profileId, 'visit_count', 40),
    [dateRange, profileId],
  )
  const mix = mixResult.data?.data ?? null
  const examplesByCategory = new Map<string, TopSite[]>()

  for (const site of topSitesResult.data?.data ?? []) {
    const current = examplesByCategory.get(site.domainCategory) ?? []
    if (
      !current.some(
        (entry) => entry.registrableDomain === site.registrableDomain,
      ) &&
      current.length < 3
    ) {
      current.push(site)
      examplesByCategory.set(site.domainCategory, current)
    }
  }

  return (
    <section className="intelligence-section activity-mix-section">
      <h2 className="intelligence-section__title">{t('activityMixTitle')}</h2>
      {mixResult.data ? (
        <IntelligenceSectionMeta
          meta={mixResult.data.meta}
          scopeLabel={scopeLabel}
        />
      ) : null}
      <p className="intelligence-section__help">{t('activityMixHelp')}</p>
      {mixResult.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !mix || mix.categories.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('activityMixEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="activity-mix">
          {mix.categories.map((category) => {
            const change = mix.changeVsPrevious.find(
              (entry) => entry.domainCategory === category.domainCategory,
            )
            const changePoints = change?.changePoints ?? 0
            const examples =
              examplesByCategory.get(category.domainCategory) ?? []
            return (
              <div key={category.domainCategory} className="activity-mix__row">
                <div className="activity-mix__summary">
                  <span className="activity-mix__category">
                    {intelligenceCategoryLabel(
                      language,
                      t,
                      category.domainCategory,
                    )}
                  </span>
                  <span className="activity-mix__share">
                    {Math.round(category.share * 100)}%
                  </span>
                </div>
                <span className="activity-mix__bar">
                  <span
                    className="activity-mix__bar-fill"
                    style={{ width: `${Math.round(category.share * 100)}%` }}
                    data-category={category.domainCategory}
                  />
                </span>
                <div className="activity-mix__meta">
                  {examples.length > 0 ? (
                    <span className="activity-mix__examples">
                      {t('activityMixExamples', {
                        domains: examples
                          .map(
                            (entry) =>
                              entry.displayName ?? entry.registrableDomain,
                          )
                          .join(', '),
                      })}
                      <span className="activity-mix__example-links">
                        {examples.map((entry) => (
                          <Link
                            key={entry.registrableDomain}
                            className="intelligence-link"
                            to={domainHref(entry.registrableDomain)}
                          >
                            {entry.displayName ?? entry.registrableDomain}
                          </Link>
                        ))}
                      </span>
                    </span>
                  ) : null}
                  {changePoints !== 0 ? (
                    <span
                      className={`activity-mix__change activity-mix__change--${changePoints > 0 ? 'positive' : 'negative'}`}
                    >
                      {changePoints > 0 ? '+' : ''}
                      {Math.round(changePoints * 100)}%
                    </span>
                  ) : null}
                </div>
              </div>
            )
          })}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}
