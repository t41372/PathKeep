/**
 * Secondary Intelligence sections and supporting helpers.
 *
 * Why this file exists:
 * - These cards share the same "secondary grid" layout and the same honesty
 *   rules: low-signal sections should degrade or disappear instead of taking
 *   over the page.
 * - Extracting them keeps the route-level coordinator focused on IA rather
 *   than dozens of smaller section implementations.
 *
 * Main declarations:
 * - `HabitsSection`
 * - `StableSourcesSection`
 * - `SearchEffectivenessSection`
 * - `FrictionDetectionSection`
 * - `ReopenedInvestigationsSection`
 * - `DiscoveryTrendSection`
 * - `BreadthIndexSection`
 * - `PathFlowsSection`
 * - `CompareSetsSection`
 * - `MultiBrowserDiffSection`
 * - `ObservedInteractionsSection`
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { CompareSetPageList } from '../../../components/intelligence/compare-set-page-list'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type BreadthIndex,
  type BrowserDiff,
  type BrowserProfileSummary,
  type CategoryMixEntry,
  type CompareSet,
  type DateRange,
  type FrictionSignal,
  type HabitPattern,
  type InterruptedHabit,
  type ObservedInteraction,
  type PathFlow,
  type ReopenedInvestigation,
  type SearchEffectiveness,
  type StableSource,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { reopenedInvestigationHref } from '../../../lib/intelligence'
import { intelligenceText } from '../copy'
import { IntelligenceSectionBody } from './section-body'
import {
  firstSectionMeta,
  formatDuration,
  formatIsoDate,
  formatNumber,
  type T,
} from './shared'

export function StableSourcesSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getStableSources(dateRange, profileId),
    [dateRange, profileId],
  )
  const sources = data?.data ?? []
  const entries = sources.filter((source) => source.sourceRole === 'entry')
  const landings = sources.filter((source) => source.sourceRole === 'landing')

  if (
    !loading &&
    data?.meta.state === 'ready' &&
    (!hasMeaningfulStableSources(entries, landings) || sources.length === 0)
  ) {
    return null
  }

  return (
    <section className="intelligence-section stable-sources-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('stableSourcesTitle')}
        </h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <p className="intelligence-section__help">{t('stableSourcesHelp')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : sources.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('stableSourcesEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="stable-sources">
          <StableSourceColumn
            emptyLabel={t('stableSourcesNoEntry')}
            help={t('stableSourcesEntryHelp')}
            metricLabel={(source) =>
              t('stableSourcesEntryCount', {
                count: source.trailCount,
              })
            }
            sources={entries}
            title={t('stableSourcesEntry')}
            toHref={domainHref}
          />
          <StableSourceColumn
            emptyLabel={t('stableSourcesNoLanding')}
            help={t('stableSourcesLandingHelp')}
            metricLabel={(source) =>
              t('stableSourcesLandingCount', {
                count: source.stableLandingCount,
              })
            }
            sources={landings}
            title={t('stableSourcesLanding')}
            toHref={domainHref}
          />
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function StableSourceColumn({
  emptyLabel,
  help,
  metricLabel,
  sources,
  title,
  toHref,
}: {
  emptyLabel: string
  help: string
  metricLabel: (source: StableSource) => string
  sources: StableSource[]
  title: string
  toHref: (domain: string) => string
}) {
  return (
    <div className="stable-sources__column">
      <div className="stable-sources__header">
        <h3 className="stable-sources__subtitle">{title}</h3>
        <p className="stable-sources__help">{help}</p>
      </div>
      {sources.length > 0 ? (
        sources.slice(0, 5).map((source, index) => (
          <Link
            key={source.registrableDomain}
            className="stable-source-row"
            to={toHref(source.registrableDomain)}
          >
            <span className="stable-source-row__rank">{index + 1}.</span>
            <span className="stable-source-row__content">
              <span className="stable-source-row__domain">
                {source.displayName ?? source.registrableDomain}
              </span>
              <span className="stable-source-row__detail">
                {metricLabel(source)}
              </span>
            </span>
          </Link>
        ))
      ) : (
        <p className="stable-sources__empty">{emptyLabel}</p>
      )}
    </div>
  )
}

export function SearchEffectivenessSection({
  dateRange,
  domainHref,
  profileId,
  queryFamilyHref,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  queryFamilyHref: (familyId: string) => string
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getSearchEffectiveness(dateRange, profileId),
    [dateRange, profileId],
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

export function FrictionDetectionSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getFrictionSignals(dateRange, profileId),
    [dateRange, profileId],
  )
  const signals = (data?.data ?? []).filter(isMeaningfulFrictionSignal)

  if (!loading && signals.length === 0 && data?.meta.state === 'ready') {
    return null
  }

  return (
    <section className="intelligence-section friction-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('frictionTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : signals.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('frictionEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <div className="friction-list">
            {signals.slice(0, 8).map((signal, index) => (
              <FrictionSignalCard
                key={index}
                domainHref={domainHref}
                signal={signal}
                t={t}
              />
            ))}
          </div>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function FrictionSignalCard({
  domainHref,
  signal,
  t,
}: {
  domainHref: (domain: string) => string
  signal: FrictionSignal
  t: T
}) {
  return (
    <div className="friction-card">
      <div className="friction-card__header">
        <span
          className={`friction-card__evidence-badge friction-card__evidence-badge--${signal.evidenceType}`}
        >
          {signal.evidenceType === 'strong'
            ? t('frictionStrong')
            : t('frictionWeak')}
        </span>
        {signal.registrableDomain ? (
          <Link
            className="friction-card__domain intelligence-link"
            to={domainHref(signal.registrableDomain)}
          >
            {signal.registrableDomain}
          </Link>
        ) : (
          <span className="friction-card__domain">{signal.url ?? '—'}</span>
        )}
        <span className="friction-card__count">{signal.occurrenceCount}×</span>
      </div>
      <p className="friction-card__description">{signal.description}</p>
    </div>
  )
}

export function ReopenedInvestigationsSection({
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
  const { data, loading } = useAsyncData(
    () => api.getReopenedInvestigations(dateRange, profileId),
    [dateRange, profileId],
  )
  const reopened = (data?.data ?? []).filter(
    isSearchBackedReopenedInvestigation,
  )

  if (!loading && reopened.length === 0 && data?.meta.state === 'ready') {
    return null
  }

  return (
    <section className="intelligence-section reopened-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('reopenedTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : reopened.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('reopenedEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <div className="reopened-list">
            {reopened.slice(0, 8).map((item) => (
              <div key={item.investigationId} className="reopened-card">
                <div className="reopened-card__header">
                  <span
                    className={`reopened-card__anchor-badge reopened-card__anchor-badge--${item.anchorType}`}
                  >
                    {item.anchorType === 'query_family'
                      ? t('reopenedAnchorQuery')
                      : t('reopenedAnchorPage')}
                  </span>
                  <Link
                    className="reopened-card__label intelligence-link"
                    to={reopenedInvestigationHref({
                      anchorId: item.anchorId,
                      anchorType: item.anchorType,
                      dateRange,
                      profileId,
                    })}
                  >
                    {item.anchorLabel}
                  </Link>
                </div>
                <div className="reopened-card__meta">
                  <span>
                    {t('reopenedOccurrences', {
                      count: item.occurrenceCount,
                    })}
                  </span>
                  <span>
                    {t('reopenedDistinctDays', { days: item.distinctDays })}
                  </span>
                </div>
                <span className="reopened-card__dates">
                  {item.firstSeenAt} - {item.lastSeenAt}
                </span>
                <ExplainabilityPanel
                  entityType="reopened_investigation"
                  entityId={item.investigationId}
                  t={t}
                />
              </div>
            ))}
          </div>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

export function DiscoveryTrendSection({
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
  const { data, loading } = useAsyncData(
    () => api.getDiscoveryTrend(dateRange, profileId, 'week'),
    [dateRange, profileId],
  )
  const trend = data?.data ?? null
  if (
    !loading &&
    (!trend || trend.points.length === 0) &&
    data?.meta.state === 'ready'
  ) {
    return null
  }
  const visiblePoints = trend ? [...trend.points].slice(-6).reverse() : []

  return (
    <section className="intelligence-section discovery-trend-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('discoveryTrendTitle')}
        </h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      <p className="intelligence-section__help">{t('discoveryTrendHelp')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !trend || trend.points.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('discoveryTrendEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="discovery-trend">
          {visiblePoints.map((point) => {
            const ratePercent = Math.round(point.discoveryRate * 100)

            return (
              <div
                key={point.dateKey}
                className="discovery-trend__row"
                title={`${humanizeDiscoveryWeekLabel(point.dateKey, t)}: ${ratePercent}% · ${point.newDomainCount} ${t('discoveryTrendNewDomains')} · ${point.totalVisits} ${t('visits')}`}
              >
                <div className="discovery-trend__row-header">
                  <span className="discovery-trend__date-label">
                    {humanizeDiscoveryWeekLabel(point.dateKey, t)}
                  </span>
                  <span className="discovery-trend__rate">
                    {t('discoveryTrendRatePercent', {
                      count: ratePercent,
                    })}
                  </span>
                </div>
                <span className="discovery-trend__bar">
                  <span
                    className="discovery-trend__bar-fill"
                    style={{ width: `${Math.max(ratePercent, 2)}%` }}
                  />
                </span>
                <div className="discovery-trend__stats">
                  <span className="discovery-trend__stat">
                    {t('discoveryTrendDomainsLabel')}: {point.newDomainCount}
                  </span>
                  <span className="discovery-trend__stat">
                    {t('discoveryTrendVisitsLabel', {
                      count: point.totalVisits,
                    })}
                  </span>
                </div>
              </div>
            )
          })}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

export function BreadthIndexSection({
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
  const { data, loading } = useAsyncData(
    () => api.getBreadthIndex(dateRange, profileId),
    [dateRange, profileId],
  )
  const breadth = data?.data ?? null

  return (
    <section className="intelligence-section breadth-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('breadthTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : !breadth ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('breadthEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <BreadthIndexBody data={breadth} t={t} />
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function BreadthIndexBody({ data, t }: { data: BreadthIndex; t: T }) {
  const score = Math.max(0, Math.min(100, Math.round(data.breadthScore)))

  return (
    <div className="breadth-index">
      <div className="breadth-index__header">
        <div className="breadth-index__score-block">
          <span className="breadth-index__score">{score}</span>
          <span className="breadth-index__score-label">
            {t('breadthScoreLabel')}
          </span>
        </div>
        <div className="breadth-index__stats">
          <div className="breadth-index__stat-card">
            <span className="breadth-index__stat-label">
              {t('breadthConcentrationLabel')}
            </span>
            <strong className="breadth-index__stat-value">
              {data.concentrationDomainCount}
            </strong>
          </div>
          <div className="breadth-index__stat-card">
            <span className="breadth-index__stat-label">
              {t('breadthHhiKey')}
            </span>
            <strong className="breadth-index__stat-value">
              {data.hhi.toFixed(3)}
            </strong>
          </div>
        </div>
      </div>
      <div className="breadth-index__meter">
        <span
          className="breadth-index__meter-fill"
          style={{ width: `${score}%` }}
        />
      </div>
      <div className="breadth-index__meter-labels">
        <span>{t('breadthAxisFocused')}</span>
        <span>{t('breadthAxisBroad')}</span>
      </div>
      <p className="breadth-index__detail">{t('breadthScoreHelp')}</p>
      <p className="breadth-index__detail">
        {t('breadthConcentrationDetail', {
          count: data.concentrationDomainCount,
        })}
      </p>
      <p className="breadth-index__meta">{t('breadthHhiHelp')}</p>
    </div>
  )
}

export function PathFlowsSection({
  dateRange,
  focusedDomainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const [stepCount, setStepCount] = useState<number>(3)
  const { data, loading } = useAsyncData(
    () => api.getPathFlows(dateRange, profileId, stepCount, 15),
    [dateRange, profileId, stepCount],
  )
  const flows = (data?.data ?? []).filter(isMeaningfulPathFlow)

  if (!loading && flows.length === 0 && data?.meta.state === 'ready') {
    return null
  }

  return (
    <section className="intelligence-section path-flows-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('pathFlowsTitle')}</h2>
        <select
          className="top-sites-controls__sort"
          value={stepCount}
          onChange={(event) => setStepCount(Number(event.target.value))}
          aria-label={t('pathFlowsStepLabel')}
        >
          <option value={2}>{t('pathFlowsStep2')}</option>
          <option value={3}>{t('pathFlowsStep3')}</option>
        </select>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : flows.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('pathFlowsEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <ul className="path-flows">
            {flows.map((flow) => (
              <PathFlowRow
                key={flow.flowId}
                focusedDomainHref={focusedDomainHref}
                flow={flow}
                profileId={profileId}
                t={t}
              />
            ))}
          </ul>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function PathFlowRow({
  focusedDomainHref,
  flow,
  profileId,
  t,
}: {
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  flow: PathFlow
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${flow.stepCount}::${flow.flowPattern}`
    : null

  return (
    <li className="path-flow-row">
      <div className="path-flow-row__chips">
        {flow.steps.map((step) => (
          <span
            key={`${flow.flowId}:${step.index}`}
            className="path-flow-row__group"
          >
            {step.registrableDomain ? (
              <Link
                className="path-flow-row__chip intelligence-link"
                to={focusedDomainHref(step.registrableDomain, {
                  focusType: 'path-flow',
                  focusId: flow.flowId,
                })}
              >
                {step.label}
              </Link>
            ) : (
              <span className="path-flow-row__chip">{step.label}</span>
            )}
            {step.index < flow.steps.length - 1 ? (
              <span className="path-flow-row__arrow" aria-hidden="true">
                →
              </span>
            ) : null}
          </span>
        ))}
      </div>
      <span className="path-flow-row__count">
        {t('pathFlowsOccurrences', { count: flow.occurrenceCount })}
      </span>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="path_flow"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

export function HabitsSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const patterns = useAsyncData(
    () => api.getHabitPatterns(dateRange, profileId),
    [dateRange, profileId],
  )
  const interrupted = useAsyncData(
    () => api.getInterruptedHabits(profileId),
    [profileId],
  )
  const patternsData = patterns.data?.data ?? []
  const interruptedData = interrupted.data?.data ?? []
  const empty =
    !patterns.loading &&
    !interrupted.loading &&
    patternsData.length === 0 &&
    interruptedData.length === 0
  const meta = firstSectionMeta(patterns.data, interrupted.data)

  return (
    <section className="intelligence-section habits-section">
      <h2 className="intelligence-section__title">{t('habitsTitle')}</h2>
      {meta ? (
        <IntelligenceSectionMeta meta={meta} scopeLabel={scopeLabel} />
      ) : null}
      {patterns.loading || interrupted.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : empty ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('habitsEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="habits-body">
          {interruptedData.length > 0 ? (
            <div className="habits-interrupted">
              <h3 className="habits-body__subtitle">
                {t('habitsInterruptedTitle')}
              </h3>
              <ul className="habits-interrupted__list">
                {interruptedData.slice(0, 5).map((habit, index) => (
                  <InterruptedHabitRow
                    key={index}
                    domainHref={domainHref}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
          {patternsData.length > 0 ? (
            <div className="habits-patterns">
              <h3 className="habits-body__subtitle">
                {t('habitsPatternsTitle')}
              </h3>
              <ul className="habits-patterns__list">
                {patternsData.slice(0, 12).map((habit, index) => (
                  <HabitPatternRow
                    key={index}
                    domainHref={domainHref}
                    habit={habit}
                    profileId={profileId}
                    t={t}
                  />
                ))}
              </ul>
            </div>
          ) : null}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function HabitPatternRow({
  domainHref,
  habit,
  profileId,
  t,
}: {
  domainHref: (domain: string) => string
  habit: HabitPattern
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row">
      <div className="habit-row__main">
        <div className="habit-row__header">
          <Link
            className="habit-row__domain intelligence-link"
            to={domainHref(habit.registrableDomain)}
          >
            {habit.displayName ?? habit.registrableDomain}
          </Link>
          <span
            className={`habit-row__type habit-row__type--${habit.habitType}`}
          >
            {t(`habitType_${habit.habitType}`)}
          </span>
        </div>
        <p className="habit-row__summary">
          {t('habitPatternSummary', {
            interval: habit.meanIntervalDays.toFixed(1),
            days: habit.visitCount,
          })}
        </p>
        <p className="habit-row__meta">
          {t('habitLastSeen', {
            date: formatIsoDate(habit.lastVisitedAt),
          })}
        </p>
      </div>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

function InterruptedHabitRow({
  domainHref,
  habit,
  profileId,
  t,
}: {
  domainHref: (domain: string) => string
  habit: InterruptedHabit
  profileId: string | null
  t: T
}) {
  const explainEntityId = profileId
    ? `${profileId}::${habit.registrableDomain}`
    : null

  return (
    <li className="habit-row habit-row--interrupted">
      <div className="habit-row__main">
        <div className="habit-row__header">
          <Link
            className="habit-row__domain intelligence-link"
            to={domainHref(habit.registrableDomain)}
          >
            {habit.displayName ?? habit.registrableDomain}
          </Link>
          <span className="habit-row__type habit-row__type--interrupted">
            {t('habitInterruptedBadge')}
          </span>
        </div>
        <p className="habit-row__summary">
          {t('habitInterruptedSummary', {
            days: habit.daysSinceLastVisit,
            expected: habit.meanIntervalDays.toFixed(1),
          })}
        </p>
        <p className="habit-row__meta">
          {t('habitLastSeen', {
            date: formatIsoDate(habit.lastVisitedAt),
          })}
        </p>
      </div>
      {explainEntityId ? (
        <ExplainabilityPanel
          entityType="habit_pattern"
          entityId={explainEntityId}
          t={t}
        />
      ) : null}
    </li>
  )
}

export function CompareSetsSection({
  compareSetHref,
  dateRange,
  focusedDomainHref,
  profileId,
  scopeLabel,
  trailHref,
  t,
}: {
  compareSetHref: (compareSetId: string) => string
  dateRange: DateRange
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  profileId: string | null
  scopeLabel: string
  trailHref: (trailId: string) => string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getCompareSets(dateRange, profileId),
    [dateRange, profileId],
  )
  const compareSets = data?.data ?? []

  return (
    <section className="intelligence-section compare-sets-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('compareSetsTitle')}</h2>
        {data ? (
          <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
        ) : null}
      </div>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : compareSets.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('compareSetsEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <ul className="compare-sets">
            {compareSets.slice(0, 6).map((set) => (
              <CompareSetCard
                compareSetHref={compareSetHref}
                key={set.compareSetId}
                focusedDomainHref={focusedDomainHref}
                set={set}
                trailHref={trailHref}
                t={t}
              />
            ))}
          </ul>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function CompareSetCard({
  compareSetHref,
  focusedDomainHref,
  set,
  trailHref,
  t,
}: {
  compareSetHref: (compareSetId: string) => string
  focusedDomainHref: (
    domain: string,
    focus: { focusType: 'compare-set' | 'path-flow'; focusId: string },
  ) => string
  set: CompareSet
  trailHref: (trailId: string) => string
  t: T
}) {
  return (
    <li className="compare-set">
      <div className="compare-set__header">
        <Link
          className="compare-set__query intelligence-link"
          to={compareSetHref(set.compareSetId)}
        >
          {set.searchQuery}
        </Link>
        <span className="compare-set__count">
          {t('compareSetsPages', { count: set.pages.length })}
        </span>
      </div>
      <div className="intelligence-actions">
        <Link className="intelligence-link" to={trailHref(set.trailId)}>
          {t('trailRouteTitle')}
        </Link>
      </div>
      <CompareSetPageList
        as="ul"
        getHref={(page) =>
          focusedDomainHref(page.registrableDomain, {
            focusType: 'compare-set',
            focusId: set.compareSetId,
          })
        }
        keyPrefix={set.compareSetId}
        landingLabel={t('compareSetsLanding')}
        maxItems={4}
        pages={set.pages}
      />
    </li>
  )
}

export function MultiBrowserDiffSection({
  dateRange,
  domainHref,
  language,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  scopeLabel: string
  t: T
}) {
  const { data, loading } = useAsyncData(
    () => api.getMultiBrowserDiff(dateRange),
    [dateRange],
  )
  const diff = data?.data ?? null

  return (
    <section className="intelligence-section multi-browser-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {t('multiBrowserTitle')}
        </h2>
        <span className="status-badge status-info">
          {intelligenceText(language, t, 'archiveWideBadge')}
        </span>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--chart" />
      ) : !diff || diff.profiles.length < 2 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('multiBrowserEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <MultiBrowserDiffBody data={diff} domainHref={domainHref} t={t} />
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function MultiBrowserDiffBody({
  data,
  domainHref,
  t,
}: {
  data: BrowserDiff
  domainHref: (domain: string) => string
  t: T
}) {
  const profileById = new Map<string, BrowserProfileSummary>(
    data.profiles.map((profile) => [profile.profileId, profile]),
  )
  const exclusiveByProfile = new Map<string, typeof data.exclusiveDomains>()
  for (const entry of data.exclusiveDomains) {
    const list = exclusiveByProfile.get(entry.profileId) ?? []
    list.push(entry)
    exclusiveByProfile.set(entry.profileId, list)
  }

  return (
    <div className="multi-browser">
      <div className="multi-browser__profiles">
        {data.profiles.map((profile) => (
          <div key={profile.profileId} className="multi-browser__profile">
            <span className="multi-browser__profile-name">
              {profile.profileName}
            </span>
            <span className="multi-browser__profile-family">
              {profile.browserFamily}
            </span>
            <span className="multi-browser__profile-stats">
              {t('multiBrowserVisits', { count: profile.visitCount })} ·{' '}
              {t('multiBrowserDomains', { count: profile.domainCount })}
            </span>
          </div>
        ))}
      </div>
      <div className="multi-browser__shared">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserShared', { count: data.sharedDomains.length })}
        </h3>
        <div className="multi-browser__shared-chips">
          {data.sharedDomains.slice(0, 10).map((domain) => (
            <Link
              key={domain}
              className="multi-browser__chip intelligence-link"
              to={domainHref(domain)}
            >
              {domain}
            </Link>
          ))}
        </div>
      </div>
      <div className="multi-browser__exclusive">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserExclusive')}
        </h3>
        <div className="multi-browser__exclusive-grid">
          {Array.from(exclusiveByProfile.entries()).map(
            ([profileId, entries]) => {
              const profile = profileById.get(profileId)
              return (
                <div
                  key={profileId}
                  className="multi-browser__exclusive-column"
                >
                  <span className="multi-browser__exclusive-header">
                    {profile?.profileName ?? profileId}
                  </span>
                  <ul className="multi-browser__exclusive-list">
                    {entries.slice(0, 5).map((entry) => (
                      <li
                        key={entry.registrableDomain}
                        className="multi-browser__exclusive-row"
                      >
                        <Link
                          className="intelligence-link"
                          to={domainHref(entry.registrableDomain)}
                        >
                          {entry.registrableDomain}
                        </Link>
                        <span className="multi-browser__exclusive-count">
                          {entry.visitCount}
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )
            },
          )}
        </div>
      </div>
      <div className="multi-browser__categories">
        <h3 className="multi-browser__subtitle">
          {t('multiBrowserCategories')}
        </h3>
        <MultiBrowserCategoryBars
          distributions={data.categoryDistributions}
          t={t}
        />
      </div>
    </div>
  )
}

function MultiBrowserCategoryBars({
  distributions,
  t,
}: {
  distributions: BrowserDiff['categoryDistributions']
  t: T
}) {
  const allCategories = new Set<string>()
  for (const distribution of distributions) {
    for (const category of distribution.categories) {
      allCategories.add(category.domainCategory)
    }
  }
  const categoryList = Array.from(allCategories)

  return (
    <div className="multi-browser__category-bars">
      {categoryList.map((category) => (
        <div key={category} className="multi-browser__category-row">
          <span className="multi-browser__category-label">
            {t(`category_${category}`) || category}
          </span>
          <div className="multi-browser__category-profiles">
            {distributions.map((distribution) => {
              const entry: CategoryMixEntry | undefined =
                distribution.categories.find(
                  (item) => item.domainCategory === category,
                )
              const share = entry ? Math.round(entry.share * 100) : 0
              return (
                <div
                  key={distribution.profileId}
                  className="multi-browser__category-bar"
                  title={`${distribution.profileName}: ${share}%`}
                >
                  <span
                    className="multi-browser__category-bar-fill"
                    style={{ width: `${share}%` }}
                  />
                  <span className="multi-browser__category-bar-meta">
                    {distribution.profileName} {share}%
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}

export function ObservedInteractionsSection({
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
  const { data, loading } = useAsyncData(
    () => api.getObservedInteractions(dateRange, profileId),
    [dateRange, profileId],
  )
  const observations = data?.data ?? []

  return (
    <section className="intelligence-section observed-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('observedTitle')}</h2>
        <span className="observed-section__badge">
          {t('observedCapabilityBadge')}
        </span>
      </div>
      {data ? (
        <IntelligenceSectionMeta meta={data.meta} scopeLabel={scopeLabel} />
      ) : null}
      <p className="observed-section__disclaimer">{t('observedDisclaimer')}</p>
      {loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      ) : observations.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('observedEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody>
          <ul className="observed-list">
            {observations.slice(0, 10).map((item) => (
              <ObservedInteractionRow key={item.visitId} item={item} t={t} />
            ))}
          </ul>
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function ObservedInteractionRow({
  item,
  t,
}: {
  item: ObservedInteraction
  t: T
}) {
  const foreground =
    item.foregroundDurationMs != null
      ? formatDuration(item.foregroundDurationMs)
      : null
  const scroll =
    item.scrollingTimeMs != null ? formatDuration(item.scrollingTimeMs) : null

  return (
    <li className="observed-row">
      <div className="observed-row__main">
        <span className="observed-row__title" title={item.url}>
          {item.title ?? item.url}
        </span>
        <span className="observed-row__family">{item.browserFamily}</span>
      </div>
      <div className="observed-row__metrics">
        {foreground ? (
          <span className="observed-row__metric">
            {t('observedForeground', { duration: foreground })}
          </span>
        ) : null}
        {scroll ? (
          <span className="observed-row__metric">
            {t('observedScroll', { duration: scroll })}
          </span>
        ) : null}
        {item.keyPresses != null && item.keyPresses > 0 ? (
          <span className="observed-row__metric">
            {t('observedKeyPresses', { count: item.keyPresses })}
          </span>
        ) : null}
        {item.loadSuccessful === false ? (
          <span className="observed-row__metric observed-row__metric--warn">
            {t('observedLoadFailed')}
          </span>
        ) : null}
      </div>
    </li>
  )
}

function looksLikeUrlOrDomain(label: string) {
  const normalized = label.trim().toLowerCase()
  if (!normalized) {
    return false
  }

  return (
    normalized.includes('://') ||
    /^www\./.test(normalized) ||
    /\b[a-z0-9-]+(?:\.[a-z0-9-]+){1,}\b/.test(normalized) ||
    normalized.includes('/auth/') ||
    normalized.includes('/login') ||
    normalized.includes('callback')
  )
}

function hasMeaningfulStableSources(
  entries: StableSource[],
  landings: StableSource[],
) {
  return entries.length > 0 && landings.length > 0
}

function isMeaningfulFrictionSignal(signal: FrictionSignal) {
  if (!signal.description.trim()) {
    return false
  }

  return (
    signal.evidenceType === 'strong' ||
    (signal.occurrenceCount >= 2 &&
      ['bounce_pattern', 'excessive_reformulation', 'redirect_chain'].includes(
        signal.signalKind,
      ))
  )
}

function isSearchBackedReopenedInvestigation(item: ReopenedInvestigation) {
  const label = item.anchorLabel.trim()
  if (item.anchorType !== 'query_family') {
    return false
  }
  if (item.occurrenceCount < 2 || item.distinctDays < 2) {
    return false
  }
  if (!label || looksLikeUrlOrDomain(label)) {
    return false
  }

  return /[\s?]/.test(label) || label.length >= 12
}

function normalizeFlowStep(step: string) {
  const normalized = step
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//, '')
    .replace(/\/.*$/, '')
    .replace(/^www\./, '')
    .replace(/^m\./, '')
    .replace(/^amp\./, '')

  if (
    normalized.includes('chat.openai.') ||
    normalized.includes('chatgpt.com')
  ) {
    return 'chatgpt'
  }
  if (normalized.includes('twitter.com') || normalized.includes('x.com')) {
    return 'x.com'
  }

  return normalized
}

function isUtilityFlowStep(step: string) {
  const normalized = step.trim().toLowerCase()
  return (
    normalized.includes('localhost') ||
    normalized.includes('callback') ||
    normalized.includes('oauth') ||
    normalized.includes('consent') ||
    normalized.includes('login') ||
    normalized.includes('sign-in') ||
    normalized.includes('signin') ||
    normalized.includes('auth.')
  )
}

function isMeaningfulPathFlow(flow: PathFlow) {
  const steps = flow.flowPattern.split(/\s*(?:->|→)\s*/).filter(Boolean)
  if (flow.occurrenceCount < 2 || steps.length < 2) {
    return false
  }
  if (steps.some(isUtilityFlowStep)) {
    return false
  }

  const normalizedSteps = steps.map(normalizeFlowStep)
  if (new Set(normalizedSteps).size < 2) {
    return false
  }

  return normalizedSteps.every(
    (step, index) => index === 0 || step !== normalizedSteps[index - 1],
  )
}

function humanizeDiscoveryWeekLabel(dateKey: string, t: T) {
  const match = /^(\d{4})-W(\d{2})$/.exec(dateKey)
  if (!match) {
    return dateKey
  }

  return t('discoveryTrendWeekLabel', {
    year: Number(match[1]),
    week: Number(match[2]),
  })
}
