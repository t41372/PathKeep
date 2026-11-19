/**
 * Conversation backups in Settings › Background work, while Agent
 * conversations are on or paused: what needs a look (Try again, Dismiss
 * for what was not saved; a format version PathKeep cannot read yet, each
 * with a Dismiss of its own that leaves the rest), what runs now (Cancel)
 * with its progress in real units, and what finished
 * today. Each group pages through the backend's cursor, so a year of hourly
 * backups never loads at once. A row opens its run on the Agent backup page.
 *
 * Responsibilities: the three groups and their buttons, and scrolling them
 * into view when the address asks (`?show=conversations`, the rail's
 * "needs a look"). A store that will not open says so here; browsing work
 * above and below is unaffected.
 * Not responsible for: what an action does (`use-task-control.ts`). When
 * the `#/tasks` page comes, it lists the same groups from the same queries.
 */
import { RotateCcw, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { SettingsGroup } from '@/components/app/setting-row'
import { CardError } from '@/components/app/section-card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  KindTask,
  TaskAction,
  TaskGroup,
} from '@/lib/backend-client/agent-backup'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import {
  isActive,
  useAgentKind,
  useKindTasks,
} from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'
import { NoticeLine } from '@/features/agent-backup/notice-line'
import {
  actionKey,
  useTaskControl,
} from '@/features/agent-backup/use-task-control'
import {
  taskName,
  taskProgressText,
  taskReasonText,
} from '@/features/agent-backup/words'

const stateWords: Record<KindTask['state'], MessageKey> = {
  queued: 'agentTasks.states.queued',
  running: 'agentTasks.states.running',
  cancelling: 'agentTasks.states.cancelling',
  succeeded: 'agentTasks.states.succeeded',
  partial: 'agentTasks.states.partial',
  interrupted: 'agentTasks.states.interrupted',
  failed: 'agentTasks.states.failed',
}

const stateStyle: Record<KindTask['state'], string> = {
  queued: 'bg-muted text-muted-foreground',
  running: 'bg-brand-soft text-brand',
  cancelling: 'bg-muted text-muted-foreground',
  succeeded: 'bg-green/12 text-green',
  partial: 'bg-brand-soft text-brand',
  interrupted: 'bg-muted text-muted-foreground',
  failed: 'bg-destructive/10 text-destructive',
}

function TaskRow({
  task,
  control,
}: {
  task: KindTask
  control: ReturnType<typeof useTaskControl>
}) {
  const { t } = useI18n()
  const format = useFormat()
  const name = taskName(t, task)
  const active = isActive(task)
  const reason = active ? null : taskReasonText(t, task.reason)
  const runLink = spaceBackupPath(
    'agent',
    `?run=${encodeURIComponent(task.runRef)}`,
  )
  const pressing = (action: TaskAction) =>
    control.busy === actionKey(task, action)
  return (
    <li
      data-task-id={task.taskId}
      data-run-ref={task.runRef}
      data-state={task.state}
      className="flex flex-col gap-1.5 border-t px-4 py-3 first:border-t-0"
    >
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px]">
        <Link to={runLink} className="font-medium hover:underline">
          {name}
        </Link>
        <Badge
          variant="secondary"
          className={cn('font-normal', stateStyle[task.state])}
        >
          {t(stateWords[task.state])}
        </Badge>
        <span className="text-xs text-muted-foreground tabular">
          {format.dayAndTime(task.endedAt ?? task.startedAt)}
        </span>
        <span className="flex-1" />
        {task.actions.includes('retry') && (
          <Button
            size="sm"
            variant="outline"
            className="h-7"
            aria-label={t('agentTasks.actions.retryLabel', { name })}
            disabled={control.busy !== null}
            onClick={() => void control.send(task, 'retry')}
          >
            <RotateCcw aria-hidden />
            {t('agentTasks.actions.retry')}
          </Button>
        )}
        {task.actions.includes('dismiss') && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7"
            aria-label={t('agentTasks.actions.dismissLabel', { name })}
            disabled={control.busy !== null}
            onClick={() => void control.send(task, 'dismiss')}
          >
            {t('agentTasks.actions.dismiss')}
          </Button>
        )}
        {task.actions.includes('cancel') && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7"
            aria-label={t('agentTasks.actions.cancelLabel', { name })}
            disabled={control.busy !== null || pressing('cancel')}
            onClick={() => void control.send(task, 'cancel')}
          >
            <X aria-hidden />
            {t('agentTasks.actions.cancel')}
          </Button>
        )}
      </div>
      {active && (
        <>
          <span
            aria-live="polite"
            className="text-xs text-muted-foreground tabular"
          >
            {taskProgressText(t, format.bytes, task)}
          </span>
          <Progress
            aria-label={name}
            className="h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand"
          />
        </>
      )}
      {reason && (
        <span className="text-xs text-muted-foreground">{reason}</span>
      )}
      {task.formatNotices?.map((notice) => (
        <NoticeLine
          key={`${notice.tool}-${notice.version}`}
          notice={notice}
          task={task}
          control={control}
          className="text-xs text-foreground"
        />
      ))}
    </li>
  )
}

