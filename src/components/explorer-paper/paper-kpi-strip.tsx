/**
 * Four-cell KPI strip used at the top of the Intelligence view.
 *
 * The layout (`pk-tokens.css` → `.intel-kpis`) is a 4-column grid with
 * 1 px ink-faint hairline gaps and a thin border around the whole strip.
 * Each cell shows a mono uppercase label, a big serif number, and a mono
 * subtitle.
 *
 * ## Responsibilities
 * - Render N (default 4) KPI cells in a single horizontal grid.
 * - Distinguish "serif numeric" and "mono identifier" variants so cells
 *   like "Top domain · github.com" render correctly alongside cells like
 *   "Pages · 1,247".
 *
 * ## Not responsible for
 * - Computing the numbers — caller passes them ready.
 * - Trend arrows / colours — caller bakes those into the `sub` string.
 */

import { cn } from '@/lib/cn'

export interface PaperKpiCell {
  id: string
  label: string
  value: string
  /** Sub line beneath the value (mono, e.g. "↑ 14% vs last week"). */
  sub?: string
  /** When true, render the value in mono instead of serif (for identifier-like KPIs). */
  monoValue?: boolean
  /** Optional secondary mono token — appears after the main value, dimmed. */
  monoTail?: string
}

export interface PaperKpiStripProps {
  cells: readonly PaperKpiCell[]
  className?: string
  testId?: string
}

export function PaperKpiStrip({
  cells,
  className,
  testId,
}: PaperKpiStripProps) {
  return (
    <div
      data-testid={testId}
      className={cn(
        'rounded-paper border-border-light bg-border-light overflow-hidden border',
        'mb-6 grid gap-px',
        cells.length > 0 &&
          `grid-cols-1 sm:grid-cols-2 lg:grid-cols-[repeat(${cells.length},minmax(0,1fr))]`,
        className,
      )}
      style={{
        // Cap inline so Tailwind's safelist isn't required for arbitrary counts.
        gridTemplateColumns: undefined,
      }}
    >
      {cells.map((cell) => (
        <div
          key={cell.id}
          data-testid={`paper-kpi-${cell.id}`}
          className="bg-card-paper px-[18px] py-[16px]"
        >
          <div className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {cell.label}
          </div>
          <div
            className={cn(
              'text-ink mt-1 leading-[1.1] tracking-[-0.02em]',
              cell.monoValue
                ? 'font-mono text-[18px] tracking-[0]'
                : 'font-serif text-[26px] font-normal',
            )}
          >
            {cell.value}
            {cell.monoTail ? (
              <span className="text-ink-faint ml-1 font-mono text-[18px]">
                {cell.monoTail}
              </span>
            ) : null}
          </div>
          {cell.sub ? (
            <div className="text-ink-faint mt-[2px] font-mono text-[10px]">
              {cell.sub}
            </div>
          ) : null}
        </div>
      ))}
    </div>
  )
}
