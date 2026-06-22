/**
 * A single Assistant chat bubble — user query or AI answer with optional
 * evidence panel.
 *
 * Visual contract follows `pk-assistant.jsx` → SAMPLE_CONVERSATION:
 *
 *   User:   right-aligned bubble with accent-soft fill, serif text, asymmetric radius
 *   AI:     left-aligned flat block (no bubble fill), small byline above
 *           with bot glyph + provider label, optional evidence panel
 *           below with accent-left-border citing source records
 *
 * ## Responsibilities
 * - Render either the user or the AI variant per `role`.
 * - Render arbitrary React content as the bubble body (so the route can
 *   pass markdown-rendered nodes, lists, code blocks, etc.).
 * - Render the evidence panel when `evidence` is non-empty; each evidence
 *   row click routes to onSelectEvidence.
 *
 * ## Not responsible for
 * - Markdown / streaming — caller pre-renders the content tree.
 */

import { type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { PKGlyph } from '@/components/shell/pk-glyph'
import { StarToggle } from '@/components/shell/star-toggle'

export type PaperAssistantRole = 'user' | 'ai'

export interface PaperAssistantEvidence {
  id: string
  /** Mono ISO-ish date label, e.g. "2025-04-05". */
  date: string
  title: string
  domain: string
  url: string
  /**
   * W-STAR star key (canonicalized URL) for this cited page (W-AI-7). When present alongside the
   * star handlers, the row shows a star toggle; absent → no star (e.g. legacy evidence).
   */
  canonicalUrl?: string | null
}

/** i18n copy for the evidence-row star toggle (mirrors `StarToggle`'s a11y contract). */
export interface PaperAssistantEvidenceStarCopy {
  /** aria-label when NOT starred, e.g. "Star this source". */
  starLabel: string
  /** aria-label when starred, e.g. "Unstar this source". */
  unstarLabel: string
  /** Live-region state words announced after a toggle. */
  status: { starred: string; unstarred: string }
}

export interface PaperAssistantMessageProps {
  role: PaperAssistantRole
  /** Mono byline shown above the AI bubble, e.g. "Local · llama 3.2". */
  byline?: string
  /** Bubble body — string or arbitrary rich content (paragraphs, lists). */
  children: ReactNode
  evidence?: readonly PaperAssistantEvidence[]
  evidenceLabel?: string
  onSelectEvidence?: (item: PaperAssistantEvidence) => void
  /**
   * Whether a cited source is starred, keyed by its `canonicalUrl` (the W-STAR key, W-AI-7). Only
   * called for rows that HAVE a canonical url, so the caller receives a guaranteed string. When
   * supplied with `onToggleEvidenceStar` + `evidenceStarCopy`, each starrable row renders a toggle.
   */
  isEvidenceStarred?: (canonicalUrl: string) => boolean
  /** Toggle the star for a cited source by its canonical url (optimistic; caller writes through). */
  onToggleEvidenceStar?: (canonicalUrl: string) => void
  /** i18n copy for the evidence-row star toggle. */
  evidenceStarCopy?: PaperAssistantEvidenceStarCopy
  className?: string
  testId?: string
}

export function PaperAssistantMessage({
  role,
  byline,
  children,
  evidence,
  evidenceLabel,
  onSelectEvidence,
  isEvidenceStarred,
  onToggleEvidenceStar,
  evidenceStarCopy,
  className,
  testId,
}: PaperAssistantMessageProps) {
  const isUser = role === 'user'
  return (
    <div
      data-testid={testId}
      data-role={role}
      className={cn('flex flex-col gap-[6px]', className)}
    >
      {!isUser && byline ? (
        <div className="text-ink-faint flex items-center gap-[6px] font-mono text-[9.5px] uppercase tracking-[0.08em]">
          <PKGlyph icon="smart_toy" size={11} strokeWidth={1.6} />
          <span>{byline}</span>
        </div>
      ) : null}
      <div
        className={cn(
          isUser
            ? cn(
                'self-end max-w-[75%] bg-accent-soft border-accent-medium border',
                'rounded-[14px_14px_4px_14px] px-4 py-3',
                'font-serif text-[15px] tracking-[-0.005em] text-ink leading-[1.4]',
              )
            : cn(
                'self-start max-w-full text-ink',
                'font-serif text-[15px] leading-[1.55] tracking-[-0.005em]',
              ),
        )}
      >
        {children}
      </div>

      {!isUser && evidence && evidence.length > 0 ? (
        <div
          data-testid="paper-assistant-evidence"
          className={cn(
            'mt-[10px] rounded-[0_3px_3px_0] border-l-[2px] border-accent',
            'bg-[color-mix(in_srgb,var(--accent)_4%,var(--bg-paper))]',
            'px-[14px] py-[12px]',
          )}
        >
          {evidenceLabel ? (
            <div className="text-ink-faint mb-2 font-mono text-[9.5px] uppercase tracking-[0.08em]">
              {evidenceLabel.replace('{count}', String(evidence.length))}
            </div>
          ) : null}
          {evidence.map((item) => (
            <div
              key={item.id}
              className="group border-border-light flex items-center gap-[8px] border-b py-[6px] last:border-b-0"
            >
              <button
                type="button"
                onClick={() => onSelectEvidence?.(item)}
                disabled={!onSelectEvidence}
                data-testid={`paper-assistant-evidence-${item.id}`}
                className={cn(
                  'grid min-w-0 flex-1 grid-cols-[80px_1fr] items-center gap-[10px] text-left',
                  'enabled:cursor-pointer enabled:hover:text-accent transition-colors duration-150',
                  'disabled:cursor-default',
                )}
              >
                <span className="text-ink-faint font-mono text-[10.5px]">
                  {item.date}
                </span>
                <span className="text-ink-secondary font-serif text-[13px] leading-[1.3] tracking-[-0.005em]">
                  {item.title}{' '}
                  <span className="text-ink-faint font-mono text-[10.5px]">
                    · {item.domain}
                  </span>
                </span>
              </button>
              <EvidenceStar
                item={item}
                isStarred={isEvidenceStarred}
                onToggle={onToggleEvidenceStar}
                copy={evidenceStarCopy}
              />
            </div>
          ))}
        </div>
      ) : null}
    </div>
  )
}

/**
 * The per-evidence-row star toggle. Renders nothing unless the caller wired BOTH handlers' copy AND
 * the row carries a `canonicalUrl` (the W-STAR key) — a single gate, so the toggle below works on
 * guaranteed-present values (no optional-chaining at the call site). Mirrors `StarToggle`'s a11y.
 */
function EvidenceStar({
  item,
  isStarred,
  onToggle,
  copy,
}: {
  item: PaperAssistantEvidence
  isStarred?: (canonicalUrl: string) => boolean
  onToggle?: (canonicalUrl: string) => void
  copy?: PaperAssistantEvidenceStarCopy
}) {
  const canonicalUrl = item.canonicalUrl
  if (!onToggle || !copy || !canonicalUrl) return null
  return (
    <StarToggle
      starred={isStarred ? isStarred(canonicalUrl) : false}
      onToggle={() => onToggle(canonicalUrl)}
      starLabel={copy.starLabel}
      unstarLabel={copy.unstarLabel}
      statusLabel={copy.status}
      testId={`paper-assistant-evidence-star-${item.id}`}
    />
  )
}
