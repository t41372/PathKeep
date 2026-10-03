/**
 * The windowed list shared by the timeline, search results, Sites and
 * Starred. Rows have fixed heights, so nothing is measured and the scroll
 * position stays put as pages are appended.
 *
 * Responsible for: virtualization, near-end paging callback, scroll-to-row.
 * Not responsible for: what a row looks like.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { useCallback, useEffect, useRef, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

interface Props<T> {
  rows: T[]
  rowKey: (row: T) => string
  rowHeight: (row: T) => number
  renderRow: (row: T) => ReactNode
  label: string
  /** Called when the last rows come into view. */
  onEndReached?: () => void
  /** Keeps this row on screen (e.g. the keyboard selection). */
  scrollToIndex?: number | null
  /** Scrolls back to the top when this changes (new query, new filters). */
  resetKey?: string
  className?: string
}

export function VirtualRows<T>({
  rows,
  rowKey,
  rowHeight,
  renderRow,
  label,
  onEndReached,
  scrollToIndex,
  resetKey,
  className,
}: Props<T>) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const estimateSize = useCallback(
    (index: number) => rowHeight(rows[index]),
    [rowHeight, rows],
  )
  const getItemKey = useCallback(
    (index: number) => rowKey(rows[index]),
    [rowKey, rows],
  )

  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize,
    getItemKey,
    overscan: 12,
  })

  const items = virtualizer.getVirtualItems()
  const lastIndex = items.at(-1)?.index ?? -1
  useEffect(() => {
    if (onEndReached && rows.length > 0 && lastIndex >= rows.length - 10)
      onEndReached()
  }, [lastIndex, rows.length, onEndReached])

  useEffect(() => {
    if (
      scrollToIndex !== null &&
      scrollToIndex !== undefined &&
      scrollToIndex >= 0
    ) {
      virtualizer.scrollToIndex(scrollToIndex, { align: 'auto' })
    }
  }, [scrollToIndex, virtualizer])

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 })
  }, [resetKey])

  return (
    <div
      ref={scrollRef}
      className={cn('min-h-0 flex-1 overflow-y-auto px-[18px]', className)}
    >
      <div
        role="listbox"
        aria-label={label}
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {items.map((item) => (
          <div
            key={item.key}
            role="presentation"
            className="absolute top-0 left-0 w-full"
            style={{
              height: item.size,
              transform: `translateY(${item.start}px)`,
            }}
          >
            {renderRow(rows[item.index])}
          </div>
        ))}
      </div>
    </div>
  )
}
