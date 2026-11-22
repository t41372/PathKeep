/** Pieces Insights and Home share: layout rows, scope controls, the year calendar, share bars. */
import { Check, ChevronDown, ChevronLeft, ChevronRight, SlidersHorizontal } from 'lucide-react'
import { useMemo, useState, type ReactNode } from 'react'
import { HeatLegend, Heatmap, type HeatCell } from '@/components/app/heatmap'
import { heatLevel } from '@/components/app/heat-level'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from '@/lib/cn'
import { HarnessIcon, ProjectIcon } from '~/components/bits'
import { archive, harnesses, harnessOrder, projectsByRecent } from '~/data/archive'
import { DAY, NOW } from '~/data/fixture'
import { localDayKey, num } from '~/data/format'
import { rangeLabels, type Range, type Scope } from './data'

export function Row({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-4 max-[900px]:grid-cols-1">{children}</div>
}

export function Heading({ children, sub }: { children: ReactNode; sub?: ReactNode }) {
  return (
    <div className="mt-3 flex items-baseline gap-3">
      <h2 className="text-[15px] font-semibold">{children}</h2>
      {sub && <span className="text-xs text-muted-foreground">{sub}</span>}
    </div>
  )
}

export function Legend({ items }: { items: { label: ReactNode; color: string }[] }) {
  return (
    <span className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
      {items.map((item, i) => (
        <span key={i} className="flex items-center gap-1.5">
          <span className="size-2 rounded-[2px]" style={{ background: item.color }} />
          {item.label}
        </span>
      ))}
    </span>
  )
}

export { harnessColor } from '~/data/archive'

/**
 * Scope controls: the range stays visible; tools and project sit behind one
 * 篩選 button that shows how many are on.
 */
export function ScopeControls({ scope, onChange }: { scope: Scope; onChange: (scope: Scope) => void }) {
  const project = scope.projectId ? archive.projectById.get(scope.projectId) : undefined
  const active = scope.harnesses.length + (project ? 1 : 0)
  const toggle = (h: (typeof harnessOrder)[number]) =>
    onChange({ ...scope, harnesses: scope.harnesses.includes(h) ? scope.harnesses.filter((x) => x !== h) : [...scope.harnesses, h] })
  return (
    <div className="flex flex-wrap items-center gap-2">
      <ToggleGroup type="single" variant="outline" size="sm" value={scope.range} onValueChange={(v) => v && onChange({ ...scope, range: v as Range })} aria-label="時間範圍">
        {(Object.keys(rangeLabels) as Range[]).map((r) => (
          <ToggleGroupItem key={r} value={r} className="px-2.5 text-[13px]">
            {rangeLabels[r]}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      <Popover>
        <PopoverTrigger asChild>
          <Button variant="outline" size="sm" className={cn('font-normal', active > 0 && 'border-brand/40')} aria-label={active ? `篩選（${active} 個條件）` : '篩選工具與專案'}>
            <SlidersHorizontal className="text-muted-foreground" />
            {active > 0 ? (
              <>
                {project ? <span className="max-w-[120px] truncate">{project.name}</span> : null}
                {scope.harnesses.length > 0 && <span>{scope.harnesses.length === 1 ? harnesses[scope.harnesses[0]].name : `${scope.harnesses.length} 個工具`}</span>}
              </>
            ) : (
              '篩選'
            )}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="end" className="w-[320px] p-0">
          <div className="flex flex-col gap-1.5 border-b px-3 py-2.5">
            <span className="text-[11.5px] font-medium text-muted-foreground">工具</span>
            <div className="flex flex-wrap gap-1.5">
              {harnessOrder.map((h) => (
                <button
                  key={h}
                  type="button"
                  aria-pressed={scope.harnesses.includes(h)}
                  onClick={() => toggle(h)}
                  className={cn(
                    'flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12.5px] whitespace-nowrap',
                    scope.harnesses.includes(h) ? 'border-brand/40 bg-brand-soft' : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                  )}
                >
                  <HarnessIcon id={h} className="size-3.5 [&_img]:size-3.5" />
                  {harnesses[h].name}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center justify-between px-3 pt-2.5 text-[11.5px] font-medium text-muted-foreground">
            專案
            {project && (
              <button type="button" className="font-normal hover:text-foreground" onClick={() => onChange({ ...scope, projectId: undefined })}>
                全部專案
              </button>
            )}
          </div>
          <Command>
            <CommandInput placeholder={`在 ${num(projectsByRecent.length)} 個專案裡找…`} />
            <CommandList className="max-h-[260px]">
              <CommandEmpty>沒有符合的專案</CommandEmpty>
              <CommandGroup>
                {projectsByRecent.map((p) => (
                  <CommandItem key={p.id} value={`${p.name} ${p.locations[0].path}`} onSelect={() => onChange({ ...scope, projectId: p.id === scope.projectId ? undefined : p.id })}>
                    <ProjectIcon project={p} />
                    <span className="truncate">{p.name}</span>
                    <span className="ml-auto truncate text-xs text-muted-foreground">{p.parent || '零散'}</span>
                    {scope.projectId === p.id && <Check className="size-3.5" />}
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  )
}

/** One row under 更多: a title and a one-line summary; the cards open under it. */
export function MoreSection({ title, summary, children }: { title: string; summary?: ReactNode; children: ReactNode }) {
  const [open, setOpen] = useState(false)
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="flex flex-col gap-4">
      <CollapsibleTrigger className="group flex w-full items-center gap-3 rounded-xl border bg-card px-5 py-3.5 text-left shadow-card transition-colors hover:bg-card/80">
        <span className="text-[14px] font-semibold">{title}</span>
        {summary && <span className="min-w-0 flex-1 truncate text-[13px] text-muted-foreground">{summary}</span>}
        <ChevronDown className="ml-auto size-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-4">{children}</CollapsibleContent>
    </Collapsible>
  )
}

function addDays(key: string, days: number) {
  return new Date(Date.parse(key) + days * DAY).toISOString().slice(0, 10)
}

export function yearColumns(year: number, counts: Map<string, number>, label: (key: string, value: number) => string): HeatCell[][] {
  const today = localDayKey(NOW)
  const max = Math.max(1e-9, ...counts.values())
  const first = `${year}-01-01`
  const offset = (new Date(Date.parse(first)).getUTCDay() + 6) % 7
  const columns: HeatCell[][] = []
  for (let week = 0; week < 54; week++) {
    const column: HeatCell[] = []
    for (let row = 0; row < 7; row++) {
      const key = addDays(first, week * 7 + row - offset)
      const outside = !key.startsWith(`${year}-`) || key > today
      const value = counts.get(key) ?? 0
      column.push(outside ? { key: `${week}-${row}`, level: 0, label: '', empty: true } : { key, level: heatLevel(value, max), label: label(key, value) })
    }
    if (column.some((cell) => !cell.empty)) columns.push(column)
  }
  return columns
}

/** Year calendar card body: year stepper lives in the caller's card action. */
export function YearStepper({ year, min, max, onChange }: { year: number; min: number; max: number; onChange: (year: number) => void }) {
  return (
    <div className="flex items-center gap-1">
      <Button variant="outline" size="icon" className="size-7" aria-label="上一年" disabled={year <= min} onClick={() => onChange(year - 1)}>
        <ChevronLeft />
      </Button>
      <span className="px-2 font-mono text-[13px] font-medium tabular">{year}</span>
      <Button variant="outline" size="icon" className="size-7" aria-label="下一年" disabled={year >= max} onClick={() => onChange(year + 1)}>
        <ChevronRight />
      </Button>
    </div>
  )
}

export function YearGrid({
  year,
  counts,
  label,
  summary,
  onDay,
}: {
  year: number
  counts: Map<string, number>
  label: (key: string, value: number) => string
  /** A line under the grid; leave it out when the totals are shown elsewhere. */
  summary?: ReactNode
  onDay?: (key: string) => void
}) {
  const columns = useMemo(() => yearColumns(year, counts, label), [year, counts, label])
  return (
    <>
      <div className="overflow-x-auto pb-1">
        <Heatmap columns={columns} label={`${year}`} onCellClick={onDay} />
      </div>
      <div className={cn('flex flex-wrap items-center gap-2 text-xs text-muted-foreground', summary ? 'justify-between' : 'justify-end')}>
        {summary && <span className="tabular">{summary}</span>}
        <HeatLegend less="少" more="多" />
      </div>
    </>
  )
}

/** Horizontal share bars: label, bar, value. */
export function ShareList({
  items,
  className,
}: {
  items: { key: string; label: ReactNode; value: number; display: ReactNode; sub?: ReactNode; color?: string; to?: ReactNode }[]
  className?: string
}) {
  const max = Math.max(1e-9, ...items.map((i) => i.value))
  return (
    <ul className={cn('flex flex-col gap-2.5', className)}>
      {items.map((item) => (
        <li key={item.key} className="flex flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2 text-[13px]">
            <span className="flex min-w-0 flex-1 items-center gap-1.5 truncate">{item.label}</span>
            {item.sub && <span className="shrink-0 text-xs text-muted-foreground tabular">{item.sub}</span>}
            <span className="w-[76px] shrink-0 text-right font-medium tabular">{item.display}</span>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-muted">
            <span className="block h-full rounded-full" style={{ width: `${(item.value / max) * 100}%`, background: item.color ?? 'var(--brand)' }} />
          </div>
        </li>
      ))}
    </ul>
  )
}
