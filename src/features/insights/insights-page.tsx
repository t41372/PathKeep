/**
 * Insights: deterministic statistics over a chosen scope, computed locally
 * from the archive. No AI involved. Grouped by the questions people ask of
 * their history: how much and where, when, what they search for, and what
 * they keep coming back to. Every card links into a drill-in (day, site,
 * search, page), and the drill-ins link onward to History.
 *
 * Opening the page reads two batches (`overview.ts`) rather than one request
 * per card.
 */
import type { ReactNode } from 'react'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { useI18n } from '@/lib/i18n'
import { BandsProvider } from './bands'
import {
  DailyActivityCard,
  KpiRow,
  RhythmCard,
  SearchesAndRefindCard,
  TopSitesCard,
} from './insights-cards'
import { ComparedPagesCard } from './compare-card'
import { ActivityMixCard } from './mix-card'
import { NewSitesCard } from './new-sites-card'
import { useOverviewBands } from './overview'
import { PathsCard } from './paths-card'
import {
  BreadthCard,
  BrowsersCard,
  HabitsCard,
  ThreadsCard,
} from './pattern-cards'
import { useScope } from './range'
import { ScopeControls } from './scope-controls'
import { ZoneNotice } from './zone-notice'
import {
  SearchEnginesCard,
  SearchLandingsCard,
  SearchOutcomesCard,
  SearchTriesCard,
  SearchWordsCard,
} from './search-cards'

function Row({ children }: { children: ReactNode }) {
  return (
    <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
      {children}
    </div>
  )
}

function Heading({ children }: { children: ReactNode }) {
  return <h2 className="mt-3 text-[15px] font-semibold">{children}</h2>
}

export default function InsightsPage() {
  const { t } = useI18n()
  const [scope] = useScope()
  const bands = useOverviewBands(scope)

  return (
    <BandsProvider value={bands}>
      <PageScroll wide>
        <PageHeader
          title={t('insights.title')}
          subtitle={t('insights.subtitle')}
          actions={<ScopeControls />}
        />
        <ZoneNotice />
        <KpiRow scope={scope} />
        <Row>
          <div className="flex min-w-0 flex-col gap-4">
            <DailyActivityCard scope={scope} />
            <ActivityMixCard scope={scope} />
          </div>
          <TopSitesCard scope={scope} />
        </Row>
        <Heading>{t('insights.sections.time')}</Heading>
        <Row>
          <RhythmCard scope={scope} />
          <NewSitesCard scope={scope} />
        </Row>
        <Heading>{t('insights.sections.searching')}</Heading>
        <Row>
          <SearchesAndRefindCard scope={scope} />
          <div className="flex min-w-0 flex-col gap-4">
            <SearchEnginesCard scope={scope} />
            <SearchWordsCard scope={scope} />
          </div>
        </Row>
        <Row>
          <SearchOutcomesCard scope={scope} />
          <SearchTriesCard scope={scope} />
        </Row>
        <Row>
          <SearchLandingsCard scope={scope} />
          <ComparedPagesCard scope={scope} />
        </Row>
        <Heading>{t('insights.patterns')}</Heading>
        <Row>
          <ThreadsCard scope={scope} />
          <HabitsCard scope={scope} />
        </Row>
        <Row>
          <PathsCard scope={scope} />
          <div className="flex min-w-0 flex-col gap-4">
            <BreadthCard scope={scope} />
            <BrowsersCard scope={scope} />
          </div>
        </Row>
      </PageScroll>
    </BandsProvider>
  )
}
