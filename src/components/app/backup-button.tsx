/**
 * Backup status pill and "Back up now" button, shared by Home and Backup.
 */
import { RefreshCw } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { useBackupRunner } from '@/app/backup-runner'
import { cn } from '@/lib/cn'
import { useNow } from '@/lib/hooks/use-now'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import {
  nextScheduledBackup,
  scheduleInstalled,
  useScheduleStatus,
} from '@/lib/queries/schedule'

export function BackupStatusPill() {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const schedule = useScheduleStatus()
  const backup = useBackupRunner()
  const now = useNow()
  const last = snapshot.archiveStatus.lastSuccessfulBackupAt
  const lastRun = snapshot.recentRuns[0]
  const failed = lastRun?.status === 'failed'
  const next = nextScheduledBackup(schedule.data, now)

  const dueHours = schedule.data?.dueAfterHours ?? 24
  const fresh = last && now - Date.parse(last) < dueHours * 2 * 3_600_000
  const dot = backup.running
    ? 'bg-brand animate-pulse'
    : failed
      ? 'bg-red'
      : fresh
        ? 'bg-green'
        : 'bg-brand'

  let text = last
    ? t('shell.backup.lastAgo', { ago: format.relative(last, now) })
    : t('shell.backup.neverRun')
  if (backup.running) {
    const progress = backup.progress
    text = progress?.totalProfiles
      ? t('shell.backup.runningDetail', {
          done: Math.min(
            progress.completedProfiles + 1,
            progress.totalProfiles,
          ),
          total: progress.totalProfiles,
        })
      : t('shell.backup.running')
  } else if (next) {
    text += ` · ${t('shell.backup.nextAt', { time: format.time(next) })}`
  } else if (schedule.data && !scheduleInstalled(schedule.data)) {
    text += ` · ${t('shell.backup.scheduleOff')}`
  }

  return (
    <Link
      to="/backup"
      className="flex h-9 items-center gap-2 rounded-lg border bg-card px-3 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
    >
      <span className={cn('size-[7px] rounded-full', dot)} />
      {text}
    </Link>
  )
}

export function BackupNowButton({ className }: { className?: string }) {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const percent = backup.progress?.progressPercent
  return (
    <Button
      onClick={() => void backup.run()}
      disabled={backup.running}
      className={cn('h-9', className)}
    >
      <RefreshCw className={cn(backup.running && 'animate-spin')} />
      {backup.running
        ? percent != null
          ? `${t('shell.backup.running')} ${Math.round(percent)}%`
          : t('shell.backup.running')
        : t('shell.backup.now')}
    </Button>
  )
}
