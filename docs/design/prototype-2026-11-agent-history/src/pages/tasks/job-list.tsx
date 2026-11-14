/**
 * The job list inside a queue row: name, state, when, progress, what went
 * wrong, and the pause / resume / retry / cancel buttons the backend allows.
 * The newest five; 顯示全部 unfolds the whole history in a scrolling box that
 * reads earlier pages as it nears the end. Mirrors
 * `src/features/settings/background/job-list.tsx` (the app virtualizes the
 * unfolded list; this one draws it plainly).
 */
import { Pause, Play, RotateCcw, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/cn'
import { number, relative } from '../schedule/format'
import type { JobState, JobView } from './jobs'

const COLLAPSED = 5
const PAGE = 12

const stateStyle: Record<JobState, string> = {
  queued: 'bg-muted text-muted-foreground',
  running: 'bg-brand-soft text-brand',
  succeeded: 'bg-green/12 text-green',
  failed: 'bg-destructive/10 text-destructive',
  cancelled: 'bg-muted text-muted-foreground',
  paused: 'bg-muted text-muted-foreground',
  stale: 'bg-brand-soft text-brand',
  unknown: 'bg-muted text-muted-foreground',
}
const stateLabels: Record<Exclude<JobState, 'unknown'>, string> = {
  queued: '等待中',
  running: '執行中',
  succeeded: '已完成',
  failed: '失敗',
  cancelled: '已取消',
  paused: '已暫停',
  stale: '已中斷',
}
/** `whenKey`: the time that matches the state. */
const whenText: Record<JobState, (when: string) => string> = {
  queued: (w) => `${w}加入`,
  paused: (w) => `${w}加入`,
  running: (w) => `${w}開始`,
  succeeded: (w) => `${w}完成`,
  failed: (w) => `${w}失敗`,
  cancelled: (w) => `${w}取消`,
  stale: (w) => `${w}更新`,
  unknown: (w) => `${w}更新`,
}

export interface RowActions {
  busy?: boolean
  /** 「重試」 for Insights jobs, 「再執行一次」 for search-index jobs. */
  retryLabel?: { text: string; aria: (name: string) => string }
  onRetry: (job: JobView) => void
  onCancel: (job: JobView) => void
  onPause?: (job: JobView) => void
}

export function JobList({ jobs, loading, ...actions }: RowActions & { jobs: JobView[]; loading: boolean }) {
  const [expanded, setExpanded] = useState(false)
  // The first page is what a collapsed list holds; the rest is read as the list scrolls.
  const [read, setRead] = useState(PAGE)
  if (loading) {
    return (
      <div className="flex flex-col gap-2 border-t pt-3" aria-hidden>
        <Skeleton className="h-4 w-2/3" />
        <Skeleton className="h-4 w-1/2" />
      </div>
    )
  }
  if (jobs.length === 0) return <p className="border-t pt-3 text-[13px] text-muted-foreground">最近沒有執行任何工作。</p>
  const shown = jobs.slice(0, read)
  const hasMore = read < jobs.length
  return (
    <div className="flex flex-col border-t pt-1">
      {expanded ? (
        <AllJobs jobs={shown} hasMore={hasMore} onLoadMore={() => setRead((n) => n + PAGE)} {...actions} />
      ) : (
        <ul aria-label="最近的工作" className="flex flex-col">
          {jobs.slice(0, COLLAPSED).map((job) => (
            <JobRow key={job.key} job={job} {...actions} />
          ))}
        </ul>
      )}
      {(jobs.length > COLLAPSED || hasMore) && (
        <Button
          size="xs"
          variant="ghost"
          className="mt-1 self-start text-muted-foreground"
          aria-expanded={expanded}
          onClick={() => {
            if (expanded) setRead(PAGE)
            setExpanded(!expanded)
          }}
        >
          {expanded ? '收起' : '顯示全部'}
        </Button>
      )}
    </div>
  )
}

function AllJobs({ jobs, hasMore, onLoadMore, ...actions }: RowActions & { jobs: JobView[]; hasMore: boolean; onLoadMore: () => void }) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [loadingMore, setLoadingMore] = useState(false)
  useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const onScroll = () => {
      if (!hasMore || loadingMore) return
      if (el.scrollTop + el.clientHeight >= el.scrollHeight - 120) {
        setLoadingMore(true)
        window.setTimeout(() => {
          setLoadingMore(false)
          onLoadMore()
        }, 700)
      }
    }
    el.addEventListener('scroll', onScroll)
    return () => el.removeEventListener('scroll', onScroll)
  }, [hasMore, loadingMore, onLoadMore])
  return (
    <div ref={scrollRef} className="max-h-[460px] overflow-y-auto">
      <ul aria-label="最近的工作" className="flex flex-col">
        {jobs.map((job) => (
          <JobRow key={job.key} job={job} {...actions} />
        ))}
      </ul>
      <div className="py-2 text-xs text-muted-foreground" aria-live="polite">
        {loadingMore ? (
          <span className="flex items-center gap-2">
            <Spinner />
            正在載入更早的工作…
          </span>
        ) : hasMore ? null : (
          '所有工作紀錄都在這裡了。'
        )}
      </div>
    </div>
  )
}

function JobRow({ job, busy, retryLabel, onRetry, onCancel, onPause }: RowActions & { job: JobView }) {
  const stateLabel = job.state === 'unknown' ? job.rawState : stateLabels[job.state]
  const progress = job.progress
  return (
    <li className="flex flex-col gap-1.5 border-b py-2.5 last:border-b-0" data-job={job.key} data-job-state={job.rawState}>
      <div className="flex items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-[13px] font-medium" title={job.name}>
            {job.name}
          </span>
          <span className="text-xs text-muted-foreground tabular-nums">
            {job.at && whenText[job.state](relative(job.at))}
            {progress?.current !== undefined && progress.total !== undefined && ` · ${number(progress.current)} / ${number(progress.total)}`}
          </span>
        </div>
        <Badge variant="secondary" className={cn('min-w-[76px] justify-center rounded-md border-0 font-normal', stateStyle[job.state])}>
          {stateLabel}
        </Badge>
        <div className="flex w-[60px] shrink-0 justify-end gap-1">
          {job.canPause && onPause && (
            <Button size="icon-xs" variant="ghost" disabled={busy} title="暫停" aria-label={`暫停：${job.name}`} onClick={() => onPause(job)}>
              <Pause />
            </Button>
          )}
          {job.canRetry &&
            (job.resumes ? (
              <Button size="icon-xs" variant="ghost" disabled={busy} title="繼續" aria-label={`繼續：${job.name}`} onClick={() => onRetry(job)}>
                <Play />
              </Button>
            ) : (
              <Button
                size="icon-xs"
                variant="ghost"
                disabled={busy}
                title={retryLabel?.text ?? '重試'}
                aria-label={retryLabel?.aria(job.name) ?? `重試：${job.name}`}
                onClick={() => onRetry(job)}
              >
                <RotateCcw />
              </Button>
            ))}
          {job.canCancel && (
            <Button size="icon-xs" variant="ghost" disabled={busy} title="取消" aria-label={`取消：${job.name}`} onClick={() => onCancel(job)}>
              <X />
            </Button>
          )}
        </div>
      </div>
      {progress && <Progress value={progress.percent} aria-label={job.name} className="h-1.5" />}
      {job.note && <p className={cn('text-xs break-words', job.note.problem ? 'text-destructive' : 'text-muted-foreground')}>{job.note.text}</p>}
      {job.error && <p className="text-xs break-words text-destructive">{job.error}</p>}
    </li>
  )
}
