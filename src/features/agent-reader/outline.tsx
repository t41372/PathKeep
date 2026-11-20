/**
 * "What you said" (`agent-history.md` §4.5): every prompt of the line on
 * screen, with its turn number and two lines of it; the turn at the top of
 * the conversation is highlighted, and a click scrolls there. Read a
 * window at a time like the conversation, so a conversation with
 * thousands of prompts opens it at once.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import type { ChatPromptRow } from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { WINDOW, useWindows } from './use-windows'

const ROW = 44

export function Outline({
  id,
  publication,
  leaf,
  promptCount,
  topTurn,
  onPick,
  onStale,
}: {
  id: string
  publication: string
  leaf: number | null
  promptCount: number
  topTurn: number | null
  /** Scrolls the conversation to the prompt's rank. */
  onPick: (visibleRank: number) => void
  onStale: () => void
}) {
  const { t, locale } = useI18n()
  const scrollRef = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState({ first: 0, last: 0 })
  const { total, rankState } = useWindows<ChatPromptRow>({
    id,
    publication,
    view: 'prompts',
    leaf,
    knownTotal: promptCount,
    range,
    onStale,
  })
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW,
    overscan: 10,
  })
  const items = virtualizer.getVirtualItems()
  const first = items[0]?.index ?? 0
  const last = items.at(-1)?.index ?? 0
  useEffect(() => {
    const next = {
      first: Math.floor(first / WINDOW) * WINDOW,
      last: Math.floor(last / WINDOW) * WINDOW,
    }
    setRange((previous) =>
      previous.first === next.first && previous.last === next.last
        ? previous
        : next,
    )
  }, [first, last])
  // Keep the highlighted prompt in view as the conversation scrolls.
  useEffect(() => {
    if (topTurn !== null && topTurn >= 1)
      virtualizer.scrollToIndex(topTurn - 1, { align: 'auto' })
  }, [topTurn, virtualizer])
  const count = useMemo(
    () => new Intl.NumberFormat(locale).format(total),
    [locale, total],
  )
  return (
    <nav
      aria-label={t('agentReader.outline.label')}
      className="hidden w-[220px] shrink-0 flex-col border-l min-[1180px]:flex"
    >
      <h2 className="px-4 pt-3 pb-2 text-[12px] font-semibold text-muted-foreground">
        {t('agentReader.outline.title', { count })}
      </h2>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto pb-3">
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {items.map((item) => {
            const state = rankState(item.index)
            return (
              <div
                key={item.key}
                className="absolute top-0 left-0 w-full"
                style={{
                  height: item.size,
                  transform: `translateY(${item.start}px)`,
                }}
              >
                {state.state === 'row' ? (
                  <button
                    type="button"
                    aria-current={
                      state.row.turn === topTurn ? 'true' : undefined
                    }
                    onClick={() => onPick(state.row.visibleRank)}
                    className={cn(
                      'flex h-full w-full gap-2 px-4 py-1 text-left text-[12px] leading-snug transition-colors duration-150 hover:bg-muted',
                      state.row.turn === topTurn
                        ? 'text-foreground'
                        : 'text-muted-foreground',
                    )}
                  >
                    <span className="w-6 shrink-0 text-right tabular opacity-60">
                      {state.row.turn}
                    </span>
                    <span className="line-clamp-2 [overflow-wrap:anywhere]">
                      {state.row.preview}
                    </span>
                  </button>
                ) : (
                  <div className="flex h-full flex-col justify-center gap-1.5 px-4">
                    <Skeleton className="h-2.5 w-4/5" />
                    <Skeleton className="h-2.5 w-3/5" />
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </nav>
  )
}
