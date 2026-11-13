/**
 * The conversation list: virtualized, grouped by day or project with the
 * current group's header pinned at the top, ↑/↓ to move the selection.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { CHAT_ROW_HEIGHT, ChatRow, type RowMetric } from '~/components/chat-row'
import { ProjectIcon } from '~/components/bits'
import { archive } from '~/data/archive'
import { cn } from '@/lib/cn'
import { dayLabel, localDayKey, num } from '~/data/format'
import type { Chat } from '~/data/types'
import type { Group } from './filters'

type Item =
  | { kind: 'header'; key: string; label: string; count: number; projectId?: string }
  | { kind: 'chat'; chat: Chat }
  | { kind: 'more'; projectId: string; rest: number }

/** A group header in the list: free space above, then the band. Pinned at the top, only the band shows. */
const BAND = 30
const HEADER = BAND + 12
const MORE = 30
/** Grouped by project, each project shows its latest few; the rest is one click away. */
const PER_PROJECT = 3

function build(chats: Chat[], group: Group): Item[] {
  if (group === 'none') return chats.map((chat) => ({ kind: 'chat', chat }))
  const groups = new Map<string, Chat[]>()
  for (const chat of chats) {
    const key = group === 'day' ? localDayKey(chat.ended) : chat.projectId
    const list = groups.get(key)
    if (list) list.push(chat)
    else groups.set(key, [chat])
  }
  const items: Item[] = []
  for (const [key, list] of groups) {
    items.push({
      kind: 'header',
      key,
      label: group === 'day' ? dayLabel(key) : archive.projectById.get(key)!.name,
      count: list.length,
      projectId: group === 'project' ? key : undefined,
    })
    const shown = group === 'project' ? list.slice(0, PER_PROJECT) : list
    for (const chat of shown) items.push({ kind: 'chat', chat })
    if (shown.length < list.length) items.push({ kind: 'more', projectId: key, rest: list.length - shown.length })
  }
  return items
}

/**
 * A day (or project) divider. Rows are rounded and inset, so the divider uses
 * what rows never do: it runs edge to edge on a tinted band between two
 * hairlines, with extra space above it, and is easy to spot while scrolling.
 */
function Header({ item, pinned }: { item: Extract<Item, { kind: 'header' }>; pinned?: boolean }) {
  const project = item.projectId ? archive.projectById.get(item.projectId) : undefined
  return (
    <div className={cn('flex flex-col justify-end', !pinned && 'h-[42px]')}>
      <div
        role="heading"
        aria-level={3}
        className={cn('flex h-[30px] items-center gap-2 border-y border-foreground/[0.07] bg-foreground/[0.04] px-5 text-[12.5px] dark:bg-white/[0.05]', pinned && 'border-t-transparent')}
      >
        <span className="flex min-w-0 items-center gap-1.5 font-semibold text-foreground">
          {project && <ProjectIcon project={project} />}
          <span className="truncate">{item.label}</span>
        </span>
        <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">{num(item.count)} 段</span>
      </div>
    </div>
  )
}

/**
 * Which group's band is pinned at the top, and how far the next band has pushed it up.
 * Nothing is pinned while a group's own band is still on screen in its place (at the
 * very top of the list, for one), so the two never show at once; when the next band
 * reaches the pinned one it slides it out, the way sticky list headers do on macOS.
 */
function pinnedBand(measured: { start: number }[], headerIndexes: number[], offset: number) {
  const bandTop = (i: number) => measured[headerIndexes[i]].start + (HEADER - BAND) - offset
  // Last header whose band has gone above the top edge (binary search: lists can be long).
  let lo = 0
  let hi = headerIndexes.length - 1
  let k = -1
  while (lo <= hi && measured.length) {
    const mid = (lo + hi) >> 1
    if (bandTop(mid) < 0) {
      k = mid
      lo = mid + 1
    } else hi = mid - 1
  }
  if (k < 0) return { k, shift: 0 }
  const next = k + 1 < headerIndexes.length ? bandTop(k + 1) : Infinity
  return { k, shift: Math.min(0, Math.round(next - BAND)) }
}