function Group({
  group,
  title,
  empty,
  control,
}: {
  group: TaskGroup
  title: MessageKey
  /** Shown when the group is empty; a group without one hides itself. */
  empty?: MessageKey
  control: ReturnType<typeof useTaskControl>
}) {
  const { t } = useI18n()
  const query = useKindTasks(group)
  const items = query.data?.pages.flatMap((page) => page.items) ?? []
  if (query.isSuccess && items.length === 0 && !empty) return null
  return (
    <section
      aria-label={t(title)}
      data-task-group={group}
      className="overflow-hidden rounded-xl border bg-card shadow-card"
    >
      <h4 className="border-b bg-muted/30 px-4 py-2 text-xs font-semibold text-muted-foreground">
        {t(title)}
      </h4>
      {query.isPending ? (
        <div aria-hidden className="flex flex-col gap-2 px-4 py-3">
          <Skeleton className="h-4 w-64" />
        </div>
      ) : query.isError && !query.data ? (
        <div className="px-4 py-3">
          <CardError error={query.error} onRetry={() => void query.refetch()} />
        </div>
      ) : items.length === 0 ? (
        <p className="px-4 py-3 text-[13px] text-muted-foreground">
          {t(empty!)}
        </p>
      ) : (
        <ul className="max-h-[420px] overflow-y-auto">
          {items.map((task) => (
            <TaskRow key={task.taskId} task={task} control={control} />
          ))}
        </ul>
      )}
      {query.hasNextPage && (
        <div className="border-t px-4 py-1.5">
          <Button
            variant="ghost"
            size="sm"
            disabled={query.isFetchingNextPage}
            onClick={() => void query.fetchNextPage()}
          >
            {t('agentTasks.group.more')}
          </Button>
        </div>
      )}
    </section>
  )
}

export function ConversationTasks() {
  const { t } = useI18n()
  const { row, readable } = useAgentKind()
  const control = useTaskControl()
  const [params] = useSearchParams()
  const anchor = useRef<HTMLDivElement>(null)
  const shown = row !== undefined && row.state !== 'off'
  const asked = params.get('show') === 'conversations'
  useEffect(() => {
    if (!asked || !shown) return
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    anchor.current?.scrollIntoView({
      block: 'start',
      behavior: reduce ? 'auto' : 'smooth',
    })
  }, [asked, shown])
  if (!shown) return null
  return (
    <>
      <div ref={anchor} className="scroll-mt-6" />
      <SettingsGroup
        title={t('agentTasks.group.title')}
        note={t('agentTasks.group.note')}
      />
      {!readable ? (
        <p
          role="status"
          className="rounded-xl border bg-card px-4 py-3 text-[13px] text-muted-foreground"
        >
          {row.availability === 'unavailable'
            ? t('agentTasks.group.unavailable')
            : t('dataKinds.manage.notInitialized')}
        </p>
      ) : (
        <div className="flex flex-col gap-3">
          <Group
            group="attention"
            title="agentTasks.group.attention"
            control={control}
          />
          <Group
            group="running"
            title="agentTasks.group.running"
            empty="agentTasks.group.noneRunning"
            control={control}
          />
          <Group
            group="today"
            title="agentTasks.group.today"
            empty="agentTasks.group.noneToday"
            control={control}
          />
        </div>
      )}
    </>
  )
}
