/**
 * The job list inside a queue row: name, state, when, progress, what went
 * wrong, and the retry / cancel buttons the backend allows.
 *
 * Shows the newest few and unfolds to the queue's whole history, read on a
 * page at a time as it scrolls. Unfolded, rows are virtualized: a queue that
 * fetched page text for every visit holds a job per page. Rows differ in
 * height (a progress bar, an error), so each is measured.
 *
 * Not responsible for fetching or for what the buttons do.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { Pause, Play, RotateCcw, X } from 'lucide-react'
import { type CSSProperties, useEffect, useRef, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import type { JobState, JobView } from './job-view'

const COLLAPSED = 5
/** A row with a progress bar or an error is taller; this is the plain one. */
const ROW_ESTIMATE = 58
const LIST_HEIGHT = 'max-h-[460px]'

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

interface RowActions {
  busy: boolean
  /** "Try again" for Insights jobs, "Run again" for search-index jobs. */
  retryLabel?: { text: string; aria: (name: string) => string }
  onRetry: (job: JobView) => void
  onCancel: (job: JobView) => void
  /** Index builds only: stops where it is, to go on later. */
  onPause?: (job: JobView) => void
}

export function JobList({
  jobs,
  loading,
  hasMore,
  loadingMore,
  loadError,
  command,
  onLoadMore,
  onFold,
  ...actions
}: RowActions & {
  jobs: JobView[]
  /** The first page is still on its way. */
  loading: boolean
  hasMore: boolean
  loadingMore: boolean
  /** Reading an earlier page failed. */
  loadError: unknown
  /** The backend command behind the list, for the error text. */
  command: string
  onLoadMore: () => void
  /** Folded back: only the first page needs to stay current. */
  onFold: () => void
}) {
  const { t } = useI18n()
  const [expanded, setExpanded] = useState(false)
  if (loading) {
    return (
      <div className="flex flex-col gap-2 border-t pt-3" aria-hidden>
        <Skeleton className="h-4 w-2/3" />
        <Skeleton className="h-4 w-1/2" />
      </div>
    )
  }
  if (jobs.length === 0) {
    return (
      <p className="border-t pt-3 text-[13px] text-muted-foreground">
        {loadError
          ? t('settingsBackground.loadFailed', {
              message: describeError(loadError, command),
            })
          : t('settingsBackground.empty')}
      </p>
    )
  }
  const foldable = jobs.length > COLLAPSED || hasMore
  return (
    <div className="flex flex-col border-t pt-1">
      {expanded ? (
        <AllJobs
          jobs={jobs}
          hasMore={hasMore}
          loadingMore={loadingMore}
          loadError={loadError}
          command={command}
          onLoadMore={onLoadMore}
          {...actions}
        />
      ) : (
        <ul
          aria-label={t('settingsBackground.recent')}
          className="flex flex-col"
        >
          {jobs.slice(0, COLLAPSED).map((job) => (
            <JobRow key={job.key} job={job} {...actions} />
          ))}
        </ul>
      )}
      {foldable && (
        <Button
          size="xs"
          variant="ghost"
          className="mt-1 self-start text-muted-foreground"
          aria-expanded={expanded}
          onClick={() => {
            if (expanded) onFold()
            setExpanded(!expanded)
          }}
        >
          {expanded
            ? t('settingsBackground.showFewer')
            : t('settingsBackground.showAll')}
        </Button>
      )}
    </div>
  )
}

/** Every job read so far, windowed, reading on near the end. */
function AllJobs({
  jobs,
  hasMore,
  loadingMore,
  loadError,
  command,
  onLoadMore,
  ...actions
}: RowActions & {
  jobs: JobView[]
  hasMore: boolean
  loadingMore: boolean
  loadError: unknown
  command: string
  onLoadMore: () => void
}) {
  const { t } = useI18n()
  const scrollRef = useRef<HTMLDivElement>(null)
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: jobs.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_ESTIMATE,
    getItemKey: (index) => jobs[index].key,
    overscan: 8,
  })
  const items = virtualizer.getVirtualItems()
  const lastIndex = items.at(-1)?.index ?? -1
  useEffect(() => {
    if (hasMore && !loadingMore && !loadError && lastIndex >= jobs.length - 8)
      onLoadMore()
  }, [hasMore, loadingMore, loadError, lastIndex, jobs.length, onLoadMore])

  return (
    <div ref={scrollRef} className={cn('overflow-y-auto', LIST_HEIGHT)}>
      <ul
        aria-label={t('settingsBackground.recent')}
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {items.map((item) => (
          <JobRow
            key={item.key}
            job={jobs[item.index]}
            measure={virtualizer.measureElement}
            index={item.index}
            className="absolute top-0 left-0 w-full"
            style={{ transform: `translateY(${item.start}px)` }}
            {...actions}
          />
        ))}
      </ul>
      <div className="py-2 text-xs text-muted-foreground" aria-live="polite">
        {loadError ? (
          <span className="flex items-center gap-2 text-destructive">
            {t('settingsBackground.moreFailed', {
              message: describeError(loadError, command),
            })}
            <Button size="xs" variant="outline" onClick={onLoadMore}>
              {t('common.retry')}
            </Button>
          </span>
        ) : loadingMore ? (
          <span className="flex items-center gap-2">
            <Spinner />
            {t('settingsBackground.loadingMore')}
          </span>
        ) : hasMore ? null : (
          t('settingsBackground.allShown')
        )}
      </div>
    </div>
  )
}

function JobRow({
  job,
  busy,
  retryLabel,
  onRetry,
  onCancel,
  onPause,
  measure,
  index,
  className,
  style,
}: RowActions & {
  job: JobView
  /** Set when the row is windowed: it reports its height. */
  measure?: (element: HTMLElement | null) => void
  index?: number
  className?: string
  style?: CSSProperties
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
      ref={measure}
      data-index={index}
      className={cn(
        'flex flex-col gap-1.5 border-b py-2.5 last:border-b-0',
        className,
      )}
      style={style}
      data-job={job.key}
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
          {job.canPause && onPause && (
            <Button
              size="icon-xs"
              variant="ghost"
              disabled={busy}
              title={t('settingsBackground.ai.pause')}
              aria-label={t('settingsBackground.ai.pauseLabel', {
                name: job.name,
              })}
              onClick={() => onPause(job)}
            >
              <Pause />
            </Button>
          )}
          {job.canRetry &&
            (job.resumes ? (
              <Button
                size="icon-xs"
                variant="ghost"
                disabled={busy}
                title={t('settingsBackground.ai.resume')}
                aria-label={t('settingsBackground.ai.resumeLabel', {
                  name: job.name,
                })}
                onClick={() => onRetry(job)}
              >
                <Play />
              </Button>
            ) : (
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
            ))}
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
      {job.note && (
        <p
          className={cn(
            'text-xs break-words',
            job.note.problem ? 'text-destructive' : 'text-muted-foreground',
          )}
        >
          {job.note.text}
        </p>
      )}
      {job.error && (
        <p className="text-xs break-words text-destructive">{job.error}</p>
      )}
    </li>
  )
}
