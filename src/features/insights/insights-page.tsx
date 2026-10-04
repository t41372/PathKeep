/**
 * Insights: deterministic statistics over a chosen range, computed locally
 * from the archive. No AI involved. Every card links into a drill-in (day,
 * site, search, page), and the drill-ins link onward to History.
 */
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { useI18n } from '@/lib/i18n'
import {
  DailyActivityCard,
  KpiRow,
  RhythmCard,
  SearchesAndRefindCard,
  TopSitesCard,
} from './insights-cards'
import {
  BreadthCard,
  BrowsersCard,
  HabitsCard,
  ThreadsCard,
} from './pattern-cards'
import { RangeToggle } from './parts'
import { useRange } from './range'

const row =
  'grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1'

export default function InsightsPage() {
  const { t } = useI18n()
  const [range, setRange] = useRange()

  return (
    <PageScroll wide>
      <PageHeader
        title={t('insights.title')}
        subtitle={t('insights.subtitle')}
        actions={<RangeToggle value={range} onChange={setRange} />}
      />
      <KpiRow range={range} />
      <div className={row}>
        <DailyActivityCard range={range} />
        <TopSitesCard range={range} />
      </div>
      <div className={row}>
        <RhythmCard range={range} />
        <SearchesAndRefindCard range={range} />
      </div>
      <h2 className="mt-3 text-[15px] font-semibold">
        {t('insights.patterns')}
      </h2>
      <div className={row}>
        <ThreadsCard range={range} />
        <BreadthCard range={range} />
      </div>
      <div className={row}>
        <HabitsCard range={range} />
        <BrowsersCard range={range} />
      </div>
    </PageScroll>
  )
}
