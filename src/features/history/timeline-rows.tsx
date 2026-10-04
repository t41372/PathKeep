/** The row types the History lists draw: day and session headers, visits and search results. */
import { GitCommitHorizontal, Star } from 'lucide-react'
import { memo, useMemo } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import { BrowserIcon } from '@/lib/browser-icons'
import { cn } from '@/lib/cn'
import { startOfDay, useFormat, useI18n } from '@/lib/i18n'
import type { VisitItem } from './history-types'
import { SiteIcon } from './site-icon'
import type { TimelineRow } from './timeline-model'

export function DayHeader({
  row,
}: {
  row: Extract<TimelineRow, { kind: 'day' }>
}) {
  const { t } = useI18n()
  const format = useFormat()
  const label = useMemo(() => {
    const day = new Date(row.dayStart)
    const days = Math.round(
      (startOfDay(new Date()).getTime() - row.dayStart) / 86_400_000,
    )
    const date = format.date(day, { month: 'long', day: 'numeric' })
    if (days === 0) return t('history.day.today', { date })
    if (days === 1) return t('history.day.yesterday', { date })
    return format.date(day, {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
      year:
        day.getFullYear() === new Date().getFullYear() ? undefined : 'numeric',
    })
  }, [row.dayStart, format, t])

  return (
    <div className="flex h-full items-end gap-2.5 px-2.5 pb-2">
      <h3 className="text-[15px] font-semibold">{label}</h3>
      {row.count !== null && (
        <span className="text-xs text-muted-foreground">
          {t('common.visits', { count: row.count })}
        </span>
      )}
    </div>
  )
}

export function SessionHeader({
  row,
}: {
  row: Extract<TimelineRow, { kind: 'session' }>
}) {
  const { t } = useI18n()
  const format = useFormat()
  const start = format.time(row.start)
  const end = format.time(row.end)
  const range = start === end ? start : `${start}–${end}`
  const label =
    row.count === null
      ? t('history.session.labelOpen', { range, site: row.site })
      : t('history.session.label', {
          range,
          pages: t('common.pages', { count: row.count }),
          site: row.site,
        })
  return (
    <div className="flex h-full items-end gap-2 px-2.5 pb-1.5 text-xs text-muted-foreground">
      <GitCommitHorizontal className="size-3.5 shrink-0" aria-hidden />
      <span className="truncate">{label}</span>
    </div>
  )
}

interface VisitRowProps {
  item: VisitItem
  selected: boolean
  starred: boolean
  browserName: string
  onSelect: (item: VisitItem) => void
  onOpen: (item: VisitItem) => void
}

function StarMark({ starred }: { starred: boolean }) {
  return starred ? (
    <Star className="size-[13px] shrink-0 fill-brand text-brand" aria-hidden />
  ) : null
}

const rowClass = (selected: boolean) =>
  cn(
    'flex h-full w-full items-center gap-3 rounded-lg px-2.5 text-left outline-none transition-colors',
    'focus-visible:ring-2 focus-visible:ring-ring/50',
    selected ? 'bg-brand-soft' : 'hover:bg-muted',
  )

export const VisitRow = memo(function VisitRow({
  item,
  selected,
  starred,
  browserName,
  onSelect,
  onOpen,
}: VisitRowProps) {
  const format = useFormat()
  const lookup = useMemo(
    () => ({
      profileId: item.profileId,
      url: item.url,
      visitTime: item.visitTime,
    }),
    [item],
  )
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      className={rowClass(selected)}
      onClick={() => onSelect(item)}
      onDoubleClick={() => onOpen(item)}
    >
      <span className="w-10 shrink-0 font-mono text-xs text-muted-foreground tabular">
        {format.time(item.visitTime)}
      </span>
      <SiteIcon
        domain={item.domain}
        lookup={lookup}
        className="size-[22px] text-[11px]"
      />
      <span className="min-w-0 flex-1 truncate">
        {item.title?.trim() || item.url}
      </span>
      <StarMark starred={starred} />
      <span className="max-w-[40%] shrink-0 truncate text-xs text-muted-foreground">
        {item.domain}
      </span>
      <BrowserIcon
        browserName={browserName}
        className="size-3.5 shrink-0 opacity-85"
        decorative
      />
    </button>
  )
})

/**
 * A search result: one page, with its most recent matching visit's date and,
 * for full-text and regex, how many visits matched.
 */
export const ResultRow = memo(function ResultRow({
  item,
  selected,
  starred,
  browserName,
  onSelect,
  onOpen,
}: VisitRowProps) {
  const { t } = useI18n()
  const format = useFormat()
  const lookup = useMemo(
    () => ({
      profileId: item.profileId,
      url: item.url,
      visitTime: item.visitTime,
    }),
    [item],
  )
  const relevance =
    item.score !== undefined && item.score >= 0 && item.score <= 1
      ? t('history.results.relevance', { percent: format.percent(item.score) })
      : null
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      className={rowClass(selected)}
      onClick={() => onSelect(item)}
      onDoubleClick={() => onOpen(item)}
    >
      <SiteIcon
        domain={item.domain}
        lookup={lookup}
        className="size-[22px] text-[11px]"
      />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate">{item.title?.trim() || item.url}</span>
        <span className="truncate text-xs text-muted-foreground">
          {item.domain} · {format.dayAndTime(item.visitTime)}
        </span>
      </span>
      {item.visitCount !== undefined && (
        <span className="shrink-0 text-xs text-muted-foreground tabular">
          {t('common.visits', { count: item.visitCount })}
        </span>
      )}
      {relevance && (
        <span
          className="shrink-0 font-mono text-xs text-muted-foreground tabular"
          title={relevance}
        >
          {format.percent(item.score ?? 0)}
        </span>
      )}
      <StarMark starred={starred} />
      <BrowserIcon
        browserName={browserName}
        className="size-3.5 shrink-0 opacity-85"
        decorative
      />
    </button>
  )
})

export function LoadingRow() {
  return (
    <div className="flex h-full items-center gap-3 px-2.5" aria-hidden>
      <Skeleton className="h-3 w-10" />
      <Skeleton className="size-[22px] rounded-md" />
      <Skeleton className="h-3 flex-1" />
      <Skeleton className="h-3 w-24" />
    </div>
  )
}
