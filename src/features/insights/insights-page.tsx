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
import { useScope } from './range'
import { ScopeControls } from './scope-controls'

const row =
  'grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1'

export default function InsightsPage() {
  const { t } = useI18n()
  const [scope] = useScope()

  return (
    <PageScroll wide>
      <PageHeader
        title={t('insights.title')}
        subtitle={t('insights.subtitle')}
        actions={<ScopeControls />}
      />
      <KpiRow scope={scope} />
      <div className={row}>
        <DailyActivityCard scope={scope} />
        <TopSitesCard scope={scope} />
      </div>
      <div className={row}>
        <RhythmCard scope={scope} />
        <SearchesAndRefindCard scope={scope} />
      </div>
      <h2 className="mt-3 text-[15px] font-semibold">
        {t('insights.patterns')}
      </h2>
      <div className={row}>
        <ThreadsCard scope={scope} />
        <BreadthCard scope={scope} />
      </div>
      <div className={row}>
        <HabitsCard scope={scope} />
        <BrowsersCard scope={scope} />
      </div>
    </PageScroll>
  )
}
