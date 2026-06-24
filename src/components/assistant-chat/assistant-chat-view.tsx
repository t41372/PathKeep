/**
 * @file assistant-chat-view.tsx
 * @description Composed streaming-chat surface: greeting/empty state, virtualized message list,
 *              and the send/cancel composer — the marquee AI surface.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Empty state: serif greeting + clickable suggested-prompt cards (reuses PaperAssistantGreeting).
 * - Conversation: render each turn via AssistantTurn; virtualize off-screen finalized turns with
 *   `useViewportMount` (height-preserving placeholders) so long chats stay light on the DOM.
 * - Auto-scroll (stick-to-bottom): follow the latest content while streaming ONLY while the user is
 *   parked at the bottom — through the reasoning-only and tool-only phases too, not just visible
 *   answer growth. The instant the user scrolls up they stay put; scrolling back down resumes
 *   following. A new turn (a deliberate send) always pins to the bottom and re-arms following.
 * - Composer: Enter=send / Shift+Enter=newline, send↔stop button states, provider byline,
 *   disabled only when no provider is configured (stays ENABLED while streaming so focus is never
 *   ripped to the body — Enter is a no-op mid-stream and the Stop affordance is shown instead).
 * - A "Connecting to {provider}…" affordance while awaiting the first chunk (cold local models
 *   have multi-second first-token latency), distinct from the in-turn thinking dots.
 *
 * ## Not responsible for
 * - Streaming mechanics — driven by `useAiChatStream`, owned by the route.
 * - Availability gating — the route decides whether to render this view at all.
 *
 * ## Fluidity
 * - Only the streaming (last) turn re-renders frame to frame; finalized turns are `memo`'d and,
 *   when scrolled away, collapse to a placeholder. The composer is its own subtree.
 */

import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
} from 'react'
import { cn } from '@/lib/cn'
import { useStickToBottom } from './use-stick-to-bottom'
import { PKGlyph } from '@/components/shell/pk-glyph'
import {
  PaperAssistantGreeting,
  type PaperAssistantEvidence,
  type PaperAssistantGreetingPrompt,
} from '@/components/explorer-paper'
import { useViewportMount } from '@/pages/explorer/hooks/use-viewport-mount'
import { AssistantTurn, type AssistantTurnCopy } from './assistant-turn'
import type { ChatMessage } from './use-ai-chat-stream'

export interface AssistantChatComposerCopy {
  placeholder: string
  /** aria-label for the send button. */
  sendLabel: string
  /** aria-label for the cancel button (shown while streaming). */
  cancelLabel: string
  /** Mono attribution string (already provider-resolved). */
  attribution: string
  /** Mono key-hint string, e.g. "↵ send · ⇧↵ newline". */
  keyHint: string
  /** Mono "Connecting to {provider}…" affordance shown before the first chunk arrives. */
  connectingLabel: string
  /**
   * Ambient, always-visible scope note (e.g. "Searches your whole archive"). Surfaced in the
   * composer footer so the assistant's scope stays honest through the whole conversation — not
   * just on the empty greeting (ASSIST-3 persistent scope honesty).
   */
  scopeNote: string
}

export interface AssistantChatViewCopy {
  greetingTitle: string
  greetingSubtitle: React.ReactNode
  turn: AssistantTurnCopy
  composer: AssistantChatComposerCopy
}

export interface AssistantChatViewProps {
  messages: readonly ChatMessage[]
  input: string
  streaming: boolean
  /** True after send but before the first chunk arrives — drives the connecting affordance. */
  awaitingFirstChunk?: boolean
  /** False when no LLM provider is configured — disables the composer. */
  canSend: boolean
  prompts?: readonly PaperAssistantGreetingPrompt[]
  copy: AssistantChatViewCopy
  onInputChange: (next: string) => void
  onSend: (text: string) => void
  onCancel: () => void
  /** Re-send the last user prompt; wired onto error/cancelled turns for in-place recovery. */
  onRetry?: () => void
  /** Re-run the assistant on the same question; wired onto the latest completed answer's actions. */
  onRegenerate?: () => void
  onPickPrompt?: (prompt: PaperAssistantGreetingPrompt) => void
  /** Resolve citations for a turn (real agent evidence rows, W-AI-7). */
  evidenceFor?: (
    message: ChatMessage,
  ) => readonly PaperAssistantEvidence[] | undefined
  onSelectEvidence?: (evidence: PaperAssistantEvidence) => void
  /** Whether a cited source is starred, keyed by its `canonicalUrl` (the W-STAR key). */
  isEvidenceStarred?: (canonicalUrl: string) => boolean
  /** Toggle the star for a cited source by its canonical url (optimistic; caller writes through). */
  onToggleEvidenceStar?: (canonicalUrl: string) => void
  /** Disable viewport virtualization (tests / short lists). */
  disableVirtualization?: boolean
  testId?: string
}

/**
 * One message row that recycles its DOM when scrolled out of view. The active (last) turn is
 * never virtualized so streaming content always renders; finished off-screen turns collapse to
 * a height-preserving placeholder.
 */
