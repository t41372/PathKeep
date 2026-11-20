/**
 * The conversation's rows, virtualized by rank (`agent-history.md` §4.5,
 * §4.9): the list is as long as the conversation from the start, each rank
 * is drawn once its window is read and holds a placeholder until then, so
 * opening, scrolling, jumping to the end or to a prompt never read the
 * whole conversation. Rows are measured as they are drawn; a row above the
 * view that grows keeps what is on screen in place.
 *
 * Responsibilities: the virtual list, which windows to read, how rows join
 * into runs of tool calls, the turn under the pointer, the prompt at the
 * top (for the outline), and scrolling to a rank on request. Not
 * responsible for: how a row looks (`row-view.tsx`).
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { TriangleAlert } from 'lucide-react'
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode,
  type RefObject,
} from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import type { ChatChildCard, ChatRow } from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters } from '@/lib/i18n'
import { useReaderPrefs } from './reader-prefs'
import { COLUMN, isToolOnly, launchesOf, toolCalls, turnKey } from './row-model'
import { RowView, type RowContext, type RunPlace } from './row-view'
import { SubagentCards } from './tool-group'
import { WINDOW, useWindows, type RankState } from './use-windows'

const ESTIMATE = 96

export interface TranscriptControl {
  scrollToRank: (rank: number, align: 'start' | 'center' | 'end') => void
  scrollToEnd: () => void
}

/** A rank to bring to the middle once drawn; a new `nonce` asks again. */
export interface ScrollRequest {
  rank: number
  nonce: number
  /**
   * The row itself is what was asked for (a record opened from a semantic
   * result or an Ask citation, with no words to light): it lights up for a
   * moment, so the eye finds it, and stays the row opened at
   * (`data-opened-at`) until another is asked for.
   */
  mark?: boolean
}

/** How long a row opened at stays lit. */
const GLOW_MS = 1600

function Placeholder() {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('agentReader.rows.placeholder')}
      className="flex flex-col gap-2 py-3"
    >
      <Skeleton className="h-3.5 w-4/5" />
      <Skeleton className="h-3.5 w-3/5" />
    </div>
  )
}

function WindowFailed({ error, retry }: { error: Error; retry: () => void }) {
  const { t } = useI18n()
  return (
    <div className="my-2 flex items-start gap-2 rounded-lg border px-3 py-2.5 text-[12.5px]">
      <TriangleAlert
        className="mt-0.5 size-3.5 shrink-0 text-destructive"
        aria-hidden
      />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span>{t('agentReader.rows.windowFailed')}</span>
        <span className="break-words text-muted-foreground">
          {error.message}
        </span>
      </div>
      <Button variant="outline" size="sm" onClick={retry}>
        {t('agentReader.rows.retry')}
      </Button>
    </div>
  )
}

/** How row `rank` stands in a run of tool-only rows. */
function placeOf(
  rank: number,
  row: ChatRow,
  rankState: (rank: number) => RankState<ChatRow>,
  total: number,
  showThinking: boolean,
): RunPlace {
  if (!isToolOnly(row, showThinking)) return { kind: 'alone' }
  const previous = rank > 0 ? rankState(rank - 1) : null
  if (
    previous?.state === 'row' &&
    isToolOnly(previous.row, showThinking) &&
    turnKey(previous.row) === turnKey(row)
  )
    return { kind: 'continued' }
  const rows = [row]
  for (let next = rank + 1; next < total; next += 1) {
    const state = rankState(next)
    if (
      state.state !== 'row' ||
      !isToolOnly(state.row, showThinking) ||
      turnKey(state.row) !== turnKey(row)
    )
      break
    rows.push(state.row)
  }
  return { kind: 'start', calls: toolCalls(rows), rows }
}

