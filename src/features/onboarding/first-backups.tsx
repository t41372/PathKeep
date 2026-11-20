/**
 * The first backup of each chosen kind of data, one line each: while it
 * runs, its progress in its own units (browsing: which source of how many,
 * and records; Agent conversations: files found, then saved, then
 * conversations, under a bar that slides since no total is known), and
 * once it ended, its result in its own unit. Numbers of different kinds
 * are never added together. A kind that failed or did not save everything
 * says why and leads to its own backup page. The Agent save, which can take
 * long the first time, can be left to go on in the background.
 *
 * Not responsible for: running the backups (`use-finish-setup.ts`) or the
 * lines setup shows when browsing history is the only kind (today's
 * progress card and result, `done-step.tsx`).
 */
import {
  ArrowDownLeft,
  Check,
  CircleAlert,
  type LucideIcon,
} from 'lucide-react'
import { useBackupRunner } from '@/app/backup-runner'
import { Button } from '@/components/ui/button'
import { taskProgressText, taskReasonText } from '@/features/agent-backup/words'
import type { ArchiveRunDetail } from '@/lib/backend-client/agent-backup'
import type { DataKindId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, type Translator } from '@/lib/i18n'
import {
  useAgentBackupSync,
  useTaskSummary,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { spaceOfKind } from '@/lib/spaces'
import type { AgentRun, BrowsingRun, KindRuns } from './use-finish-setup'

/** Conversations the first save saved: new ones and ones with new turns. */
function savedConversations(detail: ArchiveRunDetail) {
  const summary = 'newRoots' in detail.summary ? detail.summary : null
  return summary
    ? wireNumber(summary.newRoots) + wireNumber(summary.updatedRoots)
    : 0
}

/** One line per chosen kind while the first backup runs. */
export function KindProgress({
  kinds,
  runs,
  onKeepGoing,
}: {
  kinds: DataKindId[]
  runs: KindRuns
  /** Stops waiting for the Agent save; it goes on in the background. */
  onKeepGoing: () => void
}) {
  return (
    <ul className="flex animate-rise flex-col gap-3.5 rounded-xl border bg-card px-[18px] py-3.5 shadow-card">
      {kinds.map((kind) =>
        kind === 'browsing' ? (
          <BrowsingProgress key={kind} run={runs.browsing} />
        ) : (
          <AgentProgress
            key={kind}
            run={runs.agent}
            onKeepGoing={onKeepGoing}
          />
        ),
      )}
    </ul>
  )
}

function Line({
  kind,
  text,
  count,
  mark,
  percent,
  action,
}: {
  kind: DataKindId
  text: string
  /** `processed / records`, when the backup knows them. */
  count?: string
  /** `background`: still running, but nothing here waits for it. */
  mark?: 'done' | 'failed' | 'background'
  /** `null`: no total is known, so the bar slides. */
  percent?: number | null
  /** A button under the bar. */
  action?: React.ReactNode
}) {
  const { t } = useI18n()
  const name = t(spaceOfKind(kind)!.name)
  return (
    <li data-kind={kind} className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <span className="flex min-w-0 items-baseline gap-1.5">
          <span className="shrink-0 font-medium">{name}</span>
          <span className="shrink-0 text-muted-foreground">·</span>
          {mark === 'done' && (
            <Check
              className="size-3.5 shrink-0 self-center text-green"
              strokeWidth={2.5}
              aria-hidden
            />
          )}
          {mark === 'failed' && (
            <CircleAlert
              className="size-3.5 shrink-0 self-center text-destructive"
              aria-hidden
            />
          )}
          {mark === 'background' && (
            <ArrowDownLeft
              className="size-3.5 shrink-0 self-center text-muted-foreground"
              aria-hidden
            />
          )}
          <span aria-live="polite" className="truncate">
            {text}
          </span>
        </span>
        {count && (
          <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
            {count}
          </span>
        )}
      </div>
      {mark === undefined && (
        <div
          role="progressbar"
          aria-label={t('onboarding.done.progressOf', { kind: name })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent == null ? undefined : Math.round(percent)}
          className="h-1 overflow-hidden rounded-full bg-muted"
        >
          <div
            className={
              percent == null
                ? 'h-full w-1/3 animate-indeterminate rounded-full bg-brand'
                : 'h-full rounded-full bg-brand transition-[width] duration-300'
            }
            style={
              percent == null
                ? undefined
                : { width: `${Math.min(100, Math.max(2, percent))}%` }
            }
          />
        </div>
      )}
      {action}
    </li>
  )
}

function BrowsingProgress({ run }: { run: BrowsingRun | undefined }) {
  const { t } = useI18n()
  const format = useFormat()
  const { progress } = useBackupRunner()
  if (run?.status === 'ended')
    return (
      <Line
        kind="browsing"
        mark="done"
        text={t('onboarding.done.result.visits', {
          count: run.report.run?.newVisits ?? 0,
        })}
      />
    )
  if (run?.status === 'failed')
    return (
      <Line
        kind="browsing"
        mark="failed"
        text={t('onboarding.done.result.failed')}
      />
    )
  const total = progress?.totalProfiles ?? 0
  const percent =
    progress?.progressPercent ??
    (total > 0 ? (progress!.completedProfiles / total) * 100 : null)
  const processed = progress?.processedRecords
  const records = progress?.totalRecords
  return (
    <Line
      kind="browsing"
      text={[
        total > 0
          ? t('backup.progress.title', {
              done: Math.min(progress!.completedProfiles + 1, total),
              total,
            })
          : t('backup.progress.starting'),
        progress?.sourceLabel,
      ]
        .filter(Boolean)
        .join(' · ')}
      count={
        processed != null && records != null && records > 0
          ? `${format.number(processed)} / ${format.number(records)}`
          : undefined
      }
      percent={percent}
    />
  )
}

function AgentProgress({
  run,
  onKeepGoing,
}: {
  run: AgentRun | undefined
  onKeepGoing: () => void
}) {
  const { t } = useI18n()
  if (run?.status === 'saving')
    return <AgentSaving taskId={run.save.taskId} onKeepGoing={onKeepGoing} />
  if (run?.status === 'continued')
    return (
      <Line
        kind="agent-history"
        mark="background"
        text={t('enableAgent.run.continued')}
      />
    )
  if (run?.status === 'ended')
    return (
      <Line
        kind="agent-history"
        mark="done"
        text={t('onboarding.done.result.conversations', {
          count: savedConversations(run.detail),
        })}
      />
    )
  if (run?.status === 'failed')
    return (
      <Line
        kind="agent-history"
        mark="failed"
        text={t('onboarding.done.result.failed')}
      />
    )
  return (
    <Line
      kind="agent-history"
      text={t('onboarding.done.agentPreparing')}
      percent={null}
    />
  )
}

/**
 * The save as its task reports it. Nothing else listens to the backend's
 * backup events before the app opens, so this does while it shows.
 */
function AgentSaving({
  taskId,
  onKeepGoing,
}: {
  taskId: string
  onKeepGoing: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  useAgentBackupSync(ignore)
  const summary = useTaskSummary()
  const task = summary.data?.running.find((item) => item.taskId === taskId)
  return (
    <Line
      kind="agent-history"
      text={
        task
          ? taskProgressText(t, format.bytes, task)
          : t('agentTasks.progress.looking')
      }
      percent={null}
      action={
        <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
          <p className="text-xs text-muted-foreground">
            {t('onboarding.done.agentLong')}
          </p>
          <Button size="sm" variant="outline" onClick={onKeepGoing}>
            {t('enableAgent.run.continue')}
          </Button>
        </div>
      }
    />
  )
}

function ignore() {}

/** How each chosen kind's first backup ended, once setup is done. */
export function KindResults({
  kinds,
  runs,
  onOpenBackup,
}: {
  kinds: DataKindId[]
  runs: KindRuns
  onOpenBackup: (kind: DataKindId) => void
}) {
  return (
    <ul className="flex flex-col gap-2">
      {kinds.map((kind) => (
        <KindResult
          key={kind}
          kind={kind}
          runs={runs}
          onOpenBackup={() => onOpenBackup(kind)}
        />
      ))}
    </ul>
  )
}

function KindResult({
  kind,
  runs,
  onOpenBackup,
}: {
  kind: DataKindId
  runs: KindRuns
  onOpenBackup: () => void
}) {
  const { t } = useI18n()
  const name = t(spaceOfKind(kind)!.name)
  const outcome = describe(kind, runs, t)
  if (!outcome) return null
  const Icon: LucideIcon = outcome.problem ? CircleAlert : Check
  return (
    <li data-kind={kind} className="flex items-start gap-2">
      <Icon
        aria-hidden
        className={cn(
          'mt-0.5 size-4 shrink-0',
          outcome.problem ? 'text-brand' : 'text-green',
        )}
        strokeWidth={outcome.problem ? 2 : 2.5}
      />
      <span className="flex min-w-0 flex-col items-start gap-1">
        <span>
          {t('onboarding.done.result.line', {
            kind: name,
            result: outcome.text,
          })}
        </span>
        {outcome.problem && (
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0 text-[13px]"
            onClick={onOpenBackup}
          >
            {t('onboarding.done.result.openBackup', { kind: name })}
          </Button>
        )}
      </span>
    </li>
  )
}

function describe(
  kind: DataKindId,
  runs: KindRuns,
  t: Translator,
): { text: string; problem: boolean } | null {
  if (kind === 'browsing') {
    const run = runs.browsing
    if (run?.status === 'ended')
      return {
        text: t('onboarding.done.result.visits', {
          count: run.report.run?.newVisits ?? 0,
        }),
        problem: false,
      }
    if (run?.status === 'failed')
      return {
        text: t('onboarding.done.result.failedBecause', {
          reason: run.message,
        }),
        problem: true,
      }
    return null
  }
  const run = runs.agent
  if (run?.status === 'continued')
    return { text: t('onboarding.done.result.continued'), problem: false }
  if (run?.status === 'failed')
    return {
      text: t('onboarding.done.result.failedBecause', { reason: run.message }),
      problem: true,
    }
  if (run?.status !== 'ended') return null
  const { detail } = run
  const count = savedConversations(detail)
  if (detail.result === 'success')
    return {
      text: t('onboarding.done.result.conversations', { count }),
      problem: false,
    }
  if (detail.result === 'partial')
    return {
      text: t('onboarding.done.result.partial', { count }),
      problem: true,
    }
  const reason = taskReasonText(t, detail.task?.reason)
  return {
    text: reason
      ? t('onboarding.done.result.stoppedBecause', { reason })
      : t('onboarding.done.result.stopped'),
    problem: true,
  }
}
