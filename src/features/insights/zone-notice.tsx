/**
 * Tells the user when the computer is in another time zone than Insights,
 * and lets them switch Insights to it (Insights are built again) or keep
 * Insights' zone (the notice stays away while the computer stays there).
 * Shown on Insights and in Settings → General.
 */
import { Globe } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { useChooseInsightsZone, useZoneMismatch, zoneText } from './zone'

export function ZoneNotice() {
  const { t, locale } = useI18n()
  const mismatch = useZoneMismatch()
  const { choose, saving } = useChooseInsightsZone()
  if (!mismatch) return null
  const both = [mismatch.insights, mismatch.computer]
  const insights = zoneText(mismatch.insights, locale, t, both)
  const computer = zoneText(mismatch.computer, locale, t, both)

  return (
    <div
      role="status"
      className="flex flex-col gap-3 rounded-xl bg-brand-soft p-3.5 text-[13px]"
    >
      <div className="flex items-start gap-2.5">
        <Globe className="mt-0.5 size-4 shrink-0 text-brand" aria-hidden />
        <div className="flex flex-col gap-0.5">
          <p className="font-medium">
            {t('insights.zone.differs', { insights, computer })}
          </p>
          <p className="text-muted-foreground">
            {t('insights.zone.differsNote')}
          </p>
        </div>
      </div>
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={saving}
          onClick={() => void choose(mismatch.insights)}
        >
          {t('insights.zone.keep', { zone: insights })}
        </Button>
        <Button
          size="sm"
          disabled={saving}
          onClick={() => void choose(mismatch.computer)}
        >
          {saving
            ? t('insights.zone.saving')
            : t('insights.zone.use', { zone: computer })}
        </Button>
      </div>
    </div>
  )
}
