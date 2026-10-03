/**
 * Tile heatmap in the prototype's orange scale, used for the year calendar
 * and the weekly rhythm. EvilCharts has no heatmap, and the amicro one is a
 * fixed-data dither demo, so this is a small grid of our own.
 *
 * One tooltip is shared by the whole grid (event delegation on the container)
 * instead of one per cell: a year view has 371 cells. It is portalled to the
 * body with fixed coordinates, so the card's rounded, clipped edge can't cut
 * it off, and it flips below the cell when there is no room above.
 *
 * Keyboard: the grid is one tab stop. Arrow keys move to the nearest day in
 * that direction, Enter or Space opens it, and focus shows the same tooltip
 * as hovering. Each day is labelled for screen readers.
 */
import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type MouseEvent,
} from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/lib/cn'

import type { HeatLevel } from './heat-level'

export type { HeatLevel } from './heat-level'

export interface HeatCell {
  key: string
  level: HeatLevel
  label: string
  /** Cells outside the range (future days, padding) render as empty space. */
  empty?: boolean
}

const levelClass: Record<HeatLevel, string> = {
  0: 'bg-heat-0',
  1: 'bg-heat-1',
  2: 'bg-heat-2',
  3: 'bg-heat-3',
  4: 'bg-heat-4',
}

interface HeatmapProps {
  /** Column-major: each inner array is one column, top to bottom. */
  columns: HeatCell[][]
  cellSize?: number
  gap?: number
  rowLabels?: (string | null)[]
  columnLabels?: (string | null)[]
  onCellClick?: (key: string) => void
  /** What the grid shows, for screen readers ("Visits per day in 2026"). */
  label: string
  className?: string
}

const moves: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
}

/** Space the tooltip needs above a cell before it flips below. */
const TOOLTIP_ROOM = 40

