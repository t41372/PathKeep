/**
 * PaperAssistantView — composed chat shell.
 *
 * Combines the empty-state greeting, the scrolling messages list, and the
 * composer pinned at the bottom into the layout from `pk-assistant.jsx`
 * `.assist-wrap`.
 *
 * ## Responsibilities
 * - Render the greeting when no messages exist; otherwise render the
 *   conversation list.
 * - Auto-scroll the messages container to the bottom whenever the message
 *   count grows (so the user always sees the latest exchange).
 * - Pass the composer's submit / change handlers straight through.
 *
 * ## Not responsible for
 * - Calling the assistant backend — caller wires onSubmit to whatever
 *   ask_ai_assistant path the route uses.
 * - Stream rendering — caller hands the full message list each render.
 */

import { useEffect, useRef, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperAssistantComposer,
  type PaperAssistantComposerCopy,
} from './paper-assistant-composer'
import {
  PaperAssistantGreeting,
  type PaperAssistantGreetingPrompt,
} from './paper-assistant-greeting'
import {
  PaperAssistantMessage,
  type PaperAssistantEvidence,
  type PaperAssistantRole,
} from './paper-assistant-message'

export interface PaperAssistantMessageDescriptor {
  id: string
  role: PaperAssistantRole
  content: ReactNode
  byline?: string
  evidence?: readonly PaperAssistantEvidence[]
}

export interface PaperAssistantViewCopy {
  greetingTitle: string
  greetingSubtitle: ReactNode
  composer: PaperAssistantComposerCopy
  /** Mono evidence-panel label with `{count}` placeholder. */
  evidenceLabel: string
}

export interface PaperAssistantViewProps {
  messages: readonly PaperAssistantMessageDescriptor[]
  input: string
  pending?: boolean
  prompts?: readonly PaperAssistantGreetingPrompt[]
  onInputChange: (next: string) => void
  onSubmit: (value: string) => void
  onPickPrompt?: (prompt: PaperAssistantGreetingPrompt) => void
  onSelectEvidence?: (evidence: PaperAssistantEvidence) => void
  copy: PaperAssistantViewCopy
  className?: string
  testId?: string
}

export function PaperAssistantView({
  messages,
  input,
  pending = false,
  prompts,
  onInputChange,
  onSubmit,
  onPickPrompt,
  onSelectEvidence,
  copy,
  className,
  testId,
}: PaperAssistantViewProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null)
  useEffect(() => {
    const node = scrollRef.current
    if (node) node.scrollTop = node.scrollHeight
  }, [messages.length])

  const isEmpty = messages.length === 0

  return (
    <section
      data-testid={testId}
      className={cn(
        'mx-auto flex h-full w-full max-w-[780px] flex-col',
        className,
      )}
    >
      <div
        ref={scrollRef}
        data-testid="paper-assistant-messages"
        className="flex flex-1 flex-col gap-[22px] overflow-y-auto pt-2 pb-5"
      >
        {isEmpty ? (
          <PaperAssistantGreeting
            title={copy.greetingTitle}
            subtitle={copy.greetingSubtitle}
            prompts={prompts}
            onSelectPrompt={onPickPrompt}
          />
        ) : (
          messages.map((message) => (
            <PaperAssistantMessage
              key={message.id}
              role={message.role}
              byline={message.byline}
              evidence={message.evidence}
              evidenceLabel={copy.evidenceLabel}
              onSelectEvidence={onSelectEvidence}
              testId={`paper-assistant-message-${message.id}`}
            >
              {message.content}
            </PaperAssistantMessage>
          ))
        )}
      </div>

      <PaperAssistantComposer
        value={input}
        onChange={onInputChange}
        onSubmit={onSubmit}
        pending={pending}
        copy={copy.composer}
        testId="paper-assistant-composer"
      />
    </section>
  )
}
