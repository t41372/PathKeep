/**
 * Mounts PaperAssistantView with the conversation state from the v0.2
 * AssistantPage. Splitting this out keeps the route file thin and gives
 * the AssistantConversationMessage → PaperAssistantMessageDescriptor
 * mapping its own narrow surface.
 *
 * ## Responsibilities
 * - Project AssistantConversationMessage[] into the paper-shaped
 *   message descriptors PaperAssistantView consumes.
 * - Derive evidence rows from `response.citations`, falling back to a
 *   safe domain string when the citation URL can't be parsed.
 *
 * ## Not responsible for
 * - Owning conversation state — the route still calls handleSend / pushes
 *   user messages into the list.
 */

import { useMemo } from 'react'
import {
  PaperAssistantView,
  type PaperAssistantMessageDescriptor,
} from '@/components/explorer-paper'
import type { AssistantConversationMessage } from './conversation-panel'
import {
  buildPaperAssistantPrompts,
  buildPaperAssistantViewCopy,
} from './paper-assistant-copy'
import { citationsToEvidence } from './paper-assistant-helpers'

export interface PaperAssistantPanelProps {
  assistantT: (key: string, vars?: Record<string, string | number>) => string
  input: string
  messages: AssistantConversationMessage[]
  onInputChange: (next: string) => void
  onSend: () => void
  providerLabel: string | null
  sending: boolean
  userByline: string
}

export function PaperAssistantPanel({
  assistantT,
  input,
  messages,
  onInputChange,
  onSend,
  providerLabel,
  sending,
  userByline,
}: PaperAssistantPanelProps) {
  const copy = useMemo(
    () =>
      buildPaperAssistantViewCopy(assistantT, {
        providerLabel: providerLabel ?? null,
      }),
    [assistantT, providerLabel],
  )
  const prompts = useMemo(
    () => buildPaperAssistantPrompts(assistantT),
    [assistantT],
  )
  const assistantBylineLive = providerLabel
    ? assistantT('paperAssistantByline', { provider: providerLabel })
    : assistantT('paperComposerAttributionFallback')
  const mapped = useMemo<PaperAssistantMessageDescriptor[]>(
    () =>
      messages.map((message) => ({
        id: message.id,
        role: message.role === 'user' ? 'user' : 'ai',
        content: message.content,
        byline: message.role === 'user' ? userByline : assistantBylineLive,
        evidence:
          message.role === 'assistant' && message.response
            ? citationsToEvidence(message.response.citations)
            : undefined,
      })),
    [assistantBylineLive, messages, userByline],
  )

  return (
    <div className="assistant-paper-layout" data-testid="paper-assistant-panel">
      <PaperAssistantView
        messages={mapped}
        input={input}
        pending={sending}
        prompts={prompts}
        onInputChange={onInputChange}
        onSubmit={(value) => {
          onInputChange(value)
          onSend()
        }}
        onPickPrompt={(prompt) => onInputChange(prompt.text)}
        copy={copy}
        testId="paper-assistant-view"
      />
    </div>
  )
}
