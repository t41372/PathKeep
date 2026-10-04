/**
 * Insights: deterministic statistics over a chosen range, computed locally
 * from the archive. No AI involved.
 */
import { useSearchParams } from 'react-router-dom'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useI18n } from '@/lib/i18n'
import {
  DailyActivityCard,
  KpiRow,
  RhythmCard,
  SearchesAndRefindCard,
  TopSitesCard,
} from './insights-cards'
import { rangeDays, type RangeId } from './queries'

const ranges = Object.keys(rangeDays) as RangeId[]

export default function InsightsPage() {
  const { t } = useI18n()
  const [params, setParams] = useSearchParams()
  const requested = params.get('range') as RangeId | null
  const range: RangeId = requested && requested in rangeDays ? requested : 'd30'

  return (
    <PageScroll wide>
      <PageHeader
        title={t('insights.title')}
        subtitle={t('insights.subtitle')}
        actions={
          <ToggleGroup
            type="single"
            value={range}
            aria-label={t('insights.range.label')}
            onValueChange={(value) =>
              value && setParams({ range: value }, { replace: true })
            }
            className="rounded-[9px] bg-muted p-[3px]"
          >
            {ranges.map((id) => (
              <ToggleGroupItem
                key={id}
                value={id}
                className="h-[30px] rounded-[7px] px-3 text-[13px] data-[state=on]:bg-card data-[state=on]:shadow-card"
              >
                {t(`insights.range.${id}`)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        }
      />
      <KpiRow range={range} />
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <DailyActivityCard range={range} />
        <TopSitesCard range={range} />
      </div>
      <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
        <RhythmCard range={range} />
        <SearchesAndRefindCard range={range} />
      </div>
    </PageScroll>
  )
}
