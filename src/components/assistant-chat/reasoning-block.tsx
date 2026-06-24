/**
 * @file reasoning-block.tsx
 * @description Collapsible "thinking" panel for streamed model reasoning chunks.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Surface the model's reasoning as a distinct, muted, collapsible panel (separate lane from
 *   the visible answer), matching the LM Studio / Claude / lobehub "thinking" affordance.
 * - While streaming: auto-expanded with a live "thinking…" pulse so the user sees progress
 *   even before the first answer token. When done: collapsed by default, re-expandable.
 * - Stay fluid under volume: gemma streams hundreds of reasoning chunks; the panel renders one
 *   pre-wrapped text node (no per-chunk DOM), so growth is cheap.
 *
 * ## Not responsible for
 * - Accumulating chunks — `useAiChatStream` does that; this component renders the current text.
 * - The visible answer or tool calls — separate lanes.
 */

import { memo, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { cn } from '@/lib/cn'
import { useStickToBottom } from './use-stick-to-bottom'

export interface ReasoningBlockCopy {
  /** Header label while streaming, e.g. "Thinking…". */
  thinkingLabel: string
  /** Header label once finished, e.g. "Thought process". */
  thoughtLabel: string
  /** aria-label for the expand/collapse toggle. */
  toggleLabel: string
}

export interface ReasoningBlockProps {
  /** Accumulated reasoning text. */
  text: string
  /** True while this turn is still streaming. */
  streaming: boolean
  copy: ReasoningBlockCopy
  testId?: string
}

/**
 * Renders the thinking panel. Open state is auto-driven by streaming (open while thinking,
 * collapsed when done) but the user can override either way.
 */
export const ReasoningBlock = memo(function ReasoningBlock({
  text,
  streaming,
  copy,
  testId,
}: ReasoningBlockProps) {
  const [open, setOpen] = useState(streaming)
  // Auto-collapse once streaming finishes; auto-expand when a new turn starts thinking.
  // The user can still toggle afterwards (the effect only fires on the streaming flip).
  useEffect(() => {
    setOpen(streaming)
  }, [streaming])

  // Stick-to-bottom for the reasoning body: follow the newest thought while live reasoning streams
  // in, but ONLY while the user is parked at the panel's bottom — the instant they scroll up within
  // the panel, following stops so they can read earlier reasoning. A layout effect (pre-paint) keeps
  // the follow jump invisible; bounded by the panel's max-h.
  const bodyRef = useRef<HTMLDivElement | null>(null)
  // `open` is the attach key: the body only renders while expanded, so the scroll listener must
  // re-bind whenever the panel (re)opens behind this stable ref.
  const { stickToBottom, scrollToBottom } = useStickToBottom(bodyRef, open)
  useLayoutEffect(() => {
    if (!streaming || !open || !stickToBottom) return
    scrollToBottom()
  }, [streaming, open, text, stickToBottom, scrollToBottom])

  if (!text) return null

  return (
    <div
      data-testid={testId}
      data-streaming={streaming ? 'true' : 'false'}
      data-open={open ? 'true' : 'false'}
      className={cn(
        'rounded-paper border-border-light bg-paper border',
        'animate-[pk-fade-in_200ms_ease]',
      )}
    >
      <button
        type="button"
        aria-expanded={open}
        aria-label={copy.toggleLabel}
        onClick={() => setOpen((value) => !value)}
        data-testid={testId ? `${testId}-toggle` : undefined}
        className={cn(
          'flex w-full items-center gap-[8px] px-[12px] py-[8px]',
          'text-ink-faint font-mono text-[10px] uppercase tracking-[0.08em]',
          'enabled:hover:text-ink-secondary transition-colors duration-150',
        )}
      >
        <span
          aria-hidden="true"
          className={cn(
            'inline-block h-[6px] w-[6px] rounded-full',
            streaming
              ? 'bg-accent animate-[pk-pulse_1.4s_ease-in-out_infinite]'
              : 'bg-border-strong',
          )}
        />
        <span className="flex-1 text-left">
          {streaming ? copy.thinkingLabel : copy.thoughtLabel}
        </span>
        <span aria-hidden="true" className="text-[12px]">
          {open ? '−' : '+'}
        </span>
      </button>
      {open ? (
        <div
          ref={bodyRef}
          data-testid={testId ? `${testId}-body` : undefined}
          className={cn(
            'border-border-light border-t px-[12px] py-[10px]',
            'text-ink-muted whitespace-pre-wrap font-mono text-[11.5px] leading-[1.55]',
            'max-h-[260px] overflow-y-auto',
          )}
        >
          {text}
        </div>
      ) : null}
    </div>
  )
})
