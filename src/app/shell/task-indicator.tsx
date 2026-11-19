/**
 * The background-task indicator at the top of the rail's shared part
 * (`docs/features/data-spaces.md` §5.1). It stays in the same place in
 * every state, so the rail never shifts:
 * - nothing running: a quiet icon;
 * - running, every total known: a ring filled to their average;
 * - running, some total unknown: a turning ring (still under reduced motion);
 * - two or more running: their number beside it, in the plain text colour;
 * - something needs a look: a small dot.
 *
 * Hovering or focusing it opens a small list of what runs after a short
 * pause, and the list stays open while the pointer or the focus is inside
 * it; a click goes to where background work is listed. From the keyboard,
 * ↓ or → moves into the open list and Escape comes back.
 *
 * Responsibilities: drawing the indicator and its list, their two
 * buttons, and Cancel on a conversation backup. Not responsible for: what
 * counts as a task (`use-background-tasks.ts`).
 */
import {
  ChevronRight,
  CircleAlert,
  ListChecks,
  RefreshCw,
  X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { openRun } from '@/features/backup/run-link'
import { useBackUpEverything } from '@/features/agent-backup/use-back-up-everything'
import { useTaskControl } from '@/features/agent-backup/use-task-control'
import { Button } from '@/components/ui/button'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import { Progress } from '@/components/ui/progress'
import { cn } from '@/lib/cn'
import { useNow } from '@/lib/hooks/use-now'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  nextScheduledBackup,
  scheduleInstalled,
  useScheduleStatus,
} from '@/lib/queries/schedule'
import {
  useBackgroundTasks,
  type BackgroundTask,
  type BackgroundTasks,
} from './use-background-tasks'

/** Where background work is listed until it has a page of its own. */
const TASKS_PATH = '/settings/background'
/** The same, scrolled to conversation backups (`settings/background/conversation-tasks.tsx`). */
const CONVERSATION_TASKS_PATH = '/settings/background?show=conversations'
const OPEN_DELAY_MS = 180
const CLOSE_DELAY_MS = 220

function Ring({ value }: { value: number | null }) {
  const r = 8
  const c = 2 * Math.PI * r
  return (
    <svg
      viewBox="0 0 20 20"
      aria-hidden
      className={cn(
        'size-5',
        value === null &&
          'motion-safe:animate-spin motion-safe:[animation-duration:1.1s]',
      )}
    >
      <circle
        cx="10"
        cy="10"
        r={r}
        fill="none"
        strokeWidth="2.2"
        className="stroke-foreground/12"
      />
      <circle
        cx="10"
        cy="10"
        r={r}
        fill="none"
        strokeWidth="2.2"
        strokeLinecap="round"
        className="stroke-brand transition-[stroke-dashoffset] duration-500"
        strokeDasharray={c}
        strokeDashoffset={value === null ? c * 0.72 : c * (1 - value / 100)}
        transform="rotate(-90 10 10)"
      />
    </svg>
  )
}

/** Cancel on a conversation backup, while the backend offers it. */
function CancelTask({
  task,
}: {
  task: NonNullable<BackgroundTask['kindTask']>
}) {
  const { t } = useI18n()
  const control = useTaskControl()
  if (!task.actions.includes('cancel')) return null
  const name = t(
    task.trigger === 'enable'
      ? 'agentTasks.names.firstSave'
      : 'agentTasks.names.backup',
  )
  return (
    <Button
      variant="ghost"
      size="icon"
      className="-my-1 size-6 shrink-0 text-muted-foreground"
      aria-label={t('agentTasks.actions.cancelLabel', { name })}
      disabled={control.busy !== null}
      onClick={() => void control.send(task, 'cancel')}
    >
      <X aria-hidden className="size-3.5" />
    </Button>
  )
}

function TaskRow({ task }: { task: BackgroundTask }) {
  const { t } = useI18n()
  const status = task.preparing
    ? t('spaces.tasks.preparing')
    : task.percent !== null
      ? `${Math.round(task.percent)}%`
      : null
  return (
    <li
      className="flex flex-col gap-1.5 px-3 py-2"
      data-task={task.kindTask ? 'agent-history' : task.id}
    >
      <div className="flex items-baseline justify-between gap-3 text-[13px]">
        <span className="font-medium">{task.name}</span>
        {status && (
          <span className="shrink-0 text-[12px] text-muted-foreground tabular">
            {status}
          </span>
        )}
        {task.kindTask && <CancelTask task={task.kindTask} />}
      </div>
      <Progress
        aria-label={task.name}
        value={task.percent}
        className="h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand"
      />
      {task.detail && (
        <span className="truncate text-[12px] text-muted-foreground">
          {task.detail}
        </span>
      )}
    </li>
  )
}

/** "Nothing is running", with the next scheduled backup when there is one. */
function Idle() {
  const { t } = useI18n()
  const format = useFormat()
  const schedule = useScheduleStatus()
  const now = useNow()
  const next = nextScheduledBackup(schedule.data, now)
  const text = next
    ? t('spaces.tasks.nextBackup', { time: format.dayAndTime(next) })
    : schedule.data && !scheduleInstalled(schedule.data)
      ? t('spaces.tasks.scheduleOff')
      : t('spaces.tasks.nothingRunning')
  return <p className="px-3 py-3 text-[13px] text-muted-foreground">{text}</p>
}

