/**
 * The Agent space's backup page (`#/agent/backup`, `data-spaces.md` §4 and
 * `agent-history.md` §6): is this kind's data safe, and back it up now.
 * The header with its states, format versions PathKeep cannot read yet
 * (with Dismiss), the automatic-backup row (one schedule for every kind;
 * its card is in Settings › Schedule and background work), Claude Code's
 * auto-delete reminder, the sources, recent backups with their details, and
 * the sizes.
 *
 * Responsibilities: the layout, and the states that replace it: a skeleton
 * at once, a store that will not open (browsing is unaffected), an archive
 * not set up yet; and, under the header, the notice for conversations that
 * can't be read while the rest open. The page grows with the window.
 * Not responsible for: a kind that is off (`app/shell/kind-gate.tsx` shows
 * its page) or the lock (the frame is gone while locked).
 */
import { Skeleton } from '@/components/ui/skeleton'
import { CardError } from '@/components/app/section-card'
import { FluidPage } from '@/components/app/fluid-page'
import type { ArchiveRun } from '@/lib/backend-client/agent-backup'
import { agentReviewKind } from '@/lib/backend-client/data-kinds'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  useAgentKind,
  useAgentOverview,
  useAgentRuns,
  useAgentSources,
  useKindTasks,
  useTaskSummary,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { useDataKindSizes } from '@/lib/queries/data-kinds'
import { spaceById } from '@/lib/spaces'
import { ParkedNotice } from '@/features/agent-repair/parked-notice'
import { ScheduleRow } from '@/features/backup/schedule-row'
import { StoreDamage } from '@/features/data-kinds/store-damage'
import { useRetryStore } from '@/features/data-kinds/use-retry-store'
import { AgentRunSheet } from './agent-run-sheet'
import { AutoDeleteReminder } from './auto-delete-reminder'
import { BackupHeader } from './backup-header'
import { FormatNoticeCards } from './format-notice-card'
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

/** Stands for the size in the sentence until it is counted. */
const SIZE_SLOT = '\u0000'

/**
 * The size in the archive, asked for after the page is drawn (counting it
 * reads the archive files): a placeholder of about its width holds the
 * line until it comes, so nothing moves.
 */
function Footer({ sourceBytes }: { sourceBytes: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const sizes = useDataKindSizes()
  const bytes = sizes.data?.kinds.find(
    (row) => row.kind === 'agent-history',
  )?.archiveBytes
  const [before, after = ''] = t('agentBackup.footer.size', {
    size: SIZE_SLOT,
    source: format.bytes(wireNumber(sourceBytes)),
  }).split(SIZE_SLOT)
  return (
    <div className="flex flex-col gap-1.5 px-1 text-[13px] text-muted-foreground">
      <span>{t('agentBackup.footer.all')}</span>
      <span
        className="tabular"
        data-archive-size={
          bytes !== undefined
            ? 'counted'
            : sizes.isError || sizes.data
              ? 'unknown'
              : 'counting'
        }
      >
        {before}
        {bytes !== undefined ? (
          <span className="animate-in duration-200 fade-in-0">
            {format.bytes(wireNumber(bytes))}
          </span>
        ) : sizes.isError || sizes.data ? (
          // It could not be counted: the line stays whole without it.
          '—'
        ) : (
          <>
            <span className="sr-only">{t('agentBackup.footer.counting')}</span>
            <Skeleton
              aria-hidden
              className="inline-block h-3 w-12 align-[-1px]"
            />
          </>
        )}
        {after}
      </span>
    </div>
  )
}

export default function AgentBackupPage() {
  const { t } = useI18n()
  const { kinds, row, viewOnly, needsReview } = useAgentKind()
  const sources = useAgentSources()
  const approvedHere = sources.data?.pages.some((page) =>
    page.items.some((item) => item.rootId !== null),
  )
  const review = agentReviewKind(row, approvedHere)
  const overview = useAgentOverview('backup')
  const runs = useAgentRuns()
  const summary = useTaskSummary()
  // A backup paused in its last step waits there; it is still this kind's backup.
  const waiting = useKindTasks('waiting')
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
  const running =
    summary.data?.running[0] ??
    waiting.data?.pages[0]?.items.find((task) => task.state === 'paused')
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
        needsReview={needsReview}
        review={review}
      />
      <ParkedNotice row={row} />
      <FormatNoticeCards />
      {overview.isError && !data && (
        <CardError
          error={overview.error}
          onRetry={() => void overview.refetch()}
        />
      )}
      <ScheduleRow kind="agent-history" />
      {claudeHere && (
        <AutoDeleteReminder
          onlyInPathkeep={wireNumber(claude?.onlyInPathkeep)}
        />
      )}
      <SourcesCard
        overview={data}
        running={running !== undefined}
        viewOnly={viewOnly || review === 'choose-folders'}
      />
      <RecentRuns />
      {data && <Footer sourceBytes={data.sourceBytes} />}
      <AgentRunSheet />
    </FluidPage>
  )
}
