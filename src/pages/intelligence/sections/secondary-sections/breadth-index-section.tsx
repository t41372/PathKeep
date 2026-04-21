/**
 * @file breadth-index-section.tsx
 * @description Renders the secondary-grid breadth index card for concentration versus browsing spread.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load the bounded breadth-index overview payload for the current route scope.
 * - Preserve the existing empty, loading, and metadata states for the breadth card.
 * - Keep the breadth score presentation and explanatory copy local to this card.
 *
 * ## Non-Responsibilities
 * - Does not decide whether the secondary grid should render this card at the route level.
 * - Does not own shared intelligence routing or domain deep-link grammar.
 * - Does not mutate intelligence state or trigger rebuilds.
 *
 * ## Dependencies
 * - `lib/core-intelligence` for typed overview data loading.
 * - `lib/core-intelligence/api` for the breadth-index read path.
 * - `section-meta`, `section-body`, and route-local `shared` helpers for chrome and formatting.
 *
 * ## Performance Notes
 * - Reads from the staged, bounded overview payload and reuses cached data when available.
 * - Rendering stays constant-size, so the card remains safe under large archive sizes.
 */

import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type BreadthIndex,
  type DateRange,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'

/**
 * Explains whether the current scope is concentrated in a few domains or spread
 * across many. This helps the secondary grid surface "how broad was this slice?"
 * without forcing the route shell to understand the card's internal score model.
 *
 * @param dateRange The bounded time window the current intelligence route is reading.
 * @param profileId Optional profile scope; `null` keeps the archive-wide behavior.
 * @param scopeLabel Human-readable scope label used by shared evidence metadata.
 * @param t Route-local translator so copy stays identical to the existing section.
 * @returns The breadth card, its empty state, or its loading skeleton for the current scope.
 */
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
    {
      getCached: () => api.peekBreadthIndex(dateRange, profileId),
    },
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
