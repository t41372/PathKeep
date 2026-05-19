/**
 * Assistant input composer — the textarea + send button row that anchors
 * the bottom of the Assistant view.
 *
 * Layout matches `pk-assistant.jsx` → `.assist-input-wrap`:
 *
 *   ┌──────────────────────────────────────────────┐ ┌────┐
 *   │  Ask about your archive…                     │ │ →  │
 *   │                                              │ └────┘
 *   └──────────────────────────────────────────────┘
 *   Powered by local LLM · Ollama / llama3.2    ↵ send · ⇧↵ newline
 *
 * ## Responsibilities
 * - Render the textarea with auto-grow up to a max height and the send
 *   button at the same baseline.
 * - Submit on Enter (without Shift). Shift+Enter inserts a newline.
 * - Render the meta line beneath with provider attribution + key hints.
 *
 * ## Not responsible for
 * - Calling the LLM — caller passes onSubmit which receives the trimmed
 *   query string.
 * - Empty-input prompts — those live in PaperAssistantGreeting.
 */

import { useCallback, type KeyboardEvent } from 'react'
import { cn } from '@/lib/cn'

export interface PaperAssistantComposerCopy {
  placeholder: string
  sendLabel: string
  /** Mono attribution string, e.g. "Powered by local LLM · Ollama / llama3.2". */
  attribution: string
  /** Mono key-hint string, e.g. "↵ send · ⇧↵ newline". */
  keyHint: string
}

export interface PaperAssistantComposerProps {
  value: string
  onChange: (next: string) => void
  onSubmit: (value: string) => void
  copy: PaperAssistantComposerCopy
  /** True while the assistant is awaiting a response — disables the input. */
  pending?: boolean
  className?: string
  testId?: string
}

export function PaperAssistantComposer({
  value,
  onChange,
  onSubmit,
  copy,
  pending = false,
  className,
  testId,
}: PaperAssistantComposerProps) {
  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault()
        const trimmed = value.trim()
        if (!trimmed || pending) return
        onSubmit(trimmed)
      }
    },
    [onSubmit, pending, value],
  )

  const canSubmit = value.trim().length > 0 && !pending

  return (
    <form
      data-testid={testId}
      onSubmit={(event) => {
        event.preventDefault()
        if (canSubmit) onSubmit(value.trim())
      }}
      className={cn(
        'border-border-light flex flex-col border-t pb-2 pt-4',
        className,
      )}
    >
      <div className="flex items-end gap-2">
        <textarea
          data-testid="paper-assistant-input"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={copy.placeholder}
          rows={1}
          disabled={pending}
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
        <button
          type="submit"
          disabled={!canSubmit}
          aria-label={copy.sendLabel}
          title={copy.sendLabel}
          className={cn(
            'rounded-paper inline-grid h-[46px] w-[46px] place-items-center',
            'bg-accent text-paper text-[16px]',
            'enabled:hover:opacity-85 transition-opacity duration-150',
            'disabled:cursor-not-allowed disabled:opacity-40',
          )}
        >
          →
        </button>
      </div>
      <div className="text-ink-faint mt-2 flex justify-between font-mono text-[10px]">
        <span>{copy.attribution}</span>
        <span>{copy.keyHint}</span>
      </div>
    </form>
  )
}
