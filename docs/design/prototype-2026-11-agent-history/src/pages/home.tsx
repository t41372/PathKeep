/**
 * Home: across every data type that is on. What is kept (one card per type),
 * the agent's recent work and this week, the year, on-this-day, sources.
 * With only one type on, it is that type's home; with none, it says how to
 * pick one. The upgrade intro and the enable flow live here too.
 */
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { useProto } from '~/proto-state'
import {
  AgentIntroCard,
  BackupNow,
  BackupPill,
  ContinueCard,
  NothingOn,
  OnThisDayCard,
  SourcesCard,
  StatusCards,
  WeekCard,
  YearCard,
} from './home/cards'

export default function HomePage() {
  const { enabledKinds, agentEnabled } = useProto()
  const agentOn = enabledKinds.includes('agent')
  const any = enabledKinds.length > 0

  return (
    <PageScroll>
      <PageHeader
        eyebrow="2026 年 11 月 13 日 星期五"
        title="午安"
        actions={
          any ? (
            <>
              <BackupPill />
              <BackupNow />
            </>
          ) : undefined
        }
      />
      {!agentEnabled && <AgentIntroCard />}
      {!any && agentEnabled && <NothingOn />}
      <StatusCards />
      {agentOn && (
        <div className="grid grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">
          <ContinueCard />
          <WeekCard />
        </div>
      )}
      {any && <YearCard />}
      {any && (
        <div className="grid grid-cols-2 gap-4 max-[900px]:grid-cols-1">
          <OnThisDayCard />
          <SourcesCard />
        </div>
      )}
    </PageScroll>
  )
}
