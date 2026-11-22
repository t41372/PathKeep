/**
 * The one row grid every projects view uses, so 預設, 清單 and 資料夾 line
 * up the same way: name and where it lives, tools, conversations, last
 * activity, menu. 清單 adds two number columns before 最近活動. Groupings
 * (your sections, parent folders) are a chevron and text in the name cell;
 * only projects get an icon.
 */
import { ArrowDown, ArrowUp, ChevronRight, GripVertical } from 'lucide-react'
import { useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { HarnessIcon, ProjectIcon } from '~/components/bits'
import { archive, harnesses } from '~/data/archive'
import { num } from '~/data/format'
import { timeWords } from '~/data/labels'
import type { HarnessId } from '~/data/types'
import { useChatEdits } from '../chats/chat-edits'
import type { Row } from './derive'
import { locatorText, relTime, type Locator } from './use-rows'

export const ROW_H = 40
export const COLS = 'grid grid-cols-[minmax(0,1fr)_88px_56px_80px_32px] items-center gap-3'
export const LIST_COLS = 'grid grid-cols-[minmax(0,1fr)_88px_56px_92px_72px_80px_32px] items-center gap-3'
/** Each tree level indents the name cell by this much. */
export const INDENT = 18

export type SortKey = 'recent' | 'chats' | 'summed' | 'tokens' | 'name'

/** Column headings; in 清單 the sortable ones are buttons. */
export function ColumnHead({ list, sort, desc, onSort }: { list?: boolean; sort?: SortKey; desc?: boolean; onSort?: (k: SortKey) => void }) {
  const cols: { key: SortKey | null; label: string; right?: boolean; title?: string }[] = [
    { key: 'name', label: '專案' },
    { key: null, label: '工具' },
    { key: 'chats', label: '對話', right: true },
    ...(list
      ? [
          { key: 'summed' as const, label: timeWords.summed, right: true, title: timeWords.summedHow },
          { key: 'tokens' as const, label: 'Token', right: true },
        ]
      : []),
    { key: 'recent', label: '最近活動', right: true },
    { key: null, label: '' },
  ]
  return (
    <div role="row" className={cn(list ? LIST_COLS : COLS, 'h-8 border-b pr-1 pl-5 text-[12px] text-muted-foreground')}>
      {cols.map((c, i) => {
        const body = (
          <>
            {c.label}
            {sort === c.key && onSort && (desc ? <ArrowDown className="size-3" /> : <ArrowUp className="size-3" />)}
          </>
        )
        const cls = cn('flex items-center gap-1', c.right && 'justify-end', sort === c.key && onSort && 'font-medium text-foreground')
        const cell =
          c.key && onSort ? (
            <button key={i} type="button" role="columnheader" aria-sort={sort === c.key ? (desc ? 'descending' : 'ascending') : 'none'} onClick={() => onSort(c.key!)} className={cn(cls, 'hover:text-foreground')}>
              {body}
            </button>
          ) : (
            <span key={i} role="columnheader" className={cls}>
              {body}
            </span>
          )
        return c.title ? (
          <Tooltip key={i}>
            <TooltipTrigger asChild>{cell}</TooltipTrigger>
            <TooltipContent className="max-w-[260px]">{c.title}</TooltipContent>
          </Tooltip>
        ) : (
          cell
        )
      })}
    </div>
  )
}

export function Grip() {
  return <GripVertical aria-hidden className="absolute top-1/2 left-1 size-3.5 -translate-y-1/2 text-muted-foreground/60 opacity-0 group-hover:opacity-100" />
}

export function Tools({ ids }: { ids: HarnessId[] }) {
  return (
    <span className="flex items-center gap-1.5" aria-label={ids.map((h) => harnesses[h].name).join('、')}>
      {ids.map((h) => (
        <HarnessIcon key={h} id={h} />
      ))}
    </span>
  )
}

/** Name cell of a project: icon, name, and where it lives. */
export function LocatorText({ where }: { where: Locator }) {
  return (
    <span title={locatorText(where)} className="min-w-0 truncate text-[12px] text-muted-foreground">
      {where.path && <span className="font-mono text-[11.5px]">{where.path}</span>}
      {where.path && where.extra && ' · '}
      {where.extra}
    </span>
  )
}

export function NameCell({ row, name, where, depth = 0 }: { row: Row; name: string; where: Locator; depth?: number }) {
  // Chat ids follow recency, so the smallest id is the project's latest conversation. Trashed ones don't count.
  const { trashed } = useChatEdits()
  const shown = row.project.chatIds.filter((id) => !trashed.has(id))
  const latest = shown.length ? archive.chatById.get(Math.min(...shown))?.title : undefined
  return (
    <span className="flex min-w-0 items-center gap-2" style={{ paddingLeft: depth * INDENT }}>
      {row.project.gone ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <span data-no-drag className="flex">
              <ProjectIcon project={row.project} />
            </span>
          </TooltipTrigger>
          <TooltipContent>資料夾已不在磁碟上，對話仍完整保存在 PathKeep</TooltipContent>
        </Tooltip>
      ) : (
        <ProjectIcon project={row.project} />
      )}
      <span className={cn('shrink-0 truncate text-[13.5px] font-medium', row.project.gone && 'text-foreground/60')}>{name}</span>
      <LocatorText where={where} />
      {latest && (
        <span title={`最近的對話：${latest}`} className="ml-auto hidden max-w-[48%] min-w-0 truncate pl-6 text-right text-[12.5px] text-muted-foreground min-[1500px]:block">
          {latest}
        </span>
      )}
    </span>
  )
}

