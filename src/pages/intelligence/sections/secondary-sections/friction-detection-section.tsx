/**
 * @file friction-detection-section.tsx
 * @description Renders the secondary-grid card that surfaces repeated signs of browsing friction.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load deterministic friction signals for the active intelligence scope.
 * - Filter out weak or empty friction evidence so the card stays trustworthy.
 * - Preserve the existing domain drilldown links and card copy verbatim.
 *
 * ## Non-Responsibilities
 * - Does not invent new friction heuristics outside the shared secondary-section rules.
 * - Does not manage page-level section ordering or loading orchestration.
 * - Does not mutate any runtime or review state.
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` for friction-signal reads and cache peeks.
 * - `./heuristics` for the shared visibility filter that keeps this card honest.
 * - `../section-body` and `components/intelligence/section-meta` for card presentation.
 *
 * ## Performance Notes
 * - Filters bounded overview summaries, not raw event streams.
 * - Caps rendered signal cards at eight rows to prevent oversized secondary cards on large archives.
 */

import { Link } from 'react-router-dom'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type FrictionSignal,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'
import { isMeaningfulFrictionSignal } from './heuristics'

type FrictionDetectionSectionProps = {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Surfaces repeated moments where browsing likely felt costly or confusing,
 * while suppressing low-evidence noise that would read like speculation.
 *
 * @param dateRange Active intelligence time window used for the deterministic overview request.
 * @param domainHref Shared route builder for domain deep dives.
 * @param profileId Optional profile scope; `null` means the aggregate intelligence scope.
 * @param scopeLabel Localized scope summary shown in the freshness metadata.
 * @param t Route-local translator for all visible labels.
 * @returns A friction card, an empty/loading state, or `null` when no meaningful signal survives filtering.
 */
export function FrictionDetectionSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: FrictionDetectionSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getFrictionSignals(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekFrictionSignals(dateRange, profileId),
    },
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
