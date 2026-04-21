/**
 * @file compare-sets-section.tsx
 * @description Renders the secondary-grid compare-sets card for query-driven page comparison journeys.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load compare-set overview data for the current scope.
 * - Preserve the existing compare-set card layout, page list, and route links.
 * - Keep compare-set-specific rendering details out of the route shell.
 *
 * ## Non-Responsibilities
 * - Does not own compare-set route composition outside this secondary card.
 * - Does not change compare-set destination grammar or shared page-list behavior.
 * - Does not fetch unbounded result lists beyond the existing capped overview payload.
 *
 * ## Dependencies
 * - `lib/core-intelligence` and `lib/core-intelligence/api` for compare-set reads.
 * - `CompareSetPageList`, `section-meta`, and `section-body` for shared UI primitives.
 * - React Router `Link` for the existing compare-set and trail destinations.
 *
 * ## Performance Notes
 * - Reuses the staged overview cache and only renders the first six compare sets, matching prior behavior.
 * - The page list remains capped, so this module stays bounded even with very large archives.
 */

import { Link } from 'react-router-dom'
import { CompareSetPageList } from '../../../../components/intelligence/compare-set-page-list'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type CompareSet,
  type DateRange,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'

/**
 * Shows search journeys where the user compared multiple candidate pages, while
 * keeping the route shell ignorant of compare-set card internals. The section
 * preserves the existing links into shared compare-set, domain-focus, and trail routes.
 *
 * @param compareSetHref Builds the shared compare-set deep-link for a card header.
 * @param dateRange The current intelligence time window.
 * @param focusedDomainHref Builds the existing domain destination with compare-set focus context.
 * @param profileId Optional profile scope for deterministic overview reads.
 * @param scopeLabel Human-readable scope string used by shared evidence metadata.
 * @param trailHref Builds the existing trail route for the related trail CTA.
 * @param t Route-local translator used by the unchanged compare-set copy.
 * @returns The compare-set section, its empty state, or the loading skeleton for the current scope.
 */
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
    {
      getCached: () => api.peekCompareSets(dateRange, profileId),
    },
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