/** Name cell of a grouping: chevron, label, count. No icon, so it never looks like a project. */
export function GroupCell({ open, label, count, depth = 0, mono, children }: { open: boolean; label: ReactNode; count: number; depth?: number; mono?: boolean; children?: ReactNode }) {
  return (
    <span className="flex min-w-0 items-center gap-1.5" style={{ paddingLeft: depth * INDENT }}>
      <ChevronRight className={cn('size-3.5 shrink-0 text-muted-foreground transition-transform', open && 'rotate-90')} />
      {children ?? <span className={cn('truncate font-semibold text-foreground/85', mono ? 'font-mono text-[12.5px]' : 'text-[13px]')}>{label}</span>}
      <span className="shrink-0 text-xs text-muted-foreground tabular">{num(count)}</span>
    </span>
  )
}

export function Num({ children, muted }: { children: ReactNode; muted?: boolean }) {
  return <span className={cn('text-right text-[13px] tabular', muted && 'text-muted-foreground')}>{children}</span>
}

export function When({ t }: { t: number }) {
  return <span className="text-right text-xs text-muted-foreground tabular">{t ? relTime(t) : ''}</span>
}

/** A card like the other pages' cards, with its own header row. */
export function Card({ title, count, actions, children, className, ...rest }: { title: ReactNode; count?: number; actions?: ReactNode; children: ReactNode; className?: string } & React.HTMLAttributes<HTMLElement>) {
  return (
    <section className={cn('flex min-w-0 flex-col rounded-xl border border-foreground/[0.06] bg-foreground/[0.022] dark:bg-white/[0.03]', className)} {...rest}>
      <header className="flex flex-wrap items-center gap-x-3 gap-y-2 px-5 pt-4 pb-3">
        <h2 className="flex items-baseline gap-1.5 text-sm font-semibold">
          {title}
          {count !== undefined && <span className="text-xs font-normal text-muted-foreground tabular">{num(count)}</span>}
        </h2>
        <span className="flex-1" />
        {actions}
      </header>
      {children}
    </section>
  )
}

/**
 * A list inside the page's own scroll (PageScroll), virtualized against it:
 * long lists render only the rows on screen.
 */
export function usePageList(count: number, size = ROW_H) {
  const ref = useRef<HTMLDivElement>(null)
  const [scrollEl, setScrollEl] = useState<HTMLElement | null>(null)
  const [margin, setMargin] = useState(0)
  useLayoutEffect(() => {
    const el = ref.current?.closest('.overflow-y-auto') as HTMLElement | null
    setScrollEl(el)
    if (!el || !ref.current) return
    const measure = () => {
      if (!ref.current) return
      const m = ref.current.getBoundingClientRect().top - el.getBoundingClientRect().top + el.scrollTop
      setMargin((prev) => (Math.abs(prev - m) > 0.5 ? m : prev))
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el.firstElementChild ?? el)
    return () => ro.disconnect()
  }, [])
  const v = useVirtualizer({ count, getScrollElement: () => scrollEl, estimateSize: () => size, scrollMargin: margin, overscan: 8 })
  return { ref, v, margin }
}
