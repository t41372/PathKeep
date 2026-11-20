/**
 * One row of the background tasks page (`data-spaces.md` §5.2): which
 * space the task serves (its tile; work on the whole archive says so), its
 * name, and how it stands: a percentage while a total is known, or the
 * time; a bar while it runs or is paused; one line of what it is doing and
 * why it stopped; a conversation backup's format notices, each with its own
 * Dismiss; and the buttons its state allows (`task-actions.tsx`).
 *
 * Responsibilities: drawing a row of either kind. A conversation backup's
 * row keeps every word the Settings list had, since the same backups were
 * listed there. Not responsible for: what a button does.
 */
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import type { KindTask, TaskGroup } from '@/lib/backend-client/agent-backup'
import type { BrowsingTask } from '@/lib/backend-client/background-tasks'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { isActive } from '@/lib/queries/agent-backup'
import { spaceBackupPath, spaceOfKind } from '@/lib/spaces'
import type { BackgroundTask } from '@/app/shell/use-background-tasks'
import { SpaceTile } from '@/app/shell/space-switcher'
import { NoticeLine } from '@/features/agent-backup/notice-line'
import {
  taskName,
  taskProgressText,
  taskReasonText,
} from '@/features/agent-backup/words'
import {
  browsingEnded,
  browsingStateText,
  browsingTaskName,
  type TaskEntry,
} from './task-entry'
import { AgentActions, BrowsingActions, type Controls } from './task-actions'

const agentStateWords: Record<KindTask['state'], MessageKey> = {
  queued: 'agentTasks.states.queued',
  running: 'agentTasks.states.running',
  cancelling: 'agentTasks.states.cancelling',
  paused: 'agentTasks.states.paused',
  succeeded: 'agentTasks.states.succeeded',
  partial: 'agentTasks.states.partial',
  interrupted: 'agentTasks.states.interrupted',
  failed: 'agentTasks.states.failed',
}

/** Status colours; the words say the same, so colour is never the only cue. */
function stateTone(state: string) {
  if (state === 'succeeded') return 'bg-green/12 text-green'
  if (state === 'failed') return 'bg-destructive/10 text-destructive'
  if (state === 'partial' || state === 'stale')
    return 'bg-brand-soft text-brand'
  return 'bg-muted text-muted-foreground'
}

function Owner({ kind }: { kind: string }) {
  const { t } = useI18n()
  const space = spaceOfKind(kind as Parameters<typeof spaceOfKind>[0])
  if (!space)
    return (
      <span className="w-7 shrink-0 text-[11px] leading-tight text-muted-foreground">
        {t('tasks.owner.whole')}
      </span>
    )
  return (
    <span className="mt-0.5 shrink-0" title={t(space.name)}>
      <SpaceTile space={space} size="sm" />
      <span className="sr-only">{t(space.name)}</span>
    </span>
  )
}

/** The figures this window follows live for a browsing backup or import, by run type. */
export type { Controls }

export type LiveWork = Partial<Record<string, BackgroundTask>>

/** What a row shows, whichever kind its task is. */
interface RowView {
  id: string
  runRef: string | null
  kind: string
  name: string
  state: string
  stateText: string
  /** At work: a percentage (or none), a bar, words that change as it goes. */
  working: boolean
  /** Started, with nothing to count yet (this window's own backup or import). */
  preparing: boolean
  percent: number | null
  /** The bar stands still: paused, or waiting to go on. */
  still: boolean
  bar: boolean
  detail: (string | null)[]
  link: string | null
  actions: ReactNode
  notices: ReactNode
}

function useAgentView(task: KindTask, controls: Controls): RowView {
  const { t } = useI18n()
  const format = useFormat()
  const paused = task.state === 'paused'
  const working = isActive(task) || paused
  return {
    id: task.taskId,
    runRef: task.runRef,
    kind: task.kind,
    name: taskName(t, task),
    state: task.state,
    stateText: t(agentStateWords[task.state]),
    working,
    // No total is known while a conversation backup runs.
    preparing: false,
    percent: null,
    still: paused,
    bar: working,
    detail: working
      ? [
          taskProgressText(t, format.bytes, task),
          paused ? t('tasks.row.youPaused') : null,
        ]
      : [taskReasonText(t, task.reason)],
    link: spaceBackupPath('agent', `?run=${encodeURIComponent(task.runRef)}`),
    actions: <AgentActions task={task} controls={controls} />,
    notices: task.formatNotices?.map((notice) => (
      <NoticeLine
        key={`${notice.tool}-${notice.version}`}
        notice={notice}
        task={task}
        control={{
          ...controls.agent,
          busy: controls.agent.busy ?? controls.browsing.busy,
        }}
        className="text-xs text-foreground"
      />
    )),
  }
}

