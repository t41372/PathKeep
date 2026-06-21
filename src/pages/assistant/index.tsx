/**
 * This module renders the AI Assistant route — the marquee streaming-chat surface.
 *
 * Why this file exists:
 * - Route files turn design-system primitives, desktop read models, and shell scope into the
 *   user-facing workflow. This route owns the gate branches (setup / locked / unavailable /
 *   no-provider) and, when AI is ready, drives the streaming chat experience.
 *
 * Main declarations:
 * - `AssistantPage`
 *
 * Source-of-truth notes:
 * - Streaming mechanics live in `useAiChatStream` (ref-buffer + rAF flush; never freezes the
 *   main thread). This file only wires send/cancel and availability gating.
 * - Retires the old job-polling path (`askAiAssistant` / `loadAiAssistantJob`) in favor of
 *   `ai_chat_send` + `pathkeep://ai-stream` (W-AI-1 contract).
 * - The evidence/citation panel scaffold ships with the chat components (`AssistantTurn` +
 *   `PaperAssistantMessage` atoms accept `evidence`/`onSelectEvidence`); the route will pass real
 *   citations + an explorer deep-link handler once the agent produces them (W-AI-7).
 * - Stay aligned with `docs/design/screens-and-nav.md` and `docs/design/ux-principles.md`.
 */

import { useCallback, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { EmptyState } from '../../components/primitives/empty-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import {
  AssistantChatView,
  buildAssistantChatCopy,
  buildAssistantChatPrompts,
  useAiChatStream,
} from '../../components/assistant-chat'
import { backend } from '../../lib/backend-client'
import { subscribeToAiChatStream } from '../../lib/ipc/ai-stream'
import { useI18n } from '../../lib/i18n'
import { selectedAiProvider } from '../../lib/intelligence-ai-presentation'
import { optionalAiFeaturesAvailable } from '../../lib/release-capabilities'

/**
 * Renders the assistant route.
 *
 * The active surface is the streaming chat view; the early returns keep the route honest in its
 * setup / locked / unavailable / no-provider states so the UI is never broken.
 */
export function AssistantPage() {
  const { ns } = useI18n()
  const { snapshot } = useShellData()
  const [searchParams] = useSearchParams()
  const [input, setInput] = useState(searchParams.get('question') ?? '')

  const assistantT = ns('assistant')

  const llmProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'llm')
    : null
  const providerLabel = llmProvider
    ? `${llmProvider.name} / ${llmProvider.defaultModel}`
    : null

  const copy = useMemo(
    () => buildAssistantChatCopy(assistantT, { providerLabel }),
    [assistantT, providerLabel],
  )
  const prompts = useMemo(
    () => buildAssistantChatPrompts(assistantT),
    [assistantT],
  )

  // The streaming engine. Deps are stable-ish; the hook reads them via a ref each turn so a
  // provider change between turns is picked up without re-subscribing mid-stream.
  const { messages, streaming, awaitingFirstChunk, send, cancel } =
    useAiChatStream({
      sendChat: useCallback((request) => backend.sendAiChat(request), []),
      cancelChat: useCallback(
        (runId: string) => backend.cancelAiChat(runId),
        [],
      ),
      subscribe: subscribeToAiChatStream,
      providerId: llmProvider?.id ?? null,
      systemPrompt: snapshot?.config.ai.assistantSystemPrompt ?? null,
    })

  // Re-send the most recent user prompt after an error or a stop, for in-place recovery. The
  // composer is never unmounted, so the failed turn stays on screen while the retry streams in.
  const handleRetry = useCallback(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i].role === 'user') {
        send(messages[i].content)
        return
      }
    }
  }, [messages, send])

  if (!snapshot?.config.initialized) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="assistant-page"
      >
        <EmptyState
          description={assistantT('archiveNotInitializedDescription')}
          eyebrow={assistantT('statusEyebrow')}
          title={assistantT('archiveNotInitializedTitle')}
          action={
            <Link className="btn-primary" to="/onboarding">
              {assistantT('goToSetup')}
            </Link>
          }
        />
      </div>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="assistant-page"
      >
        <PermissionGate
          detail={assistantT('lockedDetail')}
          eyebrow={assistantT('lockedEyebrow')}
          title={assistantT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {assistantT('reviewSecurity')}
          </Link>
        </PermissionGate>
      </div>
    )
  }

  // Availability gate: release flag + AI toggle + assistant toggle. When closed, show the
  // roadmap / disabled state — never a broken chat box.
  if (
    !optionalAiFeaturesAvailable ||
    !snapshot.config.ai.enabled ||
    !snapshot.config.ai.assistantEnabled
  ) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
        data-testid="assistant-page"
      >
        <StatusCallout
          tone="info"
          eyebrow={assistantT('statusEyebrow')}
          title={
            optionalAiFeaturesAvailable
              ? assistantT('disabledTitle')
              : assistantT('deferredTitle')
          }
          body={
            optionalAiFeaturesAvailable
              ? assistantT('disabledBody')
              : assistantT('deferredBody')
          }
          actions={
            optionalAiFeaturesAvailable ? (
              <Link className="btn-secondary" to="/settings">
                {assistantT('openSettings')}
              </Link>
            ) : undefined
          }
        />
        <PaperCard testId="assistant-deferred-panel">
          <PaperCardHeader
            title={
              optionalAiFeaturesAvailable
                ? assistantT('emptyEyebrow')
                : assistantT('deferredPanelEyebrow')
            }
            right={
              <PaperCardBadge>
                {optionalAiFeaturesAvailable
                  ? assistantT('emptyTitle')
                  : assistantT('deferredBadge')}
              </PaperCardBadge>
            }
          />
          <PaperCardBody className="intelligence-stack">
            <p className="mono-support">
              {optionalAiFeaturesAvailable
                ? assistantT('emptyDescription')
                : assistantT('deferredPanelBody')}
            </p>
          </PaperCardBody>
        </PaperCard>
      </div>
    )
  }

  // AI is on but no LLM provider is configured: chat can't run. Offer a clear next step and
  // keep the rest of PathKeep usable (keyword search / Core Intelligence don't need a provider).
  if (!llmProvider) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
        data-testid="assistant-page"
      >
        <StatusCallout
          tone="info"
          eyebrow={assistantT('statusEyebrow')}
          title={assistantT('chatNoProviderTitle')}
          body={assistantT('chatNoProviderBody')}
          actions={
            <Link className="btn-secondary" to="/settings">
              {assistantT('openSettings')}
            </Link>
          }
        />
      </div>
    )
  }

  return (
    <div
      className="mx-auto flex h-full w-full max-w-[820px] flex-col px-2 pt-4"
      data-testid="assistant-page"
    >
      <AssistantChatView
        messages={messages}
        input={input}
        streaming={streaming}
        awaitingFirstChunk={awaitingFirstChunk}
        canSend={Boolean(llmProvider)}
        prompts={prompts}
        copy={copy}
        onInputChange={setInput}
        onSend={(text) => {
          send(text)
          setInput('')
        }}
        onCancel={cancel}
        onRetry={handleRetry}
        onPickPrompt={(prompt) => setInput(prompt.text)}
        testId="assistant-chat-view"
      />
    </div>
  )
}
