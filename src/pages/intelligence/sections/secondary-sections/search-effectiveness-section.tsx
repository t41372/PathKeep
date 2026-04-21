/**
 * @file search-effectiveness-section.tsx
 * @description Renders the secondary-grid card that explains how search sessions resolve across engines, sources, and hard topics.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load deterministic search-effectiveness summaries for the current intelligence scope.
 * - Hide the card when the overview returns no meaningful engine, source, or topic data.
 * - Preserve the existing query-family and domain drilldown links used elsewhere in Intelligence.
 *
 * ## Non-Responsibilities
 * - Does not rewrite search taxonomy or decide what counts as a query family.
 * - Does not own page-level section ordering or scroll behavior outside this card body.
 * - Does not mutate overview cache state beyond the existing read hooks.
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` for the read and peek contract.
 * - `../shared` for number formatting and route-local translations.
 * - `../section-body` and `components/intelligence/section-meta` for card presentation.
 *
 * ## Performance Notes
 * - Works on bounded overview aggregates rather than raw search trails.
 * - Keeps visible source/topic lists truncated so large archives do not expand this card indefinitely.
 */

import { Link } from 'react-router-dom'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type SearchEffectiveness,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import { formatNumber, type T } from '../shared'

type SearchEffectivenessSectionProps = {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  queryFamilyHref: (familyId: string) => string
  scopeLabel: string
  t: T
}

/**
 * Summarizes how efficiently search sessions converge so the user can spot
 * engines that resolve quickly, domains that finish the work, and topics that keep getting rewritten.
 *
 * @param dateRange Active intelligence time window used for the deterministic overview request.
 * @param domainHref Shared route builder for domain deep dives.
 * @param profileId Optional profile scope; `null` means the aggregate intelligence scope.
 * @param queryFamilyHref Shared route builder for query-family deep dives.
 * @param scopeLabel Localized scope summary shown in the freshness metadata.
 * @param t Route-local translator for all visible labels.
 * @returns A search-effectiveness card, an empty/loading state, or `null` when the overview has no meaningful content.
 */
export function SearchEffectivenessSection({
  dateRange,
  domainHref,
  profileId,
  queryFamilyHref,
  scopeLabel,
  t,
}: SearchEffectivenessSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getSearchEffectiveness(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekSearchEffectiveness(dateRange, profileId),
    },
  )
  const effectiveness = data?.data ?? null

  if (
    !loading &&
    (!effectiveness ||
      (effectiveness.engineStats.length === 0 &&
        effectiveness.topResolvingSources.length === 0 &&
        effectiveness.hardestTopics.length === 0)) &&
    data?.meta.state === 'ready'
  ) {
    return null
  }

  return (
    <section className="intelligence-section search-effectiveness-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('searchEffectivenessTitle')}
        </h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <p className="intelligence-section__help">
        {t('searchEffectivenessHelp')}
      </p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !effectiveness ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {t('searchEffectivenessEmpty')}
          </p>
        </div>
      ) : (
        <IntelligenceSectionBody className="search-effectiveness">
          {effectiveness.engineStats.length > 0 ? (
            <div className="search-effectiveness__engines">
              {effectiveness.engineStats.map((engine) => (
                <SearchEngineCard
                  key={engine.searchEngine}
                  engine={engine}
                  t={t}
                />
              ))}
            </div>
          ) : null}
          {effectiveness.topResolvingSources.length > 0 ? (
            <div className="search-effectiveness__sources">
              <h3 className="search-effectiveness__subtitle">
                {t('searchEffectivenessSources')}
              </h3>
              <p className="search-effectiveness__help">
                {t('searchEffectivenessSourcesHelp')}
              </p>
              {effectiveness.topResolvingSources.slice(0, 5).map((source) => (
                <Link
                  key={`${source.sourceRole}:${source.registrableDomain}`}
                  className="search-effectiveness__source-row"
                  to={domainHref(source.registrableDomain)}
                >
                  <span className="search-effectiveness__source-main">
                    <span className="search-effectiveness__source-domain">
                      {source.displayName ?? source.registrableDomain}
                    </span>
                    <span className="search-effectiveness__source-detail">
                      {source.sourceRole === 'landing'
                        ? t('stableSourcesLandingCount', {
                            count: source.stableLandingCount,
                          })
                        : t('stableSourcesEntryCount', {
                            count: source.trailCount,
                          })}
                    </span>
                  </span>
                  <span className="search-effectiveness__source-meta">
                    {t(
                      source.sourceRole === 'landing'
                        ? 'stableSourcesLanding'
                        : 'stableSourcesEntry',
                    )}
                  </span>
                </Link>
              ))}
            </div>
          ) : null}
          {effectiveness.hardestTopics.length > 0 ? (
            <div className="search-effectiveness__hard-topics">
              <h3 className="search-effectiveness__subtitle">
                {t('searchEffectivenessHardest')}
              </h3>
              <p className="search-effectiveness__help">
                {t('searchEffectivenessHardestHelp')}
              </p>
              {effectiveness.hardestTopics.slice(0, 3).map((topic) => (
                <div
                  key={topic.queryFamily}
                  className="search-effectiveness__topic-row"
                >
                  <span className="search-effectiveness__topic-main">
                    <Link
                      className="search-effectiveness__topic-query intelligence-link"
                      to={queryFamilyHref(topic.familyId)}
                    >
                      "{topic.queryFamily}"
                    </Link>
                    <span className="search-effectiveness__topic-detail">
                      {t('searchEffectivenessLag', {
                        days: topic.reSearchLagDays.toFixed(1),
                      })}
                    </span>
                  </span>
                  <span className="search-effectiveness__topic-stat">
                    {t('searchEffectivenessRewrites', {
                      count: topic.reformulationCount,
                    })}
                  </span>
                </div>
              ))}
            </div>
          ) : null}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function SearchEngineCard({
  engine,
  t,
}: {
  engine: SearchEffectiveness['engineStats'][number]
  t: T
}) {
  return (
    <div className="search-effectiveness__engine-row">
      <div className="search-effectiveness__engine-summary">
        <span className="search-effectiveness__engine-name">
          {engine.displayName ?? engine.searchEngine}
        </span>
        <span className="search-effectiveness__engine-stat">
          {t('searchEffectivenessTrails', {
            count: engine.totalTrails,
          })}
        </span>
      </div>
      <ul className="search-effectiveness__metric-list">
        <li className="search-effectiveness__metric-line">
          {t('searchEffectivenessEngineRewrites', {
            count: engine.avgReformulations.toFixed(1),
          })}
        </li>
        <li className="search-effectiveness__metric-line">
          {t('searchEffectivenessEngineDepth', {
            count: engine.avgDepth.toFixed(1),
          })}
        </li>
        <li className="search-effectiveness__metric-line">
          {t('searchEffectivenessEngineTrails', {
            count: formatNumber(engine.totalTrails),
          })}
        </li>
      </ul>
    </div>
  )
}
