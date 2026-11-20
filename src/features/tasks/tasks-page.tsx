/**
 * Background tasks (`#/tasks`, `data-spaces.md` §5.2): what runs now, what
 * waits, what needs a look, and what finished today, for every kind of
 * data at once; then browsing history's queues and the data they build,
 * moved here from Settings (state, not settings).
 *
 * Responsibilities: the header (how many run, and whether closing the
 * window stops them: only while the menu bar icon is off does closing the
 * window quit PathKeep; the way to the schedule; "Back up everything
 * now"), the four groups in the spec's order, and one button press at a
 * time on the whole page. Every row stays current from the backend's
 * events (the ledger's, conversation backups', the semantic build's), kept
 * by the frame; nothing here polls.
 * Not responsible for: a row (`task-row.tsx`), a group's rows
 * (`use-task-group.ts`), or the queues' rows
 * (`features/settings/background/`).
 */
import { useQuery } from '@tanstack/react-query'
import {
  CircleAlert,
  CircleCheck,
  Clock,
  HardDriveDownload,
} from 'lucide-react'
import { Link } from 'react-router-dom'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { desktopClient } from '@/lib/backend-client/desktop'
import { useNow } from '@/lib/hooks/use-now'
import { useFormat, useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useSemanticIndexControl } from '@/lib/queries/agent-semantic'
import {
  nextScheduledBackupOfAny,
  scheduleInstalled,
  useScheduleStatus,
} from '@/lib/queries/schedule'
import { useLiveWork } from '@/app/shell/use-background-tasks'
import { useBackUpEverything } from '@/features/agent-backup/use-back-up-everything'
import { useTaskControl } from '@/features/agent-backup/use-task-control'
import { QueuesSection } from './queues-section'
import { needsLook } from './task-entry'
import { TaskGroupCard } from './task-group'
import type { Controls } from './task-row'
import { useBrowsingControl } from './use-browsing-control'
import { useTaskGroup } from './use-task-group'

/** Shared with Settings › General, which changes it. */
const desktopKey = ['desktop-integration'] as const

function Subtitle({ running, loading }: { running: number; loading: boolean }) {
  const { t } = useI18n()
  const desktop = useQuery({ queryKey: desktopKey, queryFn: desktopClient.get })
  if (loading)
    return (
      <>
        <span className="sr-only">{t('tasks.groups.loading')}</span>
        <Skeleton aria-hidden className="mt-1 h-3.5 w-64" />
      </>
    )
  if (running === 0) return t('tasks.subtitle.idle')
  // With the menu bar icon on, closing the window only hides it.
  const keepsGoing =
    desktop.data?.menuBarIconSupported === true && desktop.data.menuBarIcon
  return t(
    keepsGoing ? 'tasks.subtitle.keepsGoing' : 'tasks.subtitle.running',
    {
      count: running,
    },
  )
}

/** "None", with when the next scheduled backup runs, or that none is set. */
function NoneRunning() {
  const { t } = useI18n()
  const format = useFormat()
  const schedule = useScheduleStatus()
  const now = useNow()
  const next = nextScheduledBackupOfAny(schedule.data, now)
  if (next)
    return t('tasks.groups.noneRunningNext', { time: format.dayAndTime(next) })
  if (schedule.data && !scheduleInstalled(schedule.data))
    return t('tasks.groups.noneRunningOff')
  return t('tasks.groups.noneRunning')
}

export default function TasksPage() {
  const { t } = useI18n()
  const agentKind = useAgentKind()
  const everything = useBackUpEverything()
  const agentControl = useTaskControl()
  const browsingControl = useBrowsingControl()
  const semanticControl = useSemanticIndexControl()
  const controls: Controls = {
    agent: agentControl,
    browsing: browsingControl,
    semantic: semanticControl,
    busy:
      agentControl.busy !== null ||
      browsingControl.busy !== null ||
      semanticControl.isPending,
  }

  const live = useLiveWork()
  const running = useTaskGroup('running')
  const attention = useTaskGroup('attention')
  const waiting = useTaskGroup('waiting')
  const today = useTaskGroup('today')

  const unavailable =
    agentKind.row !== undefined &&
    agentKind.row.state !== 'off' &&
    agentKind.row.availability === 'unavailable'

  return (
    <PageScroll>
      <PageHeader
        title={t('tasks.title')}
        subtitle={
          <Subtitle
            running={running.entries.length}
            loading={running.loading}
          />
        }
        actions={
          <>
            <Button variant="outline" asChild>
              <Link to="/settings/background">{t('tasks.schedule')}</Link>
            </Button>
            <Button
              disabled={everything.browsingRunning}
              onClick={() => everything.run()}
            >
              <HardDriveDownload />
              {t('spaces.tasks.backupAll')}
            </Button>
          </>
        }
      />
      {unavailable && (
        <p
          role="status"
          className="rounded-xl border bg-card px-4 py-3 text-[13px] text-muted-foreground"
        >
          {t('tasks.groups.agentUnavailable')}
        </p>
      )}
      <TaskGroupCard
        group="attention"
        title={t('tasks.groups.attention')}
        icon={<CircleAlert aria-hidden className="size-4 text-brand" />}
        rows={attention}
        controls={controls}
        live={live}
      />
      <TaskGroupCard
        group="running"
        title={t('tasks.groups.running')}
        rows={running}
        empty={<NoneRunning />}
        controls={controls}
        live={live}
      />
      <TaskGroupCard
        group="waiting"
        title={t('tasks.groups.waiting')}
        icon={<Clock aria-hidden className="size-4 text-muted-foreground" />}
        rows={waiting}
        controls={controls}
        live={live}
      />
      <TaskGroupCard
        group="today"
        title={t('tasks.groups.today')}
        icon={<CircleCheck aria-hidden className="size-4 text-green" />}
        rows={today}
        // What needs a look is listed above, once.
        entries={today.entries.filter((entry) => !needsLook(entry))}
        empty={t('tasks.groups.noneToday')}
        controls={controls}
        live={live}
      />
      <QueuesSection />
    </PageScroll>
  )
}
