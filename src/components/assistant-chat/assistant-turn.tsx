/**
 * @file assistant-turn.tsx
 * @description Renders one chat message (user prompt or assistant turn) in the paper aesthetic.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - User turn: reuse the paper user bubble (right-aligned, accent-soft, serif).
 * - Assistant turn: provider byline → reasoning panel → tool-use timeline → streaming markdown
 *   answer → cancelled/empty/error affordances → evidence/citation panel, in that order.
 * - Drive a PERSISTENT visually-hidden `aria-live="polite"` status line that announces COARSE
 *   milestones only (Thinking / Using tool / Answering / Answer complete / Stopped / error) so a
 *   screen reader hears progress without per-token spam, and never reads the streaming prose.
 * - Show a typing indicator before the first chunk and a per-turn cancelled/error/empty state.
 *
 * ## Not responsible for
 * - Streaming mechanics — `useAiChatStream` owns the buffer/flush; this is pure presentation.
 * - Conversation layout / scroll — `AssistantChatView` owns that.
 *
 * ## Accessibility
 * - The visible streaming markdown is OUT of the live region; the live region carries one short
 *   milestone string at a time (`aria-atomic="false"`), so SR users get coarse progress, not a
 *   re-read of the whole answer on every flush.
 *
 * ## Performance
 * - `memo`'d so static (finalized) turns never re-render while a later turn streams; only the
 *   active turn's props change frame to frame.
 */

import { memo } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperAssistantMessage,
  type PaperAssistantEvidence,
} from '@/components/explorer-paper'
import { ReasoningBlock, type ReasoningBlockCopy } from './reasoning-block'
import { ToolCallBlock, type ToolCallBlockCopy } from './tool-call-block'
import { StreamingMarkdown } from './streaming-markdown'
import type { ChatMessage } from './use-ai-chat-stream'

export interface AssistantTurnCopy {
  /** Mono byline for the assistant, already provider-resolved. */
  assistantByline: string
  /** Mono byline for the user bubble, e.g. "You". */
  userByline: string
  /** Typing-indicator aria-label shown before the first chunk. */
  typingLabel: string
  /** Mono evidence-panel label with `{count}` placeholder. */
  evidenceLabel: string
  /** Generic fallback shown when an error turn carries no specific message. */
  errorGeneric: string
  /** "Generation stopped" affordance shown below a cancelled turn's partial answer. */
  stoppedLabel: string
  /** "Try again" action label (re-sends the last user prompt). */
  retryLabel: string
  /** Fallback shown when a finished turn returned no answer/reasoning/tools. */
  noAnswerLabel: string
  /** Coarse live-region milestone: "Using tool: {name}". */
  statusUsingTool: string
  /** Coarse live-region milestone: "Answering…". */
  statusAnswering: string
  /** Coarse live-region milestone: "Answer complete". */
  statusComplete: string
  reasoning: ReasoningBlockCopy
  toolCalls: ToolCallBlockCopy
}

export interface AssistantTurnProps {
  message: ChatMessage
  copy: AssistantTurnCopy
  /** Citations for this turn (scaffolded; real ones land with the agent in W-AI-7). */
  evidence?: readonly PaperAssistantEvidence[]
  onSelectEvidence?: (evidence: PaperAssistantEvidence) => void
  /** Re-send the last user prompt; wired on error/cancelled turns for in-place recovery. */
  onRetry?: () => void
}

/** A three-dot typing indicator (animated; honors prefers-reduced-motion via paper.css). */
function TypingIndicator({ label }: { label: string }) {
  return (
    <div
      data-testid="assistant-typing-indicator"
      role="status"
      aria-label={label}
      className="flex items-center gap-[5px] py-[2px]"
    >
      {[0, 1, 2].map((dot) => (
        <span
          key={dot}
          aria-hidden="true"
          style={{ animationDelay: `${dot * 160}ms` }}
          className="pk-typing-dot bg-ink-faint inline-block h-[6px] w-[6px] rounded-full animate-[pk-pulse_1.2s_ease-in-out_infinite]"
        />
      ))}
      {/* Visible only under prefers-reduced-motion (the dots freeze otherwise). */}
      <span
        aria-hidden="true"
        className="pk-typing-fallback text-ink-faint font-mono text-[11px]"
      >
        {label}
      </span>
    </div>
  )
}

/**
 * Resolve the single coarse milestone string for the persistent live region. Returns null while
 * nothing meaningful has happened yet (no announcement → no SR noise). Coarse by design: the
 * streaming prose itself is never announced.
 */