const ChatRow = memo(function ChatRow({
  message,
  copy,
  evidence,
  onSelectEvidence,
  isEvidenceStarred,
  onToggleEvidenceStar,
  onRetry,
  onRegenerate,
  pinned,
  disableVirtualization,
}: {
  message: ChatMessage
  copy: AssistantTurnCopy
  evidence?: readonly PaperAssistantEvidence[]
  onSelectEvidence?: (evidence: PaperAssistantEvidence) => void
  isEvidenceStarred?: (canonicalUrl: string) => boolean
  onToggleEvidenceStar?: (canonicalUrl: string) => void
  onRetry?: () => void
  onRegenerate?: () => void
  /** When true, never virtualize (active streaming turn). */
  pinned: boolean
  disableVirtualization?: boolean
}) {
  const { ref, inView, measuredHeight } = useViewportMount<HTMLDivElement>({
    skip: pinned || disableVirtualization,
  })
  const shouldRender = pinned || disableVirtualization || inView
  const placeholderStyle =
    !shouldRender && measuredHeight !== null
      ? { minHeight: `${measuredHeight}px` }
      : undefined

  return (
    <div ref={ref} style={placeholderStyle} data-virtualized={!shouldRender}>
      {shouldRender ? (
        <AssistantTurn
          message={message}
          copy={copy}
          evidence={evidence}
          onSelectEvidence={onSelectEvidence}
          isEvidenceStarred={isEvidenceStarred}
          onToggleEvidenceStar={onToggleEvidenceStar}
          onRetry={onRetry}
          onRegenerate={onRegenerate}
        />
      ) : null}
    </div>
  )
})