export function Transcript({
  id,
  publication,
  leaf,
  knownTotal,
  subagents,
  f,
  control,
  switchedPrompt,
  onPickVersion,
  onOpenChild,
  onTopTurn,
  onStale,
  scrollRequest = null,
}: {
  id: string
  publication: string
  leaf: number | null
  knownTotal: number
  /** This conversation's subagents (cards). */
  subagents: ChatChildCard[]
  f: Formatters
  control: RefObject<TranscriptControl | null>
  /** The prompt whose earlier version is shown (`sourceId:ordinal`). */
  switchedPrompt: string | null
  onPickVersion: (leaf: number, promptKey: string) => void
  onOpenChild: (child: ChatChildCard) => void
  /** The turn number of the row at the top, for the outline. */
  onTopTurn?: (turn: number | null) => void
  onStale: () => void
  /** Find moved to a record here. */
  scrollRequest?: ScrollRequest | null
}) {
  const { t } = useI18n()
  const prefs = useReaderPrefs()
  const scrollRef = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState<{ first: number; last: number }>({
    first: 0,
    last: 0,
  })
  const { total, rankState } = useWindows<ChatRow>({
    id,
    publication,
    view: 'visible',
    leaf,
    knownTotal,
    range,
    onStale,
  })
  const unplaced = useMemo(
    () => subagents.filter((child) => child.launch === null),
    [subagents],
  )
  const launches = useMemo(() => launchesOf(subagents), [subagents])
  const count = total + (unplaced.length > 0 ? 1 : 0)

  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ESTIMATE,
    overscan: 6,
  })
  const items = virtualizer.getVirtualItems()
  const firstIndex = items[0]?.index ?? 0
  const lastIndex = items.at(-1)?.index ?? 0

  useEffect(() => {
    const first = Math.floor(firstIndex / WINDOW) * WINDOW
    const last =
      Math.floor(Math.min(lastIndex, Math.max(0, total - 1)) / WINDOW) * WINDOW
    setRange((previous) =>
      previous.first === first && previous.last === last
        ? previous
        : { first, last },
    )
  }, [firstIndex, lastIndex, total])

  useEffect(() => {
    control.current = {
      scrollToRank: (rank, align) =>
        virtualizer.scrollToIndex(Math.min(rank, Math.max(0, count - 1)), {
          align,
        }),
      scrollToEnd: () =>
        virtualizer.scrollToIndex(Math.max(0, count - 1), { align: 'end' }),
    }
  }, [control, virtualizer, count])

  const requestNonce = scrollRequest?.nonce
  const requestRank = scrollRequest?.rank
  const requestMark = scrollRequest?.mark ?? false
  const [opened, setOpened] = useState<{ rank: number; lit: boolean } | null>(
    null,
  )
  useEffect(() => {
    if (requestRank === undefined) return
    virtualizer.scrollToIndex(requestRank, { align: 'center' })
    setOpened(requestMark ? { rank: requestRank, lit: true } : null)
    if (!requestMark) return
    const timer = setTimeout(
      () => setOpened({ rank: requestRank, lit: false }),
      GLOW_MS,
    )
    return () => clearTimeout(timer)
  }, [requestNonce, requestRank, requestMark, virtualizer])
  // Its window may arrive after the first scroll, and rows drawn above it
  // then take their real heights: centre it again once it is drawn.
  const openedDrawn = opened ? rankState(opened.rank).state === 'row' : false
  const recentred = useRef<number | undefined>(undefined)
  useEffect(() => {
    if (!opened || !openedDrawn || recentred.current === requestNonce) return
    recentred.current = requestNonce
    virtualizer.scrollToIndex(opened.rank, { align: 'center' })
  }, [opened, openedDrawn, requestNonce, virtualizer])

  // The prompt at the top of the view, for the outline's highlight.
  const top = items.find(
    (item) => item.end > (virtualizer.scrollOffset ?? 0),
  )?.index
  const topState = top === undefined ? null : rankState(top)
  const topTurn = topState?.state === 'row' ? topState.row.turnNumber : null
  useEffect(() => {
    onTopTurn?.(topTurn)
  }, [topTurn, onTopTurn])

  const [activeTurn, setActiveTurn] = useState<string | null>(null)
  const onPointer = useCallback((event: MouseEvent) => {
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      '[data-turn]',
    )
    const next = target?.dataset.turn ?? null
    setActiveTurn((previous) => (previous === next ? previous : next))
  }, [])

  const switched = useMemo(() => {
    if (!switchedPrompt) return null
    for (const item of items) {
      const state = rankState(item.index)
      if (
        state.state === 'row' &&
        `${state.row.sourceId}:${state.row.ordinal}` === switchedPrompt
      )
        return {
          key: switchedPrompt,
          records: Math.max(0, total - item.index - 1),
        }
    }
    return { key: switchedPrompt, records: 0 }
  }, [switchedPrompt, items, rankState, total])

  const context: RowContext = {
    id,
    publication,
    f,
    launches,
    onOpenChild,
    onPickVersion,
    switched,
    activeTurn,
    prefs,
  }

  return (
    <div
      ref={scrollRef}
      data-transcript=""
      className="min-w-0 flex-1 overflow-y-auto"
      onMouseOver={onPointer}
      onMouseLeave={() => setActiveTurn(null)}
    >
      <div
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {items.map((item) => {
          let body: ReactNode
          let turn: string | undefined
          if (item.index >= total) {
            body = (
              <div className={cn(COLUMN, 'flex flex-col gap-2 pt-4 pb-8')}>
                <p className="text-[12px] text-muted-foreground">
                  {t('agentReader.rows.unplaced', { count: unplaced.length })}
                </p>
                <SubagentCards cards={unplaced} onOpen={onOpenChild} />
              </div>
            )
          } else {
            const state = rankState(item.index)
            if (state.state === 'loading')
              body = (
                <div className={cn(COLUMN, item.index === 0 && 'pt-5')}>
                  <Placeholder />
                </div>
              )
            else if (state.state === 'failed')
              body =
                item.index % WINDOW === 0 || item.index === firstIndex ? (
                  <div className={COLUMN}>
                    <WindowFailed error={state.error} retry={state.retry} />
                  </div>
                ) : null
            else {
              turn = turnKey(state.row) ?? undefined
              body = (
                <RowView
                  row={state.row}
                  place={placeOf(
                    item.index,
                    state.row,
                    rankState,
                    total,
                    prefs.showThinking,
                  )}
                  context={context}
                  first={item.index === 0}
                />
              )
            }
          }
          return (
            <div
              key={item.key}
              data-index={item.index}
              data-turn={turn}
              data-rank={item.index}
              data-opened-at={opened?.rank === item.index || undefined}
              ref={virtualizer.measureElement}
              className={cn(
                'absolute top-0 left-0 w-full rounded-lg transition-colors duration-700',
                opened?.rank === item.index && opened.lit && 'bg-brand-soft/70',
              )}
              style={{ transform: `translateY(${item.start}px)` }}
            >
              {body}
            </div>
          )
        })}
      </div>
    </div>
  )
}
