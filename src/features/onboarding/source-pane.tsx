/**
 * The parts every kind's pane of the sources step shares: the header (name,
 * summary, a filter once the list is long, Select all and Select none) and
 * the list, which scrolls by itself and, past eight sources, is grouped by
 * browser or tool with one tick per group.
 *
 * Not responsible for: what a row shows (each kind's pane draws its own
 * rows) or which sources are ticked (the draft).
 */
import { useId, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { filtered, GROUP_AFTER, type ListItem } from './sources-model'

export function PaneHeader({
  headingId,
  title,
  summary,
  alert,
  filter,
  onFilter,
  showFilter,
  ids,
  chosen,
  onSet,
}: {
  headingId: string
  title: string
  summary: string
  /** The summary is a problem: nothing PathKeep can read is ticked. */
  alert: boolean
  filter: string
  onFilter: (filter: string) => void
  showFilter: boolean
  /** Every source of the kind, for Select all and Select none. */
  ids: string[]
  chosen: ReadonlySet<string>
  onSet: (ids: string[], on: boolean) => void
}) {
  const { t } = useI18n()
  const picked = ids.filter((id) => chosen.has(id)).length
  return (
    <header className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-5 py-3">
      <h2 id={headingId} className="text-sm font-semibold">
        {title}
      </h2>
      <span
        aria-live="polite"
        className={cn(
          'text-xs',
          alert ? 'text-brand' : 'text-muted-foreground',
        )}
      >
        {summary}
      </span>
      <span className="flex-1" />
      {showFilter && (
        <Input
          value={filter}
          onChange={(event) => onFilter(event.target.value)}
          placeholder={t('onboarding.sources.filter')}
          aria-label={t('onboarding.sources.filterLabel', { kind: title })}
          className="h-8 w-[150px] text-[13px]"
        />
      )}
      <Button
        size="sm"
        variant="ghost"
        disabled={ids.length === 0 || picked === ids.length}
        onClick={() => onSet(ids, true)}
      >
        {t('onboarding.sources.selectAll')}
      </Button>
      <Button
        size="sm"
        variant="ghost"
        disabled={picked === 0}
        onClick={() => onSet(ids, false)}
      >
        {t('onboarding.sources.selectNone')}
      </Button>
    </header>
  )
}

function GroupHeading({
  name,
  ids,
  chosen,
  onSet,
}: {
  name: string
  ids: string[]
  chosen: ReadonlySet<string>
  onSet: (ids: string[], on: boolean) => void
}) {
  const { t } = useI18n()
  const id = useId()
  const picked = ids.filter((item) => chosen.has(item)).length
  return (
    <li className="sticky top-0 z-10 border-b bg-card/95 backdrop-blur">
      <label
        htmlFor={id}
        className="flex cursor-pointer items-center gap-3.5 px-5 py-1.5"
      >
        <Checkbox
          id={id}
          aria-label={t('onboarding.sources.groupLabel', { group: name })}
          checked={
            picked === ids.length
              ? true
              : picked === 0
                ? false
                : 'indeterminate'
          }
          onCheckedChange={(value) => onSet(ids, value === true)}
          className="size-[18px] rounded-[5px]"
        />
        <span className="text-xs font-semibold text-muted-foreground">
          {name}
        </span>
        <span className="text-xs text-muted-foreground tabular">
          {picked} / {ids.length}
        </span>
      </label>
    </li>
  )
}

/**
 * The pane's list. Rows come from `row`, which draws one `<li>`; a long list
 * is grouped, each group's tick setting all of its rows.
 */
export function SourceList<T extends ListItem>({
  label,
  items,
  chosen,
  filter,
  onSet,
  row,
}: {
  label: string
  items: T[]
  chosen: ReadonlySet<string>
  filter: string
  onSet: (ids: string[], on: boolean) => void
  row: (item: T) => ReactNode
}) {
  const { t } = useI18n()
  const long = items.length > GROUP_AFTER
  const visible = filtered(items, filter)
  const groups = [...new Set(visible.map((item) => item.group))]
  return (
    <ul aria-label={label} className="min-h-0 flex-1 overflow-y-auto">
      {visible.length === 0 && (
        <li className="px-5 py-6 text-[13px] text-muted-foreground">
          {t('onboarding.sources.noMatch', { filter: filter.trim() })}
        </li>
      )}
      {long
        ? groups.flatMap((group) => {
            const members = visible.filter((item) => item.group === group)
            return [
              <GroupHeading
                key={`group-${group}`}
                name={group}
                ids={members.map((item) => item.id)}
                chosen={chosen}
                onSet={onSet}
              />,
              ...members.map(row),
            ]
          })
        : visible.map(row)}
    </ul>
  )
}
