/**
 * The "Automatic backup" row on a space's backup page (`data-spaces.md` §4):
 * what the one schedule does for this kind of data, and a button to Settings
 * › Schedule and background work, where the card with its choices, repairs
 * and details lives. It reads the same schedule status as the card, so the
 * two never disagree:
 * - on and healthy: the interval and when this kind backs up next (Agent
 *   conversations keep their own clock, from their last finished backup;
 *   without a next time, as while they are paused, only the interval);
 * - off: says so;
 * - a scheduler problem (repair, permission, an old job) or a system where
 *   it is set up by hand: the card's sentence, as a warning;
 * - still checking: a skeleton; a check that failed: says so, with Try again.
 *
 * Not responsible for: changing the schedule (`schedule-card.tsx`).
 */
import { TriangleAlert } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useNow } from '@/lib/hooks/use-now'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { needsSchedulerReview } from '@/lib/platform-guidance'
import {
  nextScheduledBackup,
  scheduleInstalled,
  useScheduleStatus,
} from '@/lib/queries/schedule'
import type { DataKindId } from '@/lib/backend-client/data-kinds'
import { formatInterval } from './interval'
import { frequencyForHours } from './schedule-actions'

const SETTINGS = '/settings/background'

const problems: Record<string, MessageKey> = {
  mismatch: 'backup.schedule.attention.mismatch',
  'permission-warning': 'backup.schedule.attention.permission',
  'legacy-install-detected': 'backupSchedule.legacy.attention',
}

export function ScheduleRow({ kind }: { kind: DataKindId }) {
  const { t } = useI18n()
  const format = useFormat()
  const query = useScheduleStatus()
  const status = query.data
  const now = useNow()

  let text: string
  let action: string | null
  let warn = false
  if (!status) {
    text = query.isError
      ? t('tasks.scheduleRow.failed')
      : t('tasks.scheduleRow.checking')
    action = null
  } else if (!status.applySupported) {
    // As the card says: no scheduler PathKeep can install here.
    text = t('backup.schedule.manual.title')
    action = t('tasks.scheduleRow.steps')
    warn = true
  } else if (needsSchedulerReview(status)) {
    text = t(problems[status.installState] ?? 'backup.schedule.attention.title')
    action = t('tasks.scheduleRow.fix')
    warn = true
  } else if (scheduleInstalled(status)) {
    const preset = frequencyForHours(status.dueAfterHours)
    const interval = preset
      ? t(`backup.schedule.${preset}`)
      : t('tasks.scheduleRow.every', {
          interval: formatInterval(status.dueAfterHours, t),
        })
    const next = nextScheduledBackup(status, now, kind)
    text = next
      ? t('tasks.scheduleRow.next', {
          interval,
          time: format.dayAndTime(next),
        })
      : interval
    action = t('tasks.scheduleRow.change')
  } else {
    text = t('tasks.scheduleRow.off')
    action = t('tasks.scheduleRow.set')
  }

  return (
    <div
      data-schedule-row={kind}
      className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border bg-card px-5 py-3 shadow-card"
    >
      <span className="text-sm font-semibold">
        {t('tasks.scheduleRow.title')}
      </span>
      <span className="flex min-w-0 items-center gap-1.5 text-[13px] text-muted-foreground tabular">
        {warn && (
          <TriangleAlert aria-hidden className="size-3.5 shrink-0 text-brand" />
        )}
        {status || query.isError ? (
          <span role="status">{text}</span>
        ) : (
          <>
            <span className="sr-only">{text}</span>
            <Skeleton aria-hidden className="h-3.5 w-48" />
          </>
        )}
      </span>
      <span className="ml-auto flex items-center gap-2">
        {query.isError && !status && (
          <Button
            variant="outline"
            size="sm"
            onClick={() => void query.refetch()}
          >
            {t('common.retry')}
          </Button>
        )}
        {action && (
          <Button variant="outline" size="sm" asChild>
            <Link to={SETTINGS}>{action}</Link>
          </Button>
        )}
      </span>
    </div>
  )
}