function useBrowsingView(
  task: BrowsingTask,
  controls: Controls,
  live: LiveWork,
): RowView {
  const { t } = useI18n()
  const format = useFormat()
  const name = browsingTaskName(t, task)
  const running = task.state === 'running'
  // This window's own backup or import knows more than the ledger's row.
  const window = running ? live[task.operation] : undefined
  const progress = task.progress
  const percent = window
    ? window.percent
    : progress?.total
      ? Math.min(100, (progress.done / progress.total) * 100)
      : null
  const detail = window
    ? window.detail
    : progress
      ? progress.total
        ? t('tasks.row.progress', {
            done: format.number(progress.done),
            total: format.number(progress.total),
          })
        : t('tasks.row.done', { count: progress.done })
      : null
  return {
    id: task.taskId,
    runRef: task.runRef,
    kind: task.kind,
    name,
    state: task.state,
    stateText: browsingStateText(t, task.state),
    working: !browsingEnded(task.state),
    preparing: window?.preparing ?? false,
    percent,
    still: !running,
    bar: running || task.state === 'paused',
    detail: [detail],
    // A run opens its details on the browsing backup page; a queue job has none.
    link:
      task.runRef !== null
        ? spaceBackupPath('browsing', `?run=${task.jobId}`)
        : null,
    actions: <BrowsingActions task={task} name={name} controls={controls} />,
    notices: null,
  }
}

function AgentTaskRow(props: {
  task: KindTask
  at: number
  group: TaskGroup
  controls: Controls
}) {
  return (
    <RowLayout
      view={useAgentView(props.task, props.controls)}
      at={props.at}
      group={props.group}
    />
  )
}

function BrowsingTaskRow(props: {
  task: BrowsingTask
  at: number
  group: TaskGroup
  controls: Controls
  live: LiveWork
}) {
  return (
    <RowLayout
      view={useBrowsingView(props.task, props.controls, props.live)}
      at={props.at}
      group={props.group}
    />
  )
}

function RowLayout({
  view,
  at,
  group,
}: {
  view: RowView
  at: number
  group: TaskGroup
}) {
  const { t } = useI18n()
  const format = useFormat()
  const running = view.state === 'running'
  const detail = view.detail.filter(Boolean).join(t('agentTasks.progress.join'))
  return (
    <li
      data-task-id={view.id}
      data-run-ref={view.runRef ?? undefined}
      data-state={view.state}
      data-kind={view.kind}
      className="flex animate-in items-start gap-3 border-t px-4 py-3 duration-200 fade-in-0 first:border-t-0"
    >
      <Owner kind={view.kind} />
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px]">
          {view.link ? (
            <Link to={view.link} className="font-medium hover:underline">
              {view.name}
            </Link>
          ) : (
            <span className="font-medium">{view.name}</span>
          )}
          {!running && (
            <Badge
              variant="secondary"
              className={cn('font-normal', stateTone(view.state))}
            >
              {view.stateText}
            </Badge>
          )}
          <span className="flex-1" />
          <span className="shrink-0 text-xs text-muted-foreground tabular">
            {running
              ? view.preparing
                ? t('spaces.tasks.preparing')
                : view.percent !== null &&
                  format.percent(Math.round(view.percent) / 100)
              : group === 'today'
                ? format.time(at)
                : format.dayAndTime(at)}
          </span>
        </div>
        {view.bar && (
          <Progress
            aria-label={view.name}
            value={view.percent}
            className={cn(
              'h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand',
              view.still &&
                view.percent === null &&
                '[&>[data-slot=progress-indicator]]:animate-none',
            )}
          />
        )}
        {detail && (
          <span
            aria-live={view.working ? 'polite' : undefined}
            className="text-xs text-muted-foreground tabular"
          >
            {detail}
          </span>
        )}
        {view.notices}
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
        {view.actions}
      </div>
    </li>
  )
}

export function TaskRow({
  entry,
  group,
  controls,
  live,
}: {
  entry: TaskEntry
  group: TaskGroup
  controls: Controls
  live: LiveWork
}) {
  return entry.agent ? (
    <AgentTaskRow
      task={entry.agent}
      at={entry.at}
      group={group}
      controls={controls}
    />
  ) : (
    <BrowsingTaskRow
      task={entry.browsing}
      at={entry.at}
      group={group}
      controls={controls}
      live={live}
    />
  )
}