export function AssistantChatView({
  messages,
  input,
  streaming,
  awaitingFirstChunk = false,
  canSend,
  prompts,
  copy,
  onInputChange,
  onSend,
  onCancel,
  onRetry,
  onRegenerate,
  onPickPrompt,
  evidenceFor,
  onSelectEvidence,
  isEvidenceStarred,
  onToggleEvidenceStar,
  disableVirtualization,
  testId,
}: AssistantChatViewProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null)
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)
  const isEmpty = messages.length === 0
  const lastId = messages.length > 0 ? messages[messages.length - 1].id : null

  // Stick-to-bottom: the scroll listener flips `stickToBottom` false the moment the user scrolls up
  // and back true once they return to the bottom. Auto-follow consults this flag so a user reading
  // earlier output is never yanked down mid-stream.
  const { stickToBottom, scrollToBottom } = useStickToBottom(scrollRef)

  // Pin to the bottom after each NEW turn — a deliberate user send — and re-arm following. A layout
  // effect (pre-paint) keeps the jump invisible; the scroll container always renders, so its ref is
  // set by the time these effects run post-commit. (`messages.length` only grows on a real send /
  // reset, never on a mid-turn streaming flush, so this never fights a scrolled-up reader.)
  useLayoutEffect(() => {
    scrollToBottom()
  }, [messages.length, scrollToBottom])

  // While streaming, keep following the growing turn — but ONLY while the user is parked at the
  // bottom (`stickToBottom`); a user who scrolled up to read is left exactly where they are. The
  // dependency tracks total content + reasoning + tool-call volume so the effect re-runs through the
  // reasoning-only and tool-only phases, not just visible answer growth.
  const followKey = messages.reduce(
    (sum, message) =>
      sum +
      message.content.length +
      (message.reasoning?.length ?? 0) +
      (message.toolCalls?.length ?? 0),
    0,
  )
  useEffect(() => {
    if (!streaming || !stickToBottom) return
    scrollToBottom()
  }, [streaming, followKey, stickToBottom, scrollToBottom])

  // On the streaming → idle edge, return focus to the textarea (it was never disabled, but a
  // click on Stop / a button moves focus). rAF defers to after the send/stop button unmounts.
  const wasStreamingRef = useRef(streaming)
  useEffect(() => {
    if (wasStreamingRef.current && !streaming) {
      requestAnimationFrame(() => textareaRef.current?.focus())
    }
    wasStreamingRef.current = streaming
  }, [streaming])

  const submit = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed || streaming || !canSend) return
    onSend(trimmed)
  }, [canSend, input, onSend, streaming])

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault()
        // While streaming Enter is a deliberate no-op (the hook also no-ops); keeps the textarea
        // focused and editable so the user can queue the next prompt.
        submit()
      }
    },
    [submit],
  )

  // Fill + focus the textarea on a prompt pick, caret parked at the end.
  const handlePickPrompt = useCallback(
    (prompt: PaperAssistantGreetingPrompt) => {
      onPickPrompt?.(prompt)
      requestAnimationFrame(() => {
        const node = textareaRef.current
        if (!node) return
        node.focus()
        const end = node.value.length
        node.setSelectionRange(end, end)
      })
    },
    [onPickPrompt],
  )

  // The composer is disabled ONLY when no provider is configured — never while streaming, so
  // focus stays put. Sending is blocked while streaming via `submit`/`sendDisabled` instead.
  const sendDisabled = streaming || !canSend || input.trim().length === 0

  return (
    <section
      data-testid={testId}
      className="mx-auto flex h-full min-h-0 w-full max-w-[780px] flex-col"
    >
      {/* `min-h-0` is load-bearing: a `flex-1` item defaults to `min-height:auto`,
          which refuses to shrink below its content's intrinsic height — so a long
          conversation would push the composer down and overflow the section instead
          of scrolling here. With `min-h-0` this region shrinks to the fixed-height
          box and becomes the SOLE scroll surface; the composer below stays pinned. */}
      <div
        ref={scrollRef}
        data-testid="assistant-chat-messages"
        className="pk-scrollbar flex min-h-0 flex-1 flex-col gap-[22px] overflow-y-auto pt-2 pb-5"
      >
        {isEmpty ? (
          <PaperAssistantGreeting
            title={copy.greetingTitle}
            subtitle={copy.greetingSubtitle}
            prompts={prompts}
            onSelectPrompt={handlePickPrompt}
          />
        ) : (
          messages.map((message) => (
            <ChatRow
              key={message.id}
              message={message}
              copy={copy.turn}
              evidence={evidenceFor?.(message)}
              onSelectEvidence={onSelectEvidence}
              isEvidenceStarred={isEvidenceStarred}
              onToggleEvidenceStar={onToggleEvidenceStar}
              onRetry={message.id === lastId ? onRetry : undefined}
              onRegenerate={message.id === lastId ? onRegenerate : undefined}
              pinned={message.id === lastId}
              disableVirtualization={disableVirtualization}
            />
          ))
        )}
      </div>

      {/* `shrink-0` PINS the composer: it is a flex sibling of the scrolling messages
          region (which owns `flex-1 overflow-y-auto`), so it keeps its intrinsic
          height and never compresses or scrolls away. The messages list is the SOLE
          scroll surface; the composer stays anchored at the bottom in every state. */}
      <form
        data-testid="assistant-chat-composer"
        onSubmit={(event) => {
          event.preventDefault()
          submit()
        }}
        className="border-border-light flex shrink-0 flex-col border-t pb-2 pt-4"
      >
        {awaitingFirstChunk ? (
          <div
            data-testid="assistant-chat-connecting"
            role="status"
            className="text-ink-faint mb-2 flex items-center gap-[6px] font-mono text-[10px]"
          >
            <span
              aria-hidden="true"
              className="pk-typing-dot bg-accent inline-block h-[5px] w-[5px] rounded-full animate-[pk-pulse_1.2s_ease-in-out_infinite]"
            />
            <span>{copy.composer.connectingLabel}</span>
          </div>
        ) : null}
        <div className="flex items-end gap-2">
          <textarea
            ref={textareaRef}
            data-testid="assistant-chat-input"
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={copy.composer.placeholder}
            aria-label={copy.composer.placeholder}
            rows={1}
            disabled={!canSend}
            className={cn(
              'border-border-default bg-card-paper rounded-paper flex-1 resize-none border',
              'min-h-[46px] max-h-[140px] px-[14px] py-[12px]',
              'font-serif text-[15px] leading-[1.4] text-ink',
              'placeholder:text-ink-faint placeholder:italic',
              'focus:border-accent focus:outline-none',
              'transition-colors duration-150',
              'disabled:opacity-60 disabled:cursor-not-allowed',
            )}
          />
          {streaming ? (
            <button
              type="button"
              onClick={onCancel}
              aria-label={copy.composer.cancelLabel}
              title={copy.composer.cancelLabel}
              data-testid="assistant-chat-cancel"
              className={cn(
                'rounded-paper inline-grid h-[46px] w-[46px] place-items-center',
                'border-border-default text-ink-secondary border bg-card-paper',
                'hover:border-accent hover:text-accent transition-colors duration-150',
              )}
            >
              <PKGlyph icon="stop" size={18} strokeWidth={1.8} />
            </button>
          ) : (
            <button
              type="submit"
              disabled={sendDisabled}
              aria-label={copy.composer.sendLabel}
              title={copy.composer.sendLabel}
              data-testid="assistant-chat-send"
              className={cn(
                'rounded-paper inline-grid h-[46px] w-[46px] place-items-center',
                'bg-accent text-paper',
                'enabled:hover:opacity-85 transition-opacity duration-150',
                'disabled:cursor-not-allowed disabled:opacity-40',
              )}
            >
              <PKGlyph icon="arrow_forward" size={18} strokeWidth={1.8} />
            </button>
          )}
        </div>
        <div className="text-ink-faint mt-2 flex items-center justify-between font-mono text-[10px]">
          <span className="flex items-center gap-2">
            <span data-testid="assistant-chat-attribution">
              {copy.composer.attribution}
            </span>
            {/* ASSIST-3: ambient, always-visible scope note so the assistant's whole-archive scope
                stays honest through the entire conversation, not only on the empty greeting. */}
            <span aria-hidden="true">·</span>
            <span data-testid="assistant-chat-scope-note">
              {copy.composer.scopeNote}
            </span>
          </span>
          <span>{copy.composer.keyHint}</span>
        </div>
      </form>
    </section>
  )
}
