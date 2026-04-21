/**
 * `/intelligence` wrapper around the shared calendar-based browsing rhythm card.
 */

import { BrowsingRhythmCard } from '../../../components/intelligence/browsing-rhythm-card'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type TimeRangePreset,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { domainDayInsightsHref } from '../../../lib/intelligence'
import { IntelligenceSectionBody } from './section-body'
import type { T } from './shared'

export function BrowsingRhythmSection({
  dateRange,
  dayHref,
  language,
  preset,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  dayHref: (date: string) => string
  language: ResolvedLanguage
  preset: TimeRangePreset
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const trendResult = useAsyncData(
    () => api.getDiscoveryTrend(dateRange, profileId, 'day'),
    [dateRange, profileId],
    {
      getCached: () => api.peekDiscoveryTrend(dateRange, profileId, 'day'),
    },
  )

  return (
    <section className="intelligence-section rhythm-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
        {trendResult.data ? (
          <IntelligenceSectionMeta
            meta={trendResult.data.meta}
            scopeLabel={scopeLabel}
          />
        ) : null}
      </div>
      <p className="intelligence-section__help">{t('rhythmHelp')}</p>
      <IntelligenceSectionBody className="rhythm-panel" variant="workbench">
        <BrowsingRhythmCard
          dateRange={dateRange}
          dayDomainHref={(domain, date) =>
            domainDayInsightsHref(domain, date, profileId)
          }
          dayHref={dayHref}
          language={language}
          mode="range"
          profileId={profileId}
          summaryPreset={preset}
          t={t}
        />
      </IntelligenceSectionBody>
    </section>
  )
}
