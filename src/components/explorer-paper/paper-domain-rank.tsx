/**
 * Top-domains rank list used in the Intelligence view.
 *
 * Each row is a 4-column grid: mono rank · mono domain · accent-tinted
 * progress bar · mono count. Clicking a row routes the consumer to a
 * domain drill-in (insights or browse-filtered-by-domain).
 *
 * ## Responsibilities
 * - Render the rank list with a deterministic bar width scaled to the
 *   peak row's count.
 * - Surface row clicks via onSelectDomain.
 *
 * ## Not responsible for
 * - Sorting — caller passes rows in display order.
 */

import { cn } from '@/lib/cn'

export interface PaperDomainRankRow {
  domain: string
  count: number
}

export interface PaperDomainRankListProps {
  rows: readonly PaperDomainRankRow[]
  onSelectDomain?: (domain: string) => void
  className?: string
  testId?: string
}

export function PaperDomainRankList({
  rows,
  onSelectDomain,
  className,
  testId,
}: PaperDomainRankListProps) {
  const peak = rows.reduce((acc, row) => Math.max(acc, row.count), 1)

  return (
    <div data-testid={testId} className={cn('flex flex-col', className)}>
      {rows.map((row, index) => (
        <button
          type="button"
          key={row.domain}
          onClick={() => onSelectDomain?.(row.domain)}
          disabled={!onSelectDomain}
          data-testid={`paper-domain-rank-${row.domain}`}
          className={cn(
            'border-border-light grid grid-cols-[20px_1fr_60px_40px] items-center gap-[10px]',
            'border-b py-[6px] last:border-b-0',
            'group text-left',
            'enabled:cursor-pointer disabled:cursor-default',
          )}
        >
          <span className="text-ink-faint font-mono text-[10px]">
            {String(index + 1).padStart(2, '0')}
          </span>
          <span
            className={cn(
              'text-ink-secondary truncate font-mono text-[12px]',
              'group-enabled:group-hover:text-accent transition-colors duration-150',
            )}
          >
            {row.domain}
          </span>
          <span
            aria-hidden="true"
            className="bg-page block h-[6px] overflow-hidden rounded-[1px]"
          >
            <span
              className="bg-accent block h-full opacity-65"
              style={{ width: `${(row.count / peak) * 100}%` }}
            />
          </span>
          <span className="text-ink-secondary text-right font-mono text-[11px]">
            {row.count.toLocaleString()}
          </span>
        </button>
      ))}
    </div>
  )
}
