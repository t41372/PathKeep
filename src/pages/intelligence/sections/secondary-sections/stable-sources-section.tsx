/**
 * @file stable-sources-section.tsx
 * @description Renders the secondary-grid card that surfaces repeated entry and landing domains.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Load stable-source overview data for the active intelligence scope.
 * - Hide the card when the signal is too weak to read as a real pattern.
 * - Render the entry and landing columns with the same domain routing contract as before.
 *
 * ## Non-Responsibilities
 * - Does not decide secondary-grid ordering or page-level composition.
 * - Does not normalize domain routing grammar beyond calling the provided href builder.
 * - Does not own cross-card heuristics outside stable-source visibility.
 *
 * ## Dependencies
 * - `lib/core-intelligence/api` for deterministic overview reads and cache peeks.
 * - `./heuristics` for the stable-source visibility rule shared across the split modules.
 * - `../section-body` and `components/intelligence/section-meta` for route-local card chrome.
 *
 * ## Performance Notes
 * - Reads bounded overview payloads and keeps filtering O(n) over the already capped result set.
 * - Limits visible rows per column so the secondary grid stays scroll-bounded on large archives.
 */

import { Link } from 'react-router-dom'
import { IntelligenceSectionMeta } from '../../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type StableSource,
} from '../../../../lib/core-intelligence'
import * as api from '../../../../lib/core-intelligence/api'
import { IntelligenceSectionBody } from '../section-body'
import type { T } from '../shared'
import { hasMeaningfulStableSources } from './heuristics'

type StableSourcesSectionProps = {
  dateRange: DateRange
  domainHref: (domain: string) => string
  profileId: string | null
  scopeLabel: string
  t: T
}

/**
 * Shows the domains that repeatedly start or finish browsing trails within the
 * current scope, but only when both sides of the pattern exist.
 *
 * @param dateRange Active intelligence time window used for the deterministic overview request.
 * @param domainHref Shared route builder for domain deep dives.
 * @param profileId Optional profile scope; `null` means the current aggregate view.
 * @param scopeLabel Localized scope summary passed through to freshness metadata.
 * @param t Route-local translator for all visible labels.
 * @returns A stable-sources card, an empty/loading state, or `null` when the signal is too weak to show.
 */
export function StableSourcesSection({
  dateRange,
  domainHref,
  profileId,
  scopeLabel,
  t,
}: StableSourcesSectionProps) {
  const { data, loading } = useAsyncData(
    () => api.getStableSources(dateRange, profileId),
    [dateRange, profileId],
    {
      getCached: () => api.peekStableSources(dateRange, profileId),
    },
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