function liveMilestone(
  message: ChatMessage,
  copy: AssistantTurnCopy,
): string | null {
  const reasoning = message.reasoning ?? ''
  const toolCalls = message.toolCalls ?? []
  const hasAnswer = message.content.length > 0

  switch (message.status) {
    case 'error':
      return message.error || copy.errorGeneric
    case 'cancelled':
      return copy.stoppedLabel
    case 'done':
      return hasAnswer || reasoning.length > 0 || toolCalls.length > 0
        ? copy.statusComplete
        : copy.noAnswerLabel
    case 'streaming':
      if (hasAnswer) return copy.statusAnswering
      if (toolCalls.length > 0) {
        const last = toolCalls[toolCalls.length - 1]
        return copy.statusUsingTool.replace('{name}', last.name)
      }
      if (reasoning.length > 0) return copy.reasoning.thinkingLabel
      return copy.typingLabel
    default:
      return null
  }
}

export const AssistantTurn = memo(function AssistantTurn({
  message,
  copy,
  evidence,
  onSelectEvidence,
  onRetry,
}: AssistantTurnProps) {
  if (message.role === 'user') {
    return (
      <PaperAssistantMessage
        role="user"
        byline={copy.userByline}
        testId={`assistant-turn-${message.id}`}
      >
        {message.content}
      </PaperAssistantMessage>
    )
  }

  const isStreaming = message.status === 'streaming'
  const reasoning = message.reasoning ?? ''
  const toolCalls = message.toolCalls ?? []
  const hasReasoning = reasoning.length > 0
  const hasTools = toolCalls.length > 0
  const hasAnswer = message.content.length > 0
  const isError = message.status === 'error'
  const isCancelled = message.status === 'cancelled'
  const isDone = message.status === 'done'
  // Show the typing indicator only before anything has arrived for this turn.
  const showTyping = isStreaming && !hasReasoning && !hasTools && !hasAnswer
  // A finished turn that produced nothing at all gets an explicit fallback, not a blank bubble.
  const showNoAnswer = isDone && !hasAnswer && !hasReasoning && !hasTools
  const milestone = liveMilestone(message, copy)

  return (
    <PaperAssistantMessage
      role="ai"
      byline={copy.assistantByline}
      evidence={evidence}
      evidenceLabel={copy.evidenceLabel}
      onSelectEvidence={onSelectEvidence}
      testId={`assistant-turn-${message.id}`}
    >
      <div className="flex flex-col gap-[10px]">
        {/* Persistent coarse-milestone live region. Always mounted; never wraps the prose. */}
        <span
          data-testid={`assistant-live-${message.id}`}
          role="status"
          aria-live="polite"
          aria-atomic="false"
          className="sr-only"
        >
          {milestone}
        </span>

        {hasReasoning ? (
          <ReasoningBlock
            text={reasoning}
            streaming={isStreaming}
            copy={copy.reasoning}
            testId={`assistant-reasoning-${message.id}`}
          />
        ) : null}

        {hasTools ? (
          <ToolCallBlock
            calls={toolCalls}
            copy={copy.toolCalls}
            testId={`assistant-tools-${message.id}`}
          />
        ) : null}

        {showTyping ? <TypingIndicator label={copy.typingLabel} /> : null}

        {hasAnswer ? (
          <StreamingMarkdown
            content={message.content}
            streaming={isStreaming}
            showCaret={isStreaming}
            testId={`assistant-answer-${message.id}`}
          />
        ) : null}

        {showNoAnswer ? (
          <div
            data-testid={`assistant-no-answer-${message.id}`}
            className="text-ink-faint font-mono text-[11.5px] leading-[1.5]"
          >
            {copy.noAnswerLabel}
          </div>
        ) : null}

        {isCancelled ? (
          <div className="flex items-center gap-[10px]">
            <span
              data-testid={`assistant-stopped-${message.id}`}
              className="text-ink-faint font-mono text-[11px]"
            >
              {copy.stoppedLabel}
            </span>
            {onRetry ? (
              <button
                type="button"
                onClick={onRetry}
                data-testid={`assistant-retry-${message.id}`}
                className={cn(
                  'text-ink-secondary font-mono text-[11px] underline-offset-2',
                  'hover:text-accent hover:underline transition-colors duration-150',
                )}
              >
                {copy.retryLabel}
              </button>
            ) : null}
          </div>
        ) : null}

        {isError ? (
          <div
            data-testid={`assistant-error-${message.id}`}
            role="alert"
            className={cn(
              'rounded-paper border-l-[2px] border-[color:var(--error)]',
              'bg-[color-mix(in_srgb,var(--error)_8%,var(--bg-paper))]',
              'text-ink-secondary flex flex-col gap-[6px] px-[12px] py-[8px]',
              'font-mono text-[11.5px] leading-[1.5]',
            )}
          >
            <span>{message.error || copy.errorGeneric}</span>
            {onRetry ? (
              <button
                type="button"
                onClick={onRetry}
                data-testid={`assistant-retry-${message.id}`}
                className={cn(
                  'self-start text-[11px] underline-offset-2',
                  'hover:text-accent hover:underline transition-colors duration-150',
                )}
              >
                {copy.retryLabel}
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
    </PaperAssistantMessage>
  )
})
