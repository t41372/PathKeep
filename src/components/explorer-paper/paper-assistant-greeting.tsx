/**
 * Empty-state greeting for the paper Assistant view — shown when the
 * conversation list is empty.
 *
 * Layout follows `pk-assistant.jsx` `.assist-greeting` + `.assist-empty-prompts`:
 *
 *   ┌─────────────────────────────────────────────────┐
 *   │       What would you like to remember?          │
 *   │  I can read your archive and tell you what's…   │
 *   │                                                 │
 *   │  ┌─────────────────┐  ┌─────────────────────┐   │
 *   │  │ Sample prompt 1 │  │  Sample prompt 2    │   │
 *   │  └─────────────────┘  └─────────────────────┘   │
 *   └─────────────────────────────────────────────────┘
 */

import { cn } from '@/lib/cn'

export interface PaperAssistantGreetingPrompt {
  id: string
  text: string
}

export interface PaperAssistantGreetingProps {
  title: string
  /** Sub line beneath the title; supports React content for line breaks. */
  subtitle: React.ReactNode
  prompts?: readonly PaperAssistantGreetingPrompt[]
  onSelectPrompt?: (prompt: PaperAssistantGreetingPrompt) => void
  className?: string
  testId?: string
}

export function PaperAssistantGreeting({
  title,
  subtitle,
  prompts = [],
  onSelectPrompt,
  className,
  testId,
}: PaperAssistantGreetingProps) {
  return (
    <div
      data-testid={testId}
      className={cn('flex flex-col items-center', className)}
    >
      <h2 className="text-ink mt-10 font-serif text-[22px] tracking-[-0.01em]">
        {title}
      </h2>
      <p className="text-ink-faint mt-4 text-center font-serif text-[14px] italic leading-[1.5]">
        {subtitle}
      </p>
      {prompts.length > 0 ? (
        <div className="mt-5 grid w-full grid-cols-1 gap-2 sm:grid-cols-2">
          {prompts.map((prompt) => (
            <button
              type="button"
              key={prompt.id}
              onClick={() => onSelectPrompt?.(prompt)}
              disabled={!onSelectPrompt}
              data-testid={`paper-assistant-prompt-${prompt.id}`}
              className={cn(
                'rounded-paper border-border-light bg-card-paper border px-[12px] py-[10px]',
                'text-left font-serif text-[13px] italic leading-[1.4]',
                'text-ink-secondary transition-colors duration-150',
                'enabled:hover:border-accent enabled:hover:text-ink',
                'disabled:cursor-not-allowed disabled:opacity-60',
              )}
            >
              {prompt.text}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}
