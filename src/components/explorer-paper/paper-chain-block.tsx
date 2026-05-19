/**
 * Single block in the manifest-chain visualization used on the paper
 * Audit Ledger view. Each block carries an id (#1847), a short hash
 * (0a4c…ef82), a type label (BACKUP / IMPORT), and a "when" string.
 *
 * Click routes the consumer to the run-detail drill-in.
 */

import { cn } from '@/lib/cn'

export interface PaperChainBlockProps {
  id: string
  hash: string
  /** Free-form free-text classifier, e.g. "BACKUP", "IMPORT". */
  type?: string
  /** Free-form "when" label, e.g. "2h ago" / "May 17 · 14:23". */
  when?: string
  current?: boolean
  onClick?: () => void
  className?: string
  testId?: string
}

export function PaperChainBlock({
  id,
  hash,
  type,
  when,
  current = false,
  onClick,
  className,
  testId,
}: PaperChainBlockProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!onClick}
      data-testid={testId ?? `paper-chain-block-${id}`}
      data-current={current ? 'true' : undefined}
      className={cn(
        'rounded-paper bg-paper flex min-w-[120px] flex-col items-center gap-1 border px-3 py-[10px]',
        'transition-colors duration-150',
        current ? 'border-accent bg-accent-soft' : 'border-border-default',
        'enabled:cursor-pointer enabled:hover:border-accent disabled:cursor-default',
        className,
      )}
    >
      <div className="text-ink font-mono text-[11px] font-semibold">{id}</div>
      <div className="text-ink-faint font-mono text-[9px] tracking-[0.02em]">
        {hash}
      </div>
      {(type || when) && (
        <div className="text-ink-faint font-mono text-[9px]">
          {type ? <span>{type}</span> : null}
          {type && when ? <span> · </span> : null}
          {when ? <span>{when}</span> : null}
        </div>
      )}
    </button>
  )
}
