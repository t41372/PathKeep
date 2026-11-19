/**
 * The Agent space's backup page (`#/agent/backup`, `data-spaces.md` §4 and
 * `agent-history.md` §6): is this kind's data safe, and back it up now.
 * The header with its states, the automatic backup card (the same one the
 * browsing page has: one schedule for every kind), Claude Code's
 * auto-delete reminder, the sources, recent backups with their details, and
 * the sizes.
 *
 * Responsibilities: the layout, and the states that replace it: a skeleton
 * at once, a store that will not open (browsing is unaffected), an archive
 * not set up yet. The page grows with the window: on a wide one the
 * schedule moves beside the sources instead of above them.
 * Not responsible for: a kind that is off (`app/shell/kind-gate.tsx` shows
 * its page) or the lock (the frame is gone while locked).
 */
import { Skeleton } from '@/components/ui/skeleton'
import { CardError } from '@/components/app/section-card'
import { FluidPage } from '@/components/app/fluid-page'
import type { ArchiveRun } from '@/lib/backend-client/agent-backup'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  useAgentKind,
  useAgentOverview,
  useAgentRuns,
  useTaskSummary,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { spaceById } from '@/lib/spaces'
import { ScheduleCard } from '@/features/backup/schedule-card'
import { StoreDamage } from '@/features/data-kinds/store-damage'
import { useRetryStore } from '@/features/data-kinds/use-retry-store'
import { AgentRunSheet } from './agent-run-sheet'
import { AutoDeleteReminder } from './auto-delete-reminder'
import { BackupHeader } from './backup-header'
import { RecentRuns } from './recent-runs'
import { SourcesCard } from './sources-card'

function PageSkeleton() {
  return (
    <div aria-hidden className="flex flex-col gap-5">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-8 w-40" />
        <Skeleton className="h-3.5 w-80" />
      </div>
      <Skeleton className="h-28 w-full rounded-xl" />
      <Skeleton className="h-44 w-full rounded-xl" />
    </div>
  )
}

/** The newest backup of this kind that has ended; running ones are not "last". */
function lastFinished(runs: readonly ArchiveRun[], fallback?: ArchiveRun) {
  const ended = runs.find(
    (run) => run.scope === 'agent-history' && run.result !== 'running',
  )
  if (ended) return ended
  return fallback && fallback.result !== 'running' ? fallback : undefined
}

function Footer({
  archiveBytes,
  sourceBytes,
}: {
  archiveBytes: string
  sourceBytes: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  return (
    <div className="flex flex-col gap-1.5 px-1 text-[13px] text-muted-foreground">
      <span>{t('agentBackup.footer.all')}</span>
      <span className="tabular">
        {t('agentBackup.footer.size', {
          size: format.bytes(wireNumber(archiveBytes)),
          source: format.bytes(wireNumber(sourceBytes)),
        })}
      </span>
    </div>
  )
}

export default function AgentBackupPage() {
  const { t } = useI18n()
  const { kinds, row, viewOnly } = useAgentKind()
  const overview = useAgentOverview('backup')
  const runs = useAgentRuns()
  const summary = useTaskSummary()
  const retry = useRetryStore()
  const space = spaceById('agent')!

  if (!row) {
    return (
      <FluidPage>
        {kinds.isError ? (
          <CardError error={kinds.error} onRetry={() => void kinds.refetch()} />
        ) : (
          <PageSkeleton />
        )}
      </FluidPage>
    )
  }
  if (row.availability === 'unavailable') {
    return (
      <FluidPage>
        <h1 className="text-[30px] leading-tight font-semibold tracking-[-0.02em]">
          {t('agentBackup.title')}
        </h1>
        <StoreDamage
          space={space}
          row={row}
          busy={retry.busy}
          onRetry={() => void retry.retry()}
        />
      </FluidPage>
    )
  }
  if (row.availability === 'not-initialized') {
    return (
      <FluidPage>
        <p className="text-[13px] text-muted-foreground">
          {t('dataKinds.manage.notInitialized')}
        </p>
      </FluidPage>
    )
  }

  const data = overview.data
  const running = summary.data?.running[0]
  const items = runs.data?.pages.flatMap((page) => page.items) ?? []
  const claude = data?.perTool.find((tool) => tool.tool === 'claude-code')
  const claudeHere =
    data?.sources.some((source) => source.tool === 'claude-code') ||
    wireNumber(claude?.roots) > 0

  return (
    <FluidPage>
      <BackupHeader
        overview={data}
        lastFinished={lastFinished(items, data?.lastRun)}
        running={running}
        paused={row.state === 'paused'}
        viewOnly={viewOnly}
      />
      {overview.isError && !data && (
        <CardError
          error={overview.error}
          onRetry={() => void overview.refetch()}
        />
      )}
      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(340px,400px)] xl:items-start">
        <div className="flex min-w-0 flex-col gap-5 xl:col-start-2 xl:row-start-1">
          <ScheduleCard />
        </div>
        <div className="flex min-w-0 flex-col gap-5 xl:col-start-1 xl:row-start-1">
          {claudeHere && (
            <AutoDeleteReminder
              onlyInPathkeep={wireNumber(claude?.onlyInPathkeep)}
            />
          )}
          <SourcesCard
            overview={data}
            running={running !== undefined}
            viewOnly={viewOnly}
          />
          <RecentRuns />
        </div>
      </div>
      {data && (
        <Footer
          archiveBytes={data.archiveBytes}
          sourceBytes={data.sourceBytes}
        />
      )}
      <AgentRunSheet />
    </FluidPage>
  )
}
