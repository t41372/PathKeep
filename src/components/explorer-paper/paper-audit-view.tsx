/**
 * PaperAuditView — composed Audit ledger shell.
 *
 * Layout mirrors `pk-audit.jsx` AuditView: manifest chain card → 2-column
 * grid (recent runs + storage breakdown left; export + snapshots right).
 * Each card body is a slot so the route can render its own data-heavy
 * content (runs table, snapshot list, action buttons) without dragging
 * those into a presentation primitive.
 *
 * ## Responsibilities
 * - Render the manifest-chain card with the chain blocks + an "earlier ↺"
 *   placeholder + the verification callout.
 * - Provide slot props for the runs table, storage breakdown, export
 *   panel, snapshot list, and quiet footer.
 *
 * ## Not responsible for
 * - The data tables / lists themselves — those live in the route and
 *   render inside the slots.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { PaperChainBlock, type PaperChainBlockProps } from './paper-chain-block'

export type PaperAuditChainEntry = Pick<
  PaperChainBlockProps,
  'id' | 'hash' | 'type' | 'when' | 'current'
>

export interface PaperAuditViewCopy {
  manifestTitle: string
  manifestBadge?: ReactNode
  /** Italic-serif callout below the chain blocks (caller-formatted). */
  manifestCallout?: ReactNode
  earlierBlockLabel?: string
  recentRunsTitle: string
  recentRunsBadge?: ReactNode
  storageTitle: string
  storageBadge?: ReactNode
  exportTitle: string
  snapshotsTitle: string
  snapshotsBadge?: ReactNode
  /** Italic-serif quiet footer line. */
  footer?: ReactNode
}

export interface PaperAuditViewProps {
  chain: readonly PaperAuditChainEntry[]
  onSelectChainBlock?: (id: string) => void
  recentRunsSlot: ReactNode
  storageBreakdownSlot: ReactNode
  exportPanelSlot: ReactNode
  snapshotsSlot: ReactNode
  copy: PaperAuditViewCopy
  className?: string
  testId?: string
}

export function PaperAuditView({
  chain,
  onSelectChainBlock,
  recentRunsSlot,
  storageBreakdownSlot,
  exportPanelSlot,
  snapshotsSlot,
  copy,
  className,
  testId,
}: PaperAuditViewProps) {
  return (
    <section
      data-testid={testId}
      className={cn('flex w-full flex-col', className)}
    >
      <PaperCard className="mb-5">
        <PaperCardHeader
          title={copy.manifestTitle}
          right={copy.manifestBadge ?? undefined}
        />
        <PaperCardBody className="pb-2">
          <div
            data-testid="paper-audit-chain"
            className="flex items-center gap-0 overflow-x-auto py-2 pb-5"
          >
            {chain.map((entry, index) => (
              <div
                key={entry.id}
                className="flex shrink-0 items-center"
                data-chain-position={index}
              >
                <PaperChainBlock
                  id={entry.id}
                  hash={entry.hash}
                  type={entry.type}
                  when={entry.when}
                  current={entry.current}
                  onClick={
                    onSelectChainBlock
                      ? () => onSelectChainBlock(entry.id)
                      : undefined
                  }
                />
                {index < chain.length - 1 ? (
                  <div
                    aria-hidden="true"
                    className="bg-border-default mx-[-1px] h-px w-[20px]"
                  />
                ) : null}
              </div>
            ))}
            {copy.earlierBlockLabel ? (
              <>
                <div
                  aria-hidden="true"
                  className="bg-border-default mx-[-1px] h-px w-[20px]"
                />
                <div className="border-border-default text-ink-faint rounded-paper min-w-[100px] border border-dashed px-3 py-2 text-center font-mono text-[10px]">
                  {copy.earlierBlockLabel}
                </div>
              </>
            ) : null}
          </div>

          {copy.manifestCallout ? (
            <div
              data-testid="paper-audit-callout"
              className="border-success-soft text-ink-secondary mt-3 rounded-[0_3px_3px_0] border-l-2 border-success bg-[color-mix(in_srgb,var(--success)_8%,var(--bg-paper))] px-4 py-[14px] font-serif text-[13.5px] italic leading-[1.5]"
            >
              {copy.manifestCallout}
            </div>
          ) : null}
        </PaperCardBody>
      </PaperCard>

      <div className="grid gap-5 lg:grid-cols-[1fr_340px]">
        <div className="flex flex-col gap-5">
          <PaperCard>
            <PaperCardHeader
              title={copy.recentRunsTitle}
              right={
                copy.recentRunsBadge ? (
                  <PaperCardBadge>{copy.recentRunsBadge}</PaperCardBadge>
                ) : undefined
              }
            />
            <PaperCardBody padded={false} className="px-0 pb-1 pt-2">
              <div data-testid="paper-audit-runs">{recentRunsSlot}</div>
            </PaperCardBody>
          </PaperCard>

          <PaperCard>
            <PaperCardHeader
              title={copy.storageTitle}
              right={
                copy.storageBadge ? (
                  <PaperCardBadge>{copy.storageBadge}</PaperCardBadge>
                ) : undefined
              }
            />
            <PaperCardBody>
              <div data-testid="paper-audit-storage">
                {storageBreakdownSlot}
              </div>
            </PaperCardBody>
          </PaperCard>
        </div>

        <div className="flex flex-col gap-5">
          <PaperCard>
            <PaperCardHeader title={copy.exportTitle} />
            <PaperCardBody>
              <div data-testid="paper-audit-export">{exportPanelSlot}</div>
            </PaperCardBody>
          </PaperCard>

          <PaperCard>
            <PaperCardHeader
              title={copy.snapshotsTitle}
              right={
                copy.snapshotsBadge ? (
                  <PaperCardBadge>{copy.snapshotsBadge}</PaperCardBadge>
                ) : undefined
              }
            />
            <PaperCardBody padded={false} className="px-0">
              <div data-testid="paper-audit-snapshots">{snapshotsSlot}</div>
            </PaperCardBody>
          </PaperCard>

          {copy.footer ? (
            <div
              data-testid="paper-audit-footer"
              className="text-ink-faint text-center font-serif text-[13px] italic leading-[1.5]"
            >
              {copy.footer}
            </div>
          ) : null}
        </div>
      </div>
    </section>
  )
}
