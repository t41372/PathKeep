/**
 * `/intelligence` wrapper around the shared calendar-based browsing rhythm card.
 */

import { BrowsingRhythmCard } from '../../../components/intelligence/browsing-rhythm-card'
import type { DateRange } from '../../../lib/core-intelligence'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { IntelligenceSectionBody } from './section-body'
import type { T } from './shared'

export function BrowsingRhythmSection({
  dateRange,
  dayHref,
  language,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  dayHref: (date: string) => string
  language: ResolvedLanguage
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  return (
    <section className="intelligence-section rhythm-section">
      <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
      <p className="intelligence-section__help">{t('rhythmHelp')}</p>
      <IntelligenceSectionBody className="rhythm-panel" variant="workbench">
        <BrowsingRhythmCard
          dateRange={dateRange}
          dayHref={dayHref}
          language={language}
          mode="range"
          profileId={profileId}
          scopeLabel={scopeLabel}
          t={t}
        />
      </IntelligenceSectionBody>
    </section>
  )
}
