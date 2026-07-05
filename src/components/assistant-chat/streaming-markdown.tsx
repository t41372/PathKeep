/**
 * @file streaming-markdown.tsx
 * @description Paper-styled wrapper around `streamdown` for rendering assistant answers as
 *              they stream in, token by token.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Render partial / unterminated markdown gracefully while a turn streams (streamdown's core
 *   competency: it closes dangling code fences, bold, list items, etc. mid-stream).
 * - Keep the rendered prose in the paper aesthetic (serif body, mono code, accent links).
 * - Append a blinking caret at the tail while streaming so a stalled stream never reads as a
 *   finished answer.
 *
 * ## Not responsible for
 * - Owning the streaming buffer or scheduling re-renders — `useAiChatStream` flushes the
 *   accumulated text at ≤60fps and hands the current string down as `content`.
 * - Heavy renderers: math (KaTeX), diagrams (mermaid), and shiki syntax highlighting are NOT
 *   wired, so the heavy renderer engines don't ship in the startup bundle. Code fences render as
 *   styled `<pre>` — readable and lean. (Wiring those engines is an opt-in W-AI-9 size-audit
 *   item; the assistant route itself is lazy-split so streamdown stays off the startup path.)
 *
 * ## Why streamdown
 * Vetted in `docs/plan/program/ai-redesign-2026/04-current-state-and-execution.md` §6
 * (Apache-2.0, Vercel). Purpose-built for token-by-token AI streaming; without it, a naive
 * markdown renderer flickers on every partial token and mis-renders half-written syntax.
 */

import { memo } from 'react'
import { Streamdown } from 'streamdown'
import { cn } from '@/lib/cn'

export interface StreamingMarkdownProps {
  /** The accumulated answer text so far (may be partial / unterminated markdown). */
  content: string
  /** True while this turn is still streaming — disables in-block animation re-runs. */
  streaming?: boolean
  /** Append the blinking tail caret. Distinct from `streaming` so callers stay explicit. */
  showCaret?: boolean
  className?: string
  testId?: string
}

/**
 * Paper-skinned markdown surface. `mode` switches streamdown between its incremental
 * ("streaming") and final ("static") parse strategies so finished turns parse once.
 */
export const StreamingMarkdown = memo(function StreamingMarkdown({
  content,
  streaming = false,
  showCaret = false,
  className,
  testId,
}: StreamingMarkdownProps) {
  return (
    <div
      data-testid={testId}
      data-streaming={streaming ? 'true' : 'false'}
      className={cn('assistant-prose', className)}
    >
      <Streamdown
        mode={streaming ? 'streaming' : 'static'}
        parseIncompleteMarkdown
        // No shiki/katex/mermaid plugins: keep the bundle lean (see file header).
        controls={false}
      >
        {content}
      </Streamdown>
      {showCaret ? (
        <span
          data-testid={testId ? `${testId}-caret` : undefined}
          aria-hidden="true"
          className={cn(
            'pk-stream-caret bg-ink-faint ml-[1px] inline-block h-[1em] w-[2px]',
            'translate-y-[2px] align-text-bottom',
            'animate-[pk-pulse_1.1s_ease-in-out_infinite]',
          )}
        />
      ) : null}
    </div>
  )
})
