/**
 * The recent-jobs list inside a queue row: name, state, when, progress,
 * what went wrong, and the retry / cancel buttons the backend allows.
 *
 * Shows the first few jobs and expands to the rest (the backend returns at
 * most a dozen, so there is nothing to virtualize). Not responsible for
 * fetching or for what the buttons do.
 */
import { RotateCcw, X } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import type { JobState, JobView } from './job-view'

const COLLAPSED = 5

const stateStyle: Record<JobState | 'unknown', string> = {
  queued: 'bg-muted text-muted-foreground',
  running: 'bg-brand-soft text-brand',
  succeeded: 'bg-green/12 text-green',
  failed: 'bg-destructive/10 text-destructive',
  cancelled: 'bg-muted text-muted-foreground',
  paused: 'bg-muted text-muted-foreground',
  stale: 'bg-brand-soft text-brand',
  unknown: 'bg-muted text-muted-foreground',
}

const whenKey = {
  queued: 'settingsBackground.when.queued',
  paused: 'settingsBackground.when.queued',
  running: 'settingsBackground.when.running',
  succeeded: 'settingsBackground.when.succeeded',
  failed: 'settingsBackground.when.failed',
  cancelled: 'settingsBackground.when.cancelled',
  stale: 'settingsBackground.when.other',
  unknown: 'settingsBackground.when.other',
} as const

export function JobList({
  jobs,
  busy,
  retryLabel,
  onRetry,
  onCancel,
}: {
  jobs: JobView[]
  busy: boolean
  /** "Try again" for Insights jobs, "Run again" for search-index jobs. */
  retryLabel?: { text: string; aria: (name: string) => string }
  onRetry: (job: JobView) => void
  onCancel: (job: JobView) => void
}) {
  const { t } = useI18n()
  const [expanded, setExpanded] = useState(false)
  if (jobs.length === 0) {
    return (
      <p className="border-t pt-3 text-[13px] text-muted-foreground">
        {t('settingsBackground.empty')}
      </p>
    )
  }
  const shown = expanded ? jobs : jobs.slice(0, COLLAPSED)
  return (
    <div className="flex flex-col border-t pt-1">
      <ul aria-label={t('settingsBackground.recent')} className="flex flex-col">
        {shown.map((job) => (
          <JobRow
            key={job.key}
            job={job}
            busy={busy}
            retryLabel={retryLabel}
            onRetry={onRetry}
            onCancel={onCancel}
          />
        ))}
      </ul>
      {jobs.length > COLLAPSED && (
        <Button
          size="xs"
          variant="ghost"
          className="mt-1 self-start text-muted-foreground"
          onClick={() => setExpanded((value) => !value)}
        >
          {expanded
            ? t('settingsBackground.showFewer')
            : t('settingsBackground.showAll', { count: jobs.length })}
        </Button>
      )}
    </div>
  )
}

function JobRow({
  job,
  busy,
  retryLabel,
  onRetry,
  onCancel,
}: {
  job: JobView
  busy: boolean
  retryLabel?: { text: string; aria: (name: string) => string }
  onRetry: (job: JobView) => void
  onCancel: (job: JobView) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const stateLabel =
    job.state === 'unknown'
      ? job.rawState
      : t(`settingsBackground.state.${job.state}`)
  const progress = job.progress
  return (
    <li
      className="flex flex-col gap-1.5 border-b py-2.5 last:border-b-0"
      data-job-state={job.rawState}
    >
      <div className="flex items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-[13px] font-medium" title={job.name}>
            {job.name}
          </span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {job.at && t(whenKey[job.state], { when: format.relative(job.at) })}
            {progress?.current !== undefined &&
              progress.total !== undefined &&
              ` · ${t('settingsBackground.progress', {
                current: format.number(progress.current),
                total: format.number(progress.total),
              })}`}
          </span>
        </div>
        <Badge
          variant="secondary"
          className={cn(
            'min-w-[76px] justify-center rounded-md border-0 font-normal',
            stateStyle[job.state],
          )}
        >
          {stateLabel}
        </Badge>
        <div className="flex w-[60px] shrink-0 justify-end gap-1">
          {job.canRetry && (
            <Button
              size="icon-xs"
              variant="ghost"
              disabled={busy}
              title={retryLabel?.text ?? t('settingsBackground.retry')}
              aria-label={
                retryLabel?.aria(job.name) ??
                t('settingsBackground.retryLabel', { name: job.name })
              }
              onClick={() => onRetry(job)}
            >
              <RotateCcw />
            </Button>
          )}
          {job.canCancel && (
            <Button
              size="icon-xs"
              variant="ghost"
              disabled={busy}
              title={t('settingsBackground.cancel')}
              aria-label={t('settingsBackground.cancelLabel', {
                name: job.name,
              })}
              onClick={() => onCancel(job)}
            >
              <X />
            </Button>
          )}
        </div>
      </div>
      {progress && (
        <Progress
          value={progress.percent}
          aria-label={job.name}
          className="h-1.5"
        />
      )}
      {job.error && (
        <p className="text-xs break-words text-destructive">{job.error}</p>
      )}
    </li>
  )
}
