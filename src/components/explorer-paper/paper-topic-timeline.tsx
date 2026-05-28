/**
 * Topic-over-time strip used inside the Intelligence "Topics" card.
 *
 * Each row is a 3-column grid: topic name with a coloured dot · horizontal
 * track of opacity-varying coloured bars representing intensity at each
 * period in the window · trend-arrowed count on the right. An axis row
 * underneath aligns labels (date markers) to the same track.
 *
 * ## Responsibilities
 * - Render the topic rows with N tinted segments inside the track.
 * - Map `trend` ∈ "up" / "down" / "flat" to ↑ / ↓ / — and to success / error
 *   / muted colours on the count.
 * - Render the axis labels under the rows.
 *
 * ## Not responsible for
 * - Computing topic intensity over time — caller supplies `bars`.
 * - Picking topic colours — caller supplies each topic's `color`.
 */

import { cn } from '@/lib/cn'

export type PaperTopicTrend = 'up' | 'down' | 'flat'

export interface PaperTopicBar {
  /** Track-percent (0..100) where the bar starts. */
  left: number
  /** Track-percent (0..100) wide. */
  width: number
  /** 0..1 opacity. */
  opacity: number
}

export interface PaperTopicRow {
  id: string
  name: string
  color: string
  count: number
  trend: PaperTopicTrend
  bars: readonly PaperTopicBar[]
}

export interface PaperTopicTimelineProps {
  rows: readonly PaperTopicRow[]
  axisLabels?: readonly string[]
  /** Optional left padding for the axis row so it lines up with the track. */
  axisOffsetPx?: number
  className?: string
  testId?: string
}

const TREND_GLYPH: Record<PaperTopicTrend, string> = {
  up: '↑',
  down: '↓',
  flat: '—',
}

export function PaperTopicTimeline({
  rows,
  axisLabels = [],
  axisOffsetPx = 212,
  className,
  testId,
}: PaperTopicTimelineProps) {
  return (
    <div data-testid={testId} className={cn('flex flex-col', className)}>
      {rows.map((row) => (
        <div
          key={row.id}
          data-testid={`paper-topic-${row.id}`}
          className={cn(
            'border-border-light grid grid-cols-[200px_1fr_60px] items-center gap-3',
            'border-b py-2 last:border-b-0',
          )}
        >
          <div className="flex items-center gap-2">
            <span
              aria-hidden="true"
              className="h-2 w-2 shrink-0 rounded-full"
              style={{ background: row.color }}
            />
            <span className="text-ink-secondary truncate font-serif text-[13.5px]">
              {row.name}
            </span>
          </div>
          <div
            aria-hidden="true"
            className="bg-page relative h-[14px] overflow-hidden rounded-[2px]"
          >
            {row.bars.map((bar, index) => (
              <span
                key={index}
                data-testid={`paper-topic-bar-${row.id}-${index}`}
                className="absolute top-0 bottom-0 rounded-[1px]"
                style={{
                  left: `${bar.left}%`,
                  width: `${bar.width}%`,
                  background: row.color,
                  opacity: bar.opacity,
                }}
              />
            ))}
          </div>
          <div
            data-testid={`paper-topic-trend-${row.id}`}
            className={cn(
              'text-right font-mono text-[11px]',
              row.trend === 'up'
                ? 'text-success'
                : row.trend === 'down'
                  ? 'text-error'
                  : 'text-ink-secondary',
            )}
          >
            {TREND_GLYPH[row.trend]} {row.count}
          </div>
        </div>
      ))}
      {axisLabels.length > 0 ? (
        <div
          className="text-ink-faint mt-3 flex justify-between font-mono text-[9.5px]"
          style={{ paddingLeft: axisOffsetPx }}
          data-testid="paper-topic-axis"
        >
          {axisLabels.map((label, index) => (
            <span key={index}>{label}</span>
          ))}
        </div>
      ) : null}
    </div>
  )
}
