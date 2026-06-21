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
  ChatHistoryExplorer,
  buildAssistantChatCopy,
  buildAssistantChatPrompts,
  buildChatHistoryCopy,
  useAiChatStream,
  useChatHistory,
  type ChatHistoryBackend,
} from '../../components/assistant-chat'
import { backend } from '../../lib/backend-client'
import { subscribeToAiChatStream } from '../../lib/ipc/ai-stream'
import { useI18n } from '../../lib/i18n'
import { selectedAiProvider } from '../../lib/intelligence-ai-presentation'
import { optionalAiFeaturesAvailable } from '../../lib/release-capabilities'

/** Stable backend bindings for the conversation-history controller (identity never changes). */
const chatHistoryBackend: ChatHistoryBackend = {
  listConversations: () => backend.listAiConversations(),
  saveConversation: (request) => backend.saveAiConversation(request),
  loadConversation: (id) => backend.loadAiConversation(id),
  deleteConversation: (id) => backend.deleteAiConversation(id),
  renameConversation: (request) => backend.renameAiConversation(request),
}

/**
 * Renders the assistant route.
 *
 * The active surface is the streaming chat view; the early returns keep the route honest in its
 * setup / locked / unavailable / no-provider states so the UI is never broken.
 */
export function AssistantPage() {
  const { ns, language } = useI18n()
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
  const historyCopy = useMemo(
    // `now` defaults to the real clock inside the builder; pass undefined so the render path itself
    // stays pure (the lint rule forbids calling Date.now() at the call site).
    () => buildChatHistoryCopy(assistantT, undefined, language),
    [assistantT, language],
  )

  // The chat surface is fully active only when AI + assistant are on and a provider is set. The
  // history controller stays dormant otherwise so the agent plane is never touched on gated pages.
  const chatActive = Boolean(
    snapshot?.config.initialized &&
    snapshot?.archiveStatus.unlocked &&
    optionalAiFeaturesAvailable &&
    snapshot?.config.ai.enabled &&
    snapshot?.config.ai.assistantEnabled &&
    llmProvider,
  )

  // Conversation-persistence controller: lists past chats, saves on finalize, opens / deletes.
  const history = useChatHistory({
    backend: chatHistoryBackend,
    providerId: llmProvider?.id ?? null,
    enabled: chatActive,
  })

  // The streaming engine. Deps are stable-ish; the hook reads them via a ref each turn so a
  // provider change between turns is picked up without re-subscribing mid-stream. `onTurnFinalized`
  // persists each finished turn off the main thread (never per chunk), so saving cannot jank the
  // stream.
  const { messages, streaming, awaitingFirstChunk, send, cancel, reset } =
    useAiChatStream({
      sendChat: useCallback((request) => backend.sendAiChat(request), []),
      cancelChat: useCallback(
        (runId: string) => backend.cancelAiChat(runId),
        [],
      ),
      subscribe: subscribeToAiChatStream,
      providerId: llmProvider?.id ?? null,
      systemPrompt: snapshot?.config.ai.assistantSystemPrompt ?? null,
      onTurnFinalized: history.persistTurn,
    })

  // Responsive deferral (W-AI-3 review item 14): on a narrow window the 260px drawer should
  // auto-collapse / overlay rather than squeeze the chat column. That needs a width observer to flip
  // `historyOpen` (or a container-query-driven overlay variant of the drawer), which is more than a
  // low-cost change here — deferred. The drawer already starts collapsed and is user-toggleable, so
  // narrow windows are usable today; the auto-behavior is the only gap.
  const [historyOpen, setHistoryOpen] = useState(false)

  // Open a past conversation: load it, then hydrate the chat hook with its messages.
  const handleOpenConversation = useCallback(
    (id: string) => {
      void history.openConversation(id).then((hydrated) => {
        if (hydrated) reset(hydrated)
      })
    },
    [history, reset],
  )

  // New chat: clear the active selection and start a fresh, empty transcript.
  const handleNewChat = useCallback(() => {
    history.startNewChat()
    reset()
  }, [history, reset])

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
      className="mx-auto flex h-full w-full max-w-[1100px] flex-row gap-3 px-2 pt-4"
      data-testid="assistant-page"
    >
      <ChatHistoryExplorer
        open={historyOpen}
        conversations={history.conversations}
        activeId={history.activeId}
        loading={history.loading}
        error={history.error}
        copy={historyCopy}
        onToggle={() => setHistoryOpen((current) => !current)}
        onNewChat={handleNewChat}
        onOpenConversation={handleOpenConversation}
        onDeleteConversation={(id) => {
          void history.deleteConversation(id)
        }}
        onRenameConversation={(id, title) => {
          void history.renameConversation(id, title)
        }}
        onRetry={history.refresh}
        testId="assistant-chat-history"
      />
      <div className="flex min-w-0 flex-1 flex-col">
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
    </div>
  )
}