function TaskList({
  tasks,
  onClose,
}: {
  tasks: BackgroundTasks
  onClose: () => void
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const everything = useBackUpEverything()
  const { running, queued, failedRun, kindAttention } = tasks
  return (
    <>
      <div className="border-b px-3 py-2 text-[12px] font-semibold text-muted-foreground">
        {t('spaces.tasks.title')}
      </div>
      {running.length === 0 ? (
        <Idle />
      ) : (
        <ul className="flex flex-col divide-y">
          {running.map((task) => (
            <TaskRow key={task.id} task={task} />
          ))}
        </ul>
      )}
      {(queued > 0 || failedRun !== null || kindAttention > 0) && (
        <div className="flex flex-col items-start gap-1 border-t px-3 py-2 text-[12px] text-muted-foreground">
          {queued > 0 && (
            <span>{t('spaces.tasks.queued', { count: queued })}</span>
          )}
          {failedRun !== null && (
            <button
              type="button"
              onClick={() => {
                onClose()
                openRun(failedRun)
              }}
              className="flex items-center gap-1 text-red hover:underline"
            >
              <CircleAlert className="size-3.5" aria-hidden />
              {t('spaces.tasks.needsLook', { count: 1 })}
            </button>
          )}
          {kindAttention > 0 && (
            <button
              type="button"
              onClick={() => {
                onClose()
                void navigate(CONVERSATION_TASKS_PATH)
              }}
              className="flex items-center gap-1 text-red hover:underline"
            >
              <CircleAlert className="size-3.5" aria-hidden />
              {t('agentTasks.rail.needsLook', { count: kindAttention })}
            </button>
          )}
        </div>
      )}
      <button
        type="button"
        disabled={everything.browsingRunning}
        onClick={() => {
          onClose()
          everything.run()
        }}
        className="flex w-full items-center gap-2 border-t px-3 py-2 text-left text-[13px] hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
      >
        <RefreshCw className="size-4 text-muted-foreground" aria-hidden />
        {t('spaces.tasks.backupAll')}
      </button>
      <button
        type="button"
        onClick={() => {
          onClose()
          void navigate(TASKS_PATH)
        }}
        className="flex w-full items-center justify-between rounded-b-md border-t px-3 py-2 text-left text-[13px] hover:bg-muted"
      >
        {t('spaces.tasks.open')}
        <ChevronRight className="size-4 text-muted-foreground" aria-hidden />
      </button>
    </>
  )
}

export function TaskIndicator() {
  const { t } = useI18n()
  const navigate = useNavigate()
  const tasks = useBackgroundTasks()
  const [open, setOpen] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const trigger = useRef<HTMLButtonElement>(null)
  const list = useRef<HTMLDivElement>(null)
  /** Set while the focus is back on the indicator after Escape closed the list. */
  const dismissed = useRef(false)
  useEffect(() => () => clearTimeout(timer.current), [])

  const later = (next: boolean) => {
    clearTimeout(timer.current)
    timer.current = setTimeout(
      () => setOpen(next),
      next ? OPEN_DELAY_MS : CLOSE_DELAY_MS,
    )
  }
  const close = () => {
    clearTimeout(timer.current)
    setOpen(false)
  }

  const { running, failedRun, kindAttention } = tasks
  const known = running.every((task) => task.percent !== null)
  const overall =
    running.length > 0 && known
      ? running.reduce((sum, task) => sum + (task.percent ?? 0), 0) /
        running.length
      : null
  const attention = (failedRun !== null ? 1 : 0) + kindAttention
  const label = running.length
    ? t('spaces.tasks.running', { count: running.length })
    : attention
      ? t('spaces.tasks.attention', { count: attention })
      : t('spaces.tasks.idle')

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <button
          ref={trigger}
          type="button"
          aria-label={label}
          aria-haspopup="dialog"
          aria-expanded={open}
          onKeyDown={(event) => {
            // The list is drawn outside the rail, so Tab would pass it by.
            if (event.key !== 'ArrowDown' && event.key !== 'ArrowRight') return
            event.preventDefault()
            clearTimeout(timer.current)
            setOpen(true)
            // After the list has rendered, when it was not open yet.
            requestAnimationFrame(() =>
              list.current?.querySelector('button')?.focus(),
            )
          }}
          onClick={() => {
            close()
            void navigate(TASKS_PATH)
          }}
          onPointerEnter={() => later(true)}
          onPointerLeave={() => later(false)}
          onFocus={() => {
            if (!dismissed.current) later(true)
          }}
          onBlur={() => {
            dismissed.current = false
            later(false)
          }}
          className="relative flex h-9 w-10 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          {running.length > 0 ? (
            <Ring value={overall} />
          ) : (
            <ListChecks className="size-[17px]" aria-hidden />
          )}
          {running.length > 1 && (
            <span
              aria-hidden
              className="absolute -top-0.5 right-0.5 flex size-4 items-center justify-center rounded-full bg-foreground text-[10px] font-semibold text-background tabular"
            >
              {running.length}
            </span>
          )}
          {attention > 0 && (
            <span
              aria-hidden
              className="absolute bottom-1 left-1.5 size-2 rounded-full border border-background bg-red"
            />
          )}
        </button>
      </PopoverAnchor>
      <PopoverContent
        ref={list}
        side="right"
        align="end"
        sideOffset={8}
        className="w-[300px] p-0"
        onPointerEnter={() => later(true)}
        onPointerLeave={() => later(false)}
        onFocus={() => later(true)}
        onBlur={() => later(false)}
        // The pointer brings it up; the focus stays where it was.
        onOpenAutoFocus={(event) => event.preventDefault()}
        // Escape hands the focus back to the indicator without opening the list again.
        onEscapeKeyDown={() => {
          if (!list.current?.contains(document.activeElement)) return
          dismissed.current = true
          trigger.current?.focus()
        }}
      >
        <TaskList tasks={tasks} onClose={close} />
      </PopoverContent>
    </Popover>
  )
}