export function ChatList({
  chats,
  group,
  selected,
  onSelect,
  onPickProject,
  metric,
  scoped,
}: {
  chats: Chat[]
  group: Group
  selected?: number
  onSelect: (chat: Chat) => void
  onPickProject?: (projectId: string) => void
  metric?: RowMetric
  /** Listing one project's chats: no project name on each row. */
  scoped?: boolean
}) {
  const items = useMemo(() => build(chats, group), [chats, group])
  const scrollRef = useRef<HTMLDivElement>(null)
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (items[i].kind === 'header' ? HEADER : items[i].kind === 'more' ? MORE : CHAT_ROW_HEIGHT),
    overscan: 12,
    // Rows brought into view by ↑/↓ or a search hit stop below the pinned band, not under it.
    scrollPaddingStart: BAND,
  })
  const virtualItems = virtualizer.getVirtualItems()
  const headerIndexes = useMemo(() => items.flatMap((it, i) => (it.kind === 'header' ? [i] : [])), [items])
  // Worked out on every scroll event: the virtualizer only re-renders when the set of
  // rendered rows changes, so its offset can be a few pixels behind mid-scroll. State
  // changes only when the pinned group or its push changes.
  const [band, setBand] = useState({ k: -1, shift: 0 })
  const updateBand = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const next = pinnedBand(virtualizer.measurementsCache, headerIndexes, el.scrollTop)
    setBand((prev) => (prev.k === next.k && prev.shift === next.shift ? prev : next))
  }, [virtualizer, headerIndexes])
  useLayoutEffect(updateBand, [updateBand, items])
  const pinned = band.k >= 0 && band.k < headerIndexes.length ? (items[headerIndexes[band.k]] as Extract<Item, { kind: 'header' }>) : null
  const shift = band.shift

  // Keep the selected row in view when it changes from outside (search result, ↑/↓).
  const selectedIndex = useMemo(() => items.findIndex((it) => it.kind === 'chat' && it.chat.id === selected), [items, selected])
  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: 'auto' })
  }, [selectedIndex, virtualizer])

  const move = (delta: number) => {
    let i = selectedIndex
    for (;;) {
      i += delta
      if (i < 0 || i >= items.length) return
      const it = items[i]
      if (it.kind === 'chat') return onSelect(it.chat)
    }
  }

  if (chats.length === 0) {
    return <div className="flex flex-1 items-center justify-center p-8 text-center text-[13px] text-muted-foreground">沒有符合條件的對話。<br />試試放寬時間或清除篩選。</div>
  }

  return (
    // Clipped, so a pinned band being pushed out slides under the edge instead of over the search box.
    <div className="relative min-h-0 flex-1 overflow-hidden">
      {pinned && (
        // The panel is see-through glass: two layers of it plus a blur keep the rows passing
        // under the pinned band from showing through.
        <div className="absolute inset-x-0 top-0 z-10 bg-panel backdrop-blur-2xl backdrop-saturate-150" style={shift ? { transform: `translateY(${shift}px)` } : undefined}>
          <div className="bg-panel">
            <Header item={pinned} pinned />
          </div>
        </div>
      )}
      <div
        ref={scrollRef}
        tabIndex={0}
        aria-label="對話列表，用上下鍵切換"
        className="thin-scroll h-full overflow-y-auto outline-none"
        onScroll={updateBand}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown' || e.key === 'j') {
            e.preventDefault()
            move(1)
          } else if (e.key === 'ArrowUp' || e.key === 'k') {
            e.preventDefault()
            move(-1)
          }
        }}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualItems.map((v) => {
            const it = items[v.index]
            return (
              <div key={v.key} style={{ position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${v.start}px)` }}>
                {it.kind === 'header' ? (
                  <Header item={it} />
                ) : it.kind === 'more' ? (
                  <button type="button" onClick={() => onPickProject?.(it.projectId)} className="flex h-[30px] w-full items-center px-5 text-left text-[12px] text-brand hover:underline">
                    還有 {num(it.rest)} 段，只看這個專案 →
                  </button>
                ) : (
                  <ChatRow chat={it.chat} selected={it.chat.id === selected} onSelect={onSelect} showProject={group !== 'project' && !scoped} metric={metric} timeOnly={group === 'day'} />
                )}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