export const Heatmap = memo(function Heatmap({
  columns,
  cellSize = 13,
  gap = 3,
  rowLabels,
  columnLabels,
  onCellClick,
  label,
  className,
}: HeatmapProps) {
  const [hover, setHover] = useState<{
    label: string
    x: number
    /** Edge of the cell the tooltip sits against, in viewport pixels. */
    y: number
    below: boolean
  } | null>(null)

  // Where each day sits, and which one holds the tab stop: the latest day,
  // so a keyboard user starts at today rather than at last January.
  const { position, lastKey } = useMemo(() => {
    const position = new Map<string, [number, number]>()
    let lastKey: string | null = null
    columns.forEach((column, x) =>
      column.forEach((cell, y) => {
        if (cell.empty) return
        position.set(cell.key, [x, y])
        lastKey = cell.key
      }),
    )
    return { position, lastKey }
  }, [columns])
  const [focusKey, setFocusKey] = useState<string | null>(null)
  const tabStop = focusKey && position.has(focusKey) ? focusKey : lastKey

  const showFor = useCallback((target: HTMLElement | null) => {
    if (!target?.dataset.label) {
      setHover(null)
      return
    }
    const box = target.getBoundingClientRect()
    const below = box.top < TOOLTIP_ROOM
    setHover({
      label: target.dataset.label,
      x: box.left + box.width / 2,
      y: below ? box.bottom : box.top,
      below,
    })
  }, [])

  const onMove = useCallback(
    (event: MouseEvent) =>
      showFor(
        (event.target as HTMLElement).closest<HTMLElement>('[data-cell]'),
      ),
    [showFor],
  )

  const onFocus = useCallback(
    (event: FocusEvent) => {
      const target = event.target as HTMLElement
      if (target.dataset.cell) setFocusKey(target.dataset.cell)
      showFor(target)
    },
    [showFor],
  )

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const key = (event.target as HTMLElement).dataset.cell
      if (!key) return
      if ((event.key === 'Enter' || event.key === ' ') && onCellClick) {
        event.preventDefault()
        onCellClick(key)
        return
      }
      const move = moves[event.key]
      const from = position.get(key)
      if (!move || !from) return
      event.preventDefault()
      // Walk in that direction to the next real day, skipping padding.
      let [x, y] = from
      for (;;) {
        x += move[0]
        y += move[1]
        const column = columns[x]
        if (!column || y < 0 || y >= column.length) return
        const cell = column[y]
        if (!cell.empty) {
          event.currentTarget
            .querySelector<HTMLElement>(`[data-cell="${cell.key}"]`)
            ?.focus()
          return
        }
      }
    },
    [columns, onCellClick, position],
  )

  // Fixed coordinates go stale when anything scrolls; drop the tooltip.
  useEffect(() => {
    if (!hover) return
    const hide = () => setHover(null)
    window.addEventListener('scroll', hide, { capture: true, passive: true })
    return () => window.removeEventListener('scroll', hide, { capture: true })
  }, [hover])

  const onClick = useCallback(
    (event: MouseEvent) => {
      const target = (event.target as HTMLElement).closest<HTMLElement>(
        '[data-cell]',
      )
      if (target?.dataset.cell && onCellClick) onCellClick(target.dataset.cell)
    },
    [onCellClick],
  )

  return (
    <div className={className}>
      <div className="flex" style={{ gap }}>
        {rowLabels && (
          <div
            className="flex flex-col pr-1.5 text-[11px] text-muted-foreground"
            style={{ gap }}
          >
            {rowLabels.map((label, index) => (
              <span
                key={index}
                className="flex items-center leading-none"
                style={{ height: cellSize }}
              >
                {label}
              </span>
            ))}
          </div>
        )}
        <div
          role="group"
          aria-label={label}
          className="flex"
          style={{ gap }}
          onMouseMove={onMove}
          onMouseLeave={() => setHover(null)}
          onClick={onClick}
          onFocus={onFocus}
          onBlur={() => setHover(null)}
          onKeyDown={onKeyDown}
        >
          {columns.map((column, columnIndex) => (
            <div key={columnIndex} className="flex flex-col" style={{ gap }}>
              {column.map((cell) =>
                cell.empty ? (
                  <span
                    key={cell.key}
                    style={{ width: cellSize, height: cellSize }}
                  />
                ) : (
                  <span
                    key={cell.key}
                    data-cell={cell.key}
                    data-label={cell.label}
                    role={onCellClick ? 'button' : 'img'}
                    aria-label={cell.label}
                    tabIndex={cell.key === tabStop ? 0 : -1}
                    className={cn(
                      'rounded-[3px] transition-transform duration-100 outline-none hover:scale-125 focus-visible:scale-125 focus-visible:ring-2 focus-visible:ring-ring',
                      levelClass[cell.level],
                      onCellClick && 'cursor-pointer',
                    )}
                    style={{ width: cellSize, height: cellSize }}
                  />
                ),
              )}
            </div>
          ))}
        </div>
      </div>
      {columnLabels && (
        <div
          className="mt-1.5 flex font-mono text-[11px] text-muted-foreground"
          style={{ gap, paddingLeft: rowLabels ? undefined : 0 }}
        >
          {rowLabels && (
            <span
              className="pr-1.5"
              style={{ width: 'auto', visibility: 'hidden' }}
            >
              {rowLabels[0]}
            </span>
          )}
          {columnLabels.map((label, index) => (
            <span
              key={index}
              className="overflow-visible whitespace-nowrap"
              style={{ width: cellSize }}
            >
              {label}
            </span>
          ))}
        </div>
      )}
      {hover &&
        createPortal(
          <div
            role="tooltip"
            className={cn(
              'pointer-events-none fixed z-50 -translate-x-1/2 rounded-md border bg-popover px-2 py-1 text-xs whitespace-nowrap text-popover-foreground shadow-float',
              !hover.below && '-translate-y-full',
            )}
            style={{
              left: hover.x,
              top: hover.below ? hover.y + 6 : hover.y - 6,
            }}
          >
            {hover.label}
          </div>,
          document.body,
        )}
    </div>
  )
})

export function HeatLegend({ less, more }: { less: string; more: string }) {
  return (
    <span className="flex items-center gap-1 text-xs text-muted-foreground">
      {less}
      {([0, 1, 2, 3, 4] as const).map((level) => (
        <span
          key={level}
          className={cn('size-2.5 rounded-[2px]', levelClass[level])}
        />
      ))}
      {more}
    </span>
  )
}
