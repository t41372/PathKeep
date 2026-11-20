/**
 * Insights of the Agent space (`#/agent/insights`, `agent-history.md` §2):
 * a report to look at, so everything is open at once. Under the title the
 * five numbers, then the year (which the range does not change), then a
 * wall of cards in order of use: time and projects, tokens, when, what
 * was done, cost, and how it is all counted.
 *
 * Responsibilities: the header, the choices, which state the page is in,
 * and putting the cards in order. Not responsible for: reading the report
 * (`use-report.ts`) or what a card draws (each card's file).
 */
import { useState } from 'react'
import { FluidPage } from '@/components/app/fluid-page'
import { PageHeader } from '@/components/app/section-card'
import { NoConversations } from '@/features/agent-chats/list-states'
import { useInsightsZone } from '@/features/insights/zone'
import type { EquivalentReading } from '@/lib/backend-client/agent-insights'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import {
  useAgentProjectsSync,
  useProjectList,
} from '@/lib/queries/agent-projects'
import { CostCard, MethodNote } from './cost-card'
import { KpiRow } from './kpi-row'
import { FilterButton, ProjectLine, RangeToggle } from './scope-controls'
import { useScope, type Scope } from './scope'
import {
  EmptyReport,
  ProjectNotFound,
  ReportFailed,
  ReportSkeleton,
} from './states'
import { PresenceCard, RhythmCard, ToolTimeCard } from './time-cards'
import { TokenMixCard, TokensCard } from './token-cards'
import { useSavePreferences } from './use-preferences'
import { useInsightsReport, type InsightsReport } from './use-report'
import { Brick, Wall } from './wall'
import { WorkCard } from './work-card'
import {
  ChangesCard,
  LongestCard,
  ModelsCard,
  ToolCallsCard,
  TopProjectsCard,
} from './work-cards'
import { HomeAlerts } from '@/features/home/alerts-list'
import { PendingCard } from './pending-card'
import { YearSection } from './year-section'
import { windowKey } from './words'

const IDLE = {
  range: 'all',
  harnesses: [],
  projectId: null,
  year: null,
  zone: '',
  cost: '',
} as const

export default function AgentInsightsPage() {
  const { t } = useI18n()
  const zone = useInsightsZone().shown
  const { readable } = useAgentKind()
  const { scope, preferences, setRange, toggleTool, setProject, clearFilters } =
    useScope()
  const save = useSavePreferences()
  const [year, setYear] = useState<number | null>(null)
  const prefs = preferences.data
  const report = useInsightsReport(
    scope && prefs
      ? {
          ...scope,
          year,
          zone,
          cost: `${prefs.showCost}/${prefs.estimateCost}`,
        }
      : { ...IDLE, harnesses: [] },
    readable && scope !== null && prefs !== undefined,
  )
  useAgentProjectsSync()
  const projects = useProjectList({}, scope?.projectId != null)
  const project = projects.data?.items.find(
    (item) => item.id === scope?.projectId,
  )
  const projectGone =
    scope?.projectId != null && projects.isSuccess && project === undefined

  return (
    <FluidPage>
      <PageHeader
        title={
          <>
            {t('agentInsights.title')}
            {scope && (
              <span className="ml-1 text-[18px] font-normal tracking-normal text-muted-foreground">
                {t(windowKey(scope.range))}
              </span>
            )}
          </>
        }
        subtitle={
          scope?.projectId ? (
            <ProjectLine project={project} onClear={() => setProject(null)} />
          ) : undefined
        }
        actions={
          scope && (
            <div className="flex flex-wrap items-center justify-end gap-2">
              <RangeToggle range={scope.range} onRange={setRange} />
              <FilterButton
                scope={scope}
                project={project}
                onTool={toggleTool}
                onProject={setProject}
              />
            </div>
          )
        }
      />
      <HomeAlerts space="agent" />
      {report.started?.firstBackupRunning && (
        <p className="animate-rise -mt-2 text-[13px] text-muted-foreground">
          {t('agentInsights.firstBackup')}
        </p>
      )}
      {preferences.isError ? (
        <ReportFailed
          code={String(preferences.error.message)}
          onRetry={() => void preferences.refetch()}
        />
      ) : !scope || !prefs ? (
        <ReportSkeleton />
      ) : projectGone ? (
        <ProjectNotFound onClear={() => setProject(null)} />
      ) : (
        <Body
          firstBackupRunning={report.started?.firstBackupRunning ?? false}
          report={report}
          scope={scope}
          zone={zone}
          reading={prefs.equivalent}
          onReading={(equivalent) => void save({ equivalent })}
          onEstimate={(estimateCost) => void save({ estimateCost })}
          onRange={setRange}
          onClearFilters={clearFilters}
          onYear={setYear}
        />
      )}
    </FluidPage>
  )
}

