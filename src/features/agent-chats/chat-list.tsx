/**
 * The conversation list (`agent-history.md` §3.3): virtualized, grouped by
 * day or project with the current group's heading pinned at the top (the
 * next heading pushes it up), ↑/↓ or k/j to move the selection, the next
 * page read as the end comes into view.
 *
 * Responsibilities: drawing the list model, the pinned heading, keys,
 * keeping the selected row in view, asking for more. Not responsible for:
 * what the rows hold (`list-model.ts`) or what selecting does.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { FolderClosed } from 'lucide-react'
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  ChatGroup,
  ChatProject,
  ChatSort,
  ChatSummary,
} from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters } from '@/lib/i18n'
import { ChatRowView } from './chat-row'
import { dayHeading } from './chat-words'
import {
  BAND_HEIGHT,
  HEADER_HEIGHT,
  ROW_HEIGHT,
  itemHeight,
  pinnedHeading,
  type ListItem,
  type ListModel,
} from './list-model'

type Header = Extract<ListItem, { kind: 'header' }>

function Heading({
  item,
  pinned,
  zone,
  f,
}: {
  item: Header
  pinned?: boolean
  zone: string
  f: Formatters
}) {
  const { t } = useI18n()
  const label =
    item.project !== undefined
      ? (item.project?.label ?? t('agentChats.groups.noFolder'))
      : dayHeading(t, f, zone, item.day ?? null)
  return (
    <div
      className={cn('flex flex-col justify-end', !pinned && 'h-full')}
      aria-hidden={pinned || undefined}
    >
      <div
        role={pinned ? undefined : 'heading'}
        aria-level={pinned ? undefined : 3}
        className={cn(
          'flex items-center gap-2 border-y border-foreground/[0.07] bg-foreground/[0.04] px-5 text-[12.5px] dark:bg-white/[0.05]',
          pinned && 'border-t-transparent',
        )}
        style={{ height: BAND_HEIGHT }}
      >
        <span className="flex min-w-0 items-center gap-1.5 font-semibold text-foreground">
          {item.project !== undefined && (
            <FolderClosed className="size-3.5 shrink-0" aria-hidden />
          )}
          <span className="truncate" title={item.project?.id}>
            {label}
          </span>
        </span>
        <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">
          {t('agentChats.groups.count', { count: item.count })}
        </span>
      </div>
    </div>
  )
}

function LoadingRow() {
  return (
    <div className="flex h-full flex-col justify-center gap-2 px-5">
      <Skeleton className="h-3.5 w-3/5" />
      <Skeleton className="h-3 w-2/5" />
    </div>
  )
}

export function ChatList({
  model,
  selectedId,
  sort,
  group,
  showProject,
  zone,
  f,
  hasMore,
  dimmed,
  onSelect,
  onPickProject,
  onEndReached,
}: {
  model: ListModel
  selectedId: string | null
  sort: ChatSort
  group: ChatGroup
  /** False when grouped by project, where the heading names it. */
  showProject: boolean
  zone: string
  f: Formatters
  hasMore: boolean
  /** Old rows while the new filters' rows load. */
  dimmed: boolean
  onSelect: (chat: ChatSummary) => void
  onPickProject: (project: ChatProject) => void
  onEndReached: () => void
}) {
  const { t } = useI18n()
  const scrollRef = useRef<HTMLDivElement>(null)
  const { items } = model
  const count = items.length + (hasMore ? 1 : 0)
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) =>
      index < items.length ? itemHeight(items[index]) : ROW_HEIGHT,
    getItemKey: (index) => (index < items.length ? items[index].key : 'more'),
    overscan: 12,
    // Rows brought into view by ↑/↓ stop below the pinned heading, not under it.
    scrollPaddingStart: group === 'none' ? 0 : BAND_HEIGHT,
  })
  const virtualItems = virtualizer.getVirtualItems()

  const [band, setBand] = useState({ index: -1, shift: 0 })
  const updateBand = useCallback(() => {
    const element = scrollRef.current
    if (!element) return
    const next = pinnedHeading(model, element.scrollTop)
    setBand((previous) =>
      previous.index === next.index && previous.shift === next.shift
        ? previous
        : next,
    )
  }, [model])
  useLayoutEffect(updateBand, [updateBand])

  const lastIndex = virtualItems.at(-1)?.index ?? -1
  useEffect(() => {
    if (hasMore && lastIndex >= items.length - 10) onEndReached()
  }, [hasMore, lastIndex, items.length, onEndReached])

  const selectedIndex = items.findIndex(
    (item) => item.kind === 'chat' && item.chat.id === selectedId,
  )
  useEffect(() => {
    if (selectedIndex >= 0)
      virtualizer.scrollToIndex(selectedIndex, { align: 'auto' })
  }, [selectedIndex, virtualizer])

  const move = (step: number) => {
    const chats = model.chats
    if (chats.length === 0) return
    const at = chats.findIndex((chat) => chat.id === selectedId)
    const next = at < 0 ? 0 : Math.min(chats.length - 1, Math.max(0, at + step))
    onSelect(chats[next])
  }

  const pinned =
    band.index >= 0 && items[band.index]?.kind === 'header'
      ? (items[band.index] as Header)
      : null

  return (
    // Clipped, so a pinned heading pushed out slides under the edge.
    <div
      className={cn(
        'relative min-h-0 flex-1 overflow-hidden transition-opacity duration-150',
        dimmed && 'opacity-60',
      )}
    >
      {pinned && (
        // The panel is see-through glass: two layers of it and a blur keep
        // the rows passing under the pinned heading from showing through.
        <div
          className="absolute inset-x-0 top-0 z-10 bg-panel backdrop-blur-2xl backdrop-saturate-150"
          style={
            band.shift
              ? { transform: `translateY(${band.shift}px)` }
              : undefined
          }
        >
          <div className="bg-panel">
            <Heading item={pinned} pinned zone={zone} f={f} />
          </div>
        </div>
      )}
      <div
        ref={scrollRef}
        role="group"
        tabIndex={0}
        aria-label={t('agentChats.listLabel')}
        aria-busy={dimmed || undefined}
        className="h-full overflow-y-auto outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset"
        onScroll={updateBand}
        onKeyDown={(event) => {
          if (event.metaKey || event.ctrlKey || event.altKey) return
          if (event.key === 'ArrowDown' || event.key === 'j') {
            event.preventDefault()
            move(1)
          } else if (event.key === 'ArrowUp' || event.key === 'k') {
            event.preventDefault()
            move(-1)
          }
        }}
      >
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualItems.map((virtual) => {
            const item = items[virtual.index]
            return (
              <div
                key={virtual.key}
                className="absolute top-0 left-0 w-full"
                style={{
                  height: virtual.size,
                  transform: `translateY(${virtual.start}px)`,
                }}
              >
                {!item ? (
                  <LoadingRow />
                ) : item.kind === 'header' ? (
                  <div style={{ height: HEADER_HEIGHT }}>
                    <Heading item={item} zone={zone} f={f} />
                  </div>
                ) : item.kind === 'more' ? (
                  <button
                    type="button"
                    data-project={item.project.id}
                    onClick={() => onPickProject(item.project)}
                    className="flex h-full w-full items-center px-5 text-left text-[12px] text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
                  >
                    {t('agentChats.groups.more', { count: item.remaining })}
                  </button>
                ) : (
                  <ChatRowView
                    chat={item.chat}
                    selected={item.chat.id === selectedId}
                    onSelect={onSelect}
                    showProject={showProject}
                    sort={sort}
                    clockOnly={group === 'day'}
                    zone={zone}
                    f={f}
                  />
                )}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}
