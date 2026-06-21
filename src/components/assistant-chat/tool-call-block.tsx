/**
 * @file tool-call-block.tsx
 * @description Visible timeline of the tool calls a turn requested (transparency contract).
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Render each `toolCall` chunk as a step the user can see: tool name + arguments in mono,
 *   in request order. This is the trust/transparency surface — the user always sees what the
 *   assistant asked to run.
 * - Stay quiet when no tool was called (most turns), so the answer stays the focus.
 *
 * ## Not responsible for
 * - Tool RESULTS / citations — those arrive with the agent in W-AI-7. The evidence/citation
 *   panel is scaffolded separately via the `PaperAssistantMessage` evidence atoms; this block
 *   is request-side only.
 * - Executing tools — the worker does that.
 */

import { memo } from 'react'
import { cn } from '@/lib/cn'
import type { AssistantToolCall } from './use-ai-chat-stream'

export interface ToolCallBlockCopy {
  /** Section label, e.g. "Tools used". */
  label: string
  /** Per-step "ran {name}" template with `{name}` placeholder. */
  ranTemplate: string
}

export interface ToolCallBlockProps {
  calls: readonly AssistantToolCall[]
  copy: ToolCallBlockCopy
  testId?: string
}

/** Renders the tool-use timeline; renders nothing when there are no calls. */
export const ToolCallBlock = memo(function ToolCallBlock({
  calls,
  copy,
  testId,
}: ToolCallBlockProps) {
  if (calls.length === 0) return null

  return (
    <div
      data-testid={testId}
      className={cn(
        'rounded-paper border-border-light bg-paper border',
        'animate-[pk-fade-in_200ms_ease]',
        'px-[12px] py-[10px]',
      )}
    >
      <div className="text-ink-faint mb-[8px] font-mono text-[10px] uppercase tracking-[0.08em]">
        {copy.label}
      </div>
      <ol className="flex flex-col gap-[8px]">
        {calls.map((call, index) => (
          <li
            key={call.id}
            data-testid={testId ? `${testId}-step-${index}` : undefined}
            className="grid grid-cols-[16px_1fr] gap-[8px]"
          >
            <span
              aria-hidden="true"
              className="text-accent mt-[1px] font-mono text-[11px]"
            >
              {index + 1}.
            </span>
            <div className="min-w-0">
              <div className="text-ink-secondary font-mono text-[11.5px]">
                {copy.ranTemplate.replace('{name}', call.name)}
              </div>
              {call.arguments ? (
                <pre
                  data-testid={testId ? `${testId}-args-${index}` : undefined}
                  className={cn(
                    'text-ink-muted mt-[4px] overflow-x-auto whitespace-pre-wrap',
                    'bg-card-paper rounded-[2px] px-[8px] py-[6px]',
                    'font-mono text-[10.5px] leading-[1.5]',
                  )}
                >
                  {call.arguments}
                </pre>
              ) : null}
            </div>
          </li>
        ))}
      </ol>
    </div>
  )
})
