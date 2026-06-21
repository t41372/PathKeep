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

export type PaperAssistantRole = 'user' | 'ai'

export interface PaperAssistantEvidence {
  id: string
  /** Mono ISO-ish date label, e.g. "2025-04-05". */
  date: string
  title: string
  domain: string
  url: string
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
            <button
              type="button"
              key={item.id}
              onClick={() => onSelectEvidence?.(item)}
              disabled={!onSelectEvidence}
              data-testid={`paper-assistant-evidence-${item.id}`}
              className={cn(
                'border-border-light grid w-full grid-cols-[80px_1fr] items-center gap-[10px]',
                'border-b py-[6px] last:border-b-0 text-left',
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
          ))}
        </div>
      ) : null}
    </div>
  )
}
