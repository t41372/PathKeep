/** The detail panel's numbers: four stat tiles and the 12-week bar chart, from `get_url_detail`. */
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { useUrlDetail } from './queries'

function Tile({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 flex-col gap-[3px] rounded-[10px] bg-muted px-3 py-2.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="truncate font-semibold tabular" title={value}>
        {value}
      </span>
    </div>
  )
}

export function DetailStats({ url }: { url: string }) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = useUrlDetail(url)

  if (detail.isPending) {
    return (
      <>
        <div className="grid grid-cols-2 gap-2.5" aria-hidden>
          {Array.from({ length: 4 }, (_, index) => (
            <Skeleton key={index} className="h-[58px] rounded-[10px]" />
          ))}
        </div>
        <Skeleton className="h-16 w-full rounded-md" aria-hidden />
      </>
    )
  }
  if (detail.isError) {
    return (
      <div className="flex flex-col items-start gap-2 rounded-[10px] bg-muted px-3 py-2.5">
        <p className="text-[13px] text-muted-foreground">
          {t('history.detail.loadFailed')}
        </p>
        <Button
          variant="outline"
          size="xs"
          onClick={() => void detail.refetch()}
        >
          {t('common.retry')}
        </Button>
      </div>
    )
  }

  const data = detail.data
  const weeks = data.weeklyVisits
  const peak = Math.max(1, ...weeks.map((week) => week.visits))
  return (
    <>
      <div className="grid grid-cols-2 gap-2.5">
        <Tile
          label={t('history.detail.statVisits')}
          value={format.number(data.totalVisits)}
        />
        <Tile
          label={t('history.detail.statFirst')}
          value={data.firstVisitAt ? format.date(data.firstVisitAt) : '—'}
        />
        <Tile
          label={t('history.detail.statLast')}
          value={data.lastVisitAt ? format.dayAndTime(data.lastVisitAt) : '—'}
        />
        <Tile
          label={t('history.detail.statBrowser')}
          value={data.browsers.length > 0 ? data.browsers.join(', ') : '—'}
        />
      </div>
      <div className="flex flex-col gap-2">
        <span className="text-xs text-muted-foreground">
          {t('history.detail.weeks')}
        </span>
        <div
          className="flex h-12 items-end gap-[3px]"
          role="img"
          aria-label={t('history.detail.weeks')}
        >
          {weeks.map((week, index) => (
            <span
              key={week.weekStart}
              title={t('history.detail.weekTip', {
                date: format.monthDay(week.weekStart),
                visits: t('common.visits', { count: week.visits }),
              })}
              className={cn(
                'min-h-[2px] flex-1 rounded-t-[3px] bg-brand transition-[height] duration-300',
                index < weeks.length - 1 && 'opacity-55',
                week.visits === 0 && 'bg-muted-foreground/30 opacity-100',
              )}
              style={{ height: `${Math.max(4, (week.visits / peak) * 100)}%` }}
            />
          ))}
        </div>
      </div>
    </>
  )
}
