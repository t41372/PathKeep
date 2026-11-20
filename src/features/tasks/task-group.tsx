/**
 * One group card of the background tasks page: its title, then its rows,
 * a page at a time ("Show more" reads on, so a year of hourly backups or a
 * queue of page summaries never loads at once, and the list scrolls inside
 * the card), or what it says when it is empty.
 *
 * Responsibilities: the card and its states: rows still on their way (a
 * skeleton the size of a row), a read that failed (with Try again), empty.
 * A group with no empty sentence is left out while empty, as "Needs a look"
 * and "Waiting" are. Not responsible for: which rows it has
 * (`use-task-group.ts`) or a row (`task-row.tsx`).
 */
import type { ReactNode } from 'react'
import { CardError } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { TaskGroup as Group } from '@/lib/backend-client/agent-backup'
import { useI18n } from '@/lib/i18n'
import type { TaskEntry } from './task-entry'
import { TaskRow, type Controls, type LiveWork } from './task-row'
import type { TaskGroupRows } from './use-task-group'

export function TaskGroupCard({
  group,
  title,
  icon,
  rows,
  entries = rows.entries,
  empty,
  controls,
  live,
}: {
  group: Group
  title: string
  icon?: ReactNode
  rows: TaskGroupRows
  /** The rows to draw, when the caller leaves some out. */
  entries?: TaskEntry[]
  /** What an empty group says; without it an empty group is not drawn. */
  empty?: ReactNode
  controls: Controls
  live: LiveWork
}) {
  const { t } = useI18n()
  const settled = !rows.loading && !rows.error
  if (settled && entries.length === 0 && !rows.hasMore && !empty) return null
  if (rows.loading && !empty) return null
  return (
    <section
      aria-label={title}
      data-task-group={group}
      className="animate-in overflow-hidden rounded-xl border bg-card shadow-card duration-200 fade-in-0"
    >
      <h2 className="flex items-center gap-1.5 border-b px-4 py-2.5 text-sm font-semibold">
        {icon}
        {title}
      </h2>
      {rows.loading ? (
        <div className="flex items-start gap-3 px-4 py-3">
          <span className="sr-only">{t('tasks.groups.loading')}</span>
          <Skeleton aria-hidden className="size-7 rounded-[9px]" />
          <div aria-hidden className="flex flex-1 flex-col gap-2">
            <Skeleton className="h-4 w-56" />
            <Skeleton className="h-3 w-40" />
          </div>
        </div>
      ) : rows.error && entries.length === 0 ? (
        <div className="px-4 py-3">
          <CardError error={rows.error} onRetry={rows.retry} />
        </div>
      ) : entries.length === 0 && !rows.hasMore ? (
        <p className="px-4 py-3 text-[13px] text-muted-foreground">{empty}</p>
      ) : (
        <ul className="max-h-[420px] overflow-y-auto">
          {entries.map((entry) => (
            <TaskRow
              key={entry.key}
              entry={entry}
              group={group}
              controls={controls}
              live={live}
            />
          ))}
        </ul>
      )}
      {rows.hasMore && !rows.loading && (
        <div className="border-t px-4 py-1.5">
          <Button
            variant="ghost"
            size="sm"
            disabled={rows.loadingMore}
            onClick={rows.loadMore}
          >
            {t('tasks.groups.more')}
          </Button>
        </div>
      )}
    </section>
  )
}
