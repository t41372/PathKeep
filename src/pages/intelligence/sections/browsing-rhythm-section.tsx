/**
 * `/intelligence` wrapper around the shared calendar-based browsing rhythm card.
 */

import { useCallback, useState } from 'react'
import { BrowsingRhythmCard } from '../../../components/intelligence/browsing-rhythm-card'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  type CoreIntelligenceSectionMeta,
  type DateRange,
  type TimeRangePreset,
} from '../../../lib/core-intelligence'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { domainDayInsightsHref } from '../../../lib/core-intelligence/routes'
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
  const [trendMeta, setTrendMeta] =
    useState<CoreIntelligenceSectionMeta | null>(null)
  const handleTrendMetaChange = useCallback(
    (nextMeta: CoreIntelligenceSectionMeta | null) => {
      setTrendMeta((currentMeta) =>
        currentMeta === nextMeta ? currentMeta : nextMeta,
      )
    },
    [],
  )

  return (
    <section className="intelligence-section rhythm-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
        {trendMeta ? (
          <IntelligenceSectionMeta meta={trendMeta} scopeLabel={scopeLabel} />
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
          onTrendMetaChange={handleTrendMetaChange}
          profileId={profileId}
          summaryPreset={preset}
          t={t}
        />
      </IntelligenceSectionBody>
    </section>
  )
}