function Body({
  firstBackupRunning,
  report,
  scope,
  zone,
  reading,
  onReading,
  onEstimate,
  onRange,
  onClearFilters,
  onYear,
}: {
  firstBackupRunning: boolean
  report: InsightsReport
  scope: Scope
  zone: string
  reading: EquivalentReading
  onReading: (reading: EquivalentReading) => void
  onEstimate: (on: boolean) => void
  onRange: (range: Scope['range']) => void
  onClearFilters: () => void
  onYear: (year: number) => void
}) {
  const { overview, series, breakdowns, work } = report.report
  const filtered = scope.harnesses.length > 0 || scope.projectId !== null
  const year = (
    <YearSection
      calendar={report.calendar.data}
      failed={report.calendar.failed}
      filters={scope}
      onYear={onYear}
      onRetry={() => report.retry('calendar')}
    />
  )

  if (report.report.failed)
    return (
      <>
        <ReportFailed
          code={report.report.failed}
          onRetry={() => report.retry('report')}
        />
        {year}
      </>
    )
  if (!overview) return <ReportSkeleton between={year} />
  const data = overview.value
  if (data.savedRoots === 0 && !overview.stale && !firstBackupRunning)
    return <NoConversations />
  if (data.conversationCount === 0)
    return overview.stale ? (
      <ReportSkeleton between={year} />
    ) : (
      <>
        <EmptyReport
          range={scope.range}
          filtered={filtered}
          onAllTime={() => onRange('all')}
          onClearFilters={onClearFilters}
        />
        {year}
      </>
    )

  const stale = overview.stale || !report.report.done
  const parts = breakdowns?.value
  return (
    <>
      {/* The rise holds full opacity when it ends, so the dimming is inside it. */}
      <div className="animate-rise">
        <div
          aria-busy={stale}
          className={cn(
            'transition-opacity duration-200',
            overview.stale && 'opacity-60',
          )}
        >
          <KpiRow
            overview={data}
            scope={scope}
            reading={reading}
            onReading={onReading}
          />
        </div>
      </div>
      {year}
      <div className="animate-rise">
        <div
          aria-busy={stale}
          className={cn(
            'transition-opacity duration-200',
            (series?.stale || breakdowns?.stale) && 'opacity-60',
          )}
        >
          <Wall>
            <Brick>
              <WorkCard series={series} work={work} scope={scope} zone={zone} />
            </Brick>
            <Brick>
              {parts ? (
                <TopProjectsCard
                  rows={parts.topProjects}
                  overview={data}
                  scope={scope}
                />
              ) : (
                <PendingCard />
              )}
            </Brick>
            <Brick>
              <TokensCard
                series={series?.value ?? null}
                scope={scope}
                zone={zone}
              />
            </Brick>
            <Brick>
              <TokenMixCard
                overview={data}
                byHarness={parts?.byHarness ?? null}
              />
            </Brick>
            {parts ? (
              <>
                <Brick>
                  <RhythmCard
                    hourOfWeek={parts.hourOfWeek}
                    overview={data}
                    scope={scope}
                  />
                </Brick>
                <Brick>
                  <PresenceCard overview={data} />
                </Brick>
                <Brick>
                  <ToolTimeCard rows={parts.byHarness} />
                </Brick>
                <Brick>
                  <LongestCard rows={parts.longest} />
                </Brick>
                <Brick>
                  <ChangesCard breakdowns={parts} overview={data} />
                </Brick>
                <Brick>
                  <ToolCallsCard calls={parts.toolCalls} />
                </Brick>
                <Brick>
                  <ModelsCard models={parts.models} />
                </Brick>
              </>
            ) : (
              Array.from({ length: 7 }, (_, index) => (
                <Brick key={index}>
                  <PendingCard />
                </Brick>
              ))
            )}
            {data.cost && (
              <Brick>
                <CostCard cost={data.cost} onEstimate={onEstimate} />
              </Brick>
            )}
            <Brick>
              <MethodNote />
            </Brick>
          </Wall>
        </div>
      </div>
    </>
  )
}
