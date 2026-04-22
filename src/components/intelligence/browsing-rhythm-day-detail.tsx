/**
 * @file browsing-rhythm-day-detail.tsx
 * @description Selected-day detail rail for the browsing-rhythm card.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Render the selected-day preview, summary badge, and direct detail CTA.
 * - Keep the day-specific inline detail layout separate from the calendar grid owner.
 *
 * ## Not responsible for
 * - Fetching the day-insight payload.
 * - Building the calendar heatmap or year controls.
 * - Owning translation catalogs or route definitions.
 *
 * ## Dependencies
 * - Depends on the shared day-insight card primitives and translation labels.
 * - Reuses the core intelligence category label helper for the activity mix legend.
 *
 * ## Performance notes
 * - The component is render-only; the parent owns the async data lifecycle and selected-day state.
 */

import { Link } from 'react-router-dom'
import {
  RhythmActivityProportionBar,
  RhythmHourStrip,
} from './browsing-rhythm-detail'
import type { DayInsights } from '../../lib/core-intelligence'
import { intelligenceCategoryLabel } from '../../pages/intelligence/copy'
import type { BrowsingRhythmTranslator } from './browsing-rhythm-card-helpers'

/**
 * Renders the selected-day preview so the main card can keep the calendar and detail ownership separate.
 */
export function BrowsingRhythmDayDetail({
  dateKey,
  dayDomainHref,
  dayHref,
  detail,
  error,
  loading,
  language,
  t,
}: {
  dateKey: string
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  detail: DayInsights | null
  error: string | null
  loading: boolean
  language: string
  t: BrowsingRhythmTranslator
}) {
  return (
    <div className="rhythm-day-detail" data-testid="browsing-rhythm-day-detail">
      <div className="rhythm-day-detail__header">
        <div>
          <h3 className="rhythm-day-detail__title">
            {t('rhythmDaySummaryTitle', { date: dateKey })}
          </h3>
          <p className="rhythm-day-detail__subtitle">
            {detail
              ? t('rhythmDayVisits', {
                  count: detail.digestSummary.totalVisits.value,
                })
              : t('rhythmDetailLoading')}
          </p>
        </div>
        <div className="rhythm-day-detail__actions">
          {detail ? (
            <span className="rhythm-day-detail__badge">
              {t('rhythmDayNewSites', {
                count: detail.digestSummary.newDomains.value,
              })}
            </span>
          ) : null}
          <Link className="rhythm-day-detail__link" to={dayHref(dateKey)}>
            {t('rhythmViewDetails')}
          </Link>
        </div>
      </div>

      {loading ? (
        <div className="rhythm-day-detail__skeletons">
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
        </div>
      ) : error ? (
        <p className="rhythm-day-detail__empty">{error}</p>
      ) : detail ? (
        <div className="rhythm-day-detail__content">
          <div className="rhythm-day-detail__block">
            <span className="rhythm-day-detail__stat-label">
              {t('rhythmDayHoursTitle')}
            </span>
            {detail.hourlyActivity.some((bucket) => bucket.visitCount > 0) ? (
              <RhythmHourStrip
                date={dateKey}
                hourly={detail.hourlyActivity}
                t={t}
              />
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('rhythmDayNoHourlyData')}
              </p>
            )}
          </div>

          <div className="rhythm-day-detail__block">
            <span className="rhythm-day-detail__stat-label">
              {t('dayInsightsTopSitesTitle')}
            </span>
            {detail.topSites.length > 0 ? (
              <div className="rhythm-day-detail__site-list">
                {detail.topSites.slice(0, 6).map((site) => {
                  const label = site.displayName ?? site.registrableDomain

                  return dayDomainHref ? (
                    <Link
                      key={site.registrableDomain}
                      className="rhythm-day-detail__site-chip"
                      to={dayDomainHref(site.registrableDomain, dateKey)}
                    >
                      {label}
                    </Link>
                  ) : (
                    <span
                      key={site.registrableDomain}
                      className="rhythm-day-detail__site-chip"
                    >
                      {label}
                    </span>
                  )
                })}
              </div>
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('rhythmDayNoSites')}
              </p>
            )}
          </div>

          <div className="rhythm-day-detail__block">
            <span className="rhythm-day-detail__stat-label">
              {t('dayInsightsActivityMixTitle')}
            </span>
            {detail.activityMix.categories.length > 0 ? (
              <RhythmActivityProportionBar
                categories={detail.activityMix.categories}
                categoryLabel={(domainCategory) =>
                  intelligenceCategoryLabel(
                    language as 'en' | 'zh-CN' | 'zh-TW',
                    t,
                    domainCategory,
                  )
                }
                language={language as 'en' | 'zh-CN' | 'zh-TW'}
                t={t}
              />
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('activityMixEmpty')}
              </p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  )
}
