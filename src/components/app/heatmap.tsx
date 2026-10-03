/**
 * Tile heatmap in the prototype's orange scale, used for the year calendar
 * and the weekly rhythm. EvilCharts has no heatmap, and the amicro one is a
 * fixed-data dither demo, so this is a small grid of our own.
 *
 * One tooltip is shared by the whole grid (event delegation on the container)
 * instead of one per cell: a year view has 371 cells.
 */
import { memo, useCallback, useRef, useState, type MouseEvent } from 'react'
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
  className?: string
}

export const Heatmap = memo(function Heatmap({
  columns,
  cellSize = 13,
  gap = 3,
  rowLabels,
  columnLabels,
  onCellClick,
  className,
}: HeatmapProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [hover, setHover] = useState<{
    label: string
    x: number
    y: number
  } | null>(null)

  const onMove = useCallback((event: MouseEvent) => {
    const target = (event.target as HTMLElement).closest<HTMLElement>(
      '[data-cell]',
    )
    const container = containerRef.current
    if (!target || !container || !target.dataset.label) {
      setHover(null)
      return
    }
    const box = target.getBoundingClientRect()
    const origin = container.getBoundingClientRect()
    setHover({
      label: target.dataset.label,
      x: box.left - origin.left + box.width / 2,
      y: box.top - origin.top,
    })
  }, [])

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
    <div className={cn('relative', className)} ref={containerRef}>
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
          className="flex"
          style={{ gap }}
          onMouseMove={onMove}
          onMouseLeave={() => setHover(null)}
          onClick={onClick}
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
                    className={cn(
                      'rounded-[3px] transition-transform duration-100 hover:scale-125',
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
      {hover && (
        <div
          role="tooltip"
          className="pointer-events-none absolute z-10 -translate-x-1/2 -translate-y-full rounded-md border bg-popover px-2 py-1 text-xs whitespace-nowrap text-popover-foreground shadow-float"
          style={{ left: hover.x, top: hover.y - 6 }}
        >
          {hover.label}
        </div>
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
