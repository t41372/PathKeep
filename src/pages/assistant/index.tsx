/**
 * This module renders the AI Assistant route and keeps it grounded in explicit evidence, queue state, and shared profile scope.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `AssistantPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  assistantResponseMeta,
  selectedAiProvider,
} from '../../lib/intelligence-ai-presentation'
import { optionalAiFeaturesAvailable } from '../../lib/release-capabilities'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import type {
  AiAssistantResponse,
  AiProviderConfig,
  AiProviderConnectionTestReport,
} from '../../lib/types'
import {
  AssistantConversationPanel,
  type AssistantConversationMessage,
} from './conversation-panel'
import { AssistantQueueSidebar, AssistantRuntimePanels } from './runtime-panels'
import { PaperAssistantPanel } from './paper-assistant-panel'

/**
 * Explains how message id works.
 *
 * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function messageId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

/**
 * Renders the assistant route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Assistant expectations in the design docs.
 */
export function AssistantPage() {
  const { language, ns, t } = useI18n()
  const {
    refreshAppData,
    refreshRuntimeStatus,
    snapshot,
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
  } = useShellData()
  const { activeProfileId } = useProfileScope()
  const [searchParams, setSearchParams] = useSearchParams()
  const [messages, setMessages] = useState<AssistantConversationMessage[]>([])
  const [input, setInput] = useState(searchParams.get('question') ?? '')
  const [sending, setSending] = useState(false)
  const [providerProbe, setProviderProbe] =
    useState<AiProviderConnectionTestReport | null>(null)
  const [queueAction, setQueueAction] = useState<string | null>(null)
  const [pageError, setPageError] = useState<string | null>(null)

  const assistantT = ns('assistant')
  const intelligenceT = ns('intelligence')
  const suggestedQuestions = [
    assistantT('examplePrompt'),
    assistantT('examplePromptFocus'),
    assistantT('examplePromptTimeline'),
  ]
  const aiMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const llmProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'llm')
    : null
  const embeddingProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'embedding')
    : null

  useEffect(() => {
    const seededQuestion = searchParams.get('question')
    if (seededQuestion) setInput(seededQuestion)
  }, [searchParams])
  const queueStatus = runtimeStatus.aiQueue
  const queueError = runtimeStatus.error
  const assistantAttention = pageError ?? queueError

  const queuedAssistantJobs = useMemo(
    () =>
      (queueStatus?.recentJobs ?? []).filter(
        (job) => job.jobType === 'assistant',
      ),
    [queueStatus],
  )

  /**
   * Merges assistant message into an existing collection without losing stable identifiers.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function upsertAssistantMessage(
    jobId: number | undefined | null,
    response: AiAssistantResponse,
  ) {
    if (!jobId) {
      setMessages((current) => [
        ...current,
        {
          id: messageId('assistant'),
          role: 'assistant',
          content: response.answer,
          response,
        },
      ])
      return
    }
    setMessages((current) => {
      const next = [...current]
      const index = next.findIndex(
        (message) => message.response?.jobId === jobId,
      )
      if (index === -1) {
        next.push({
          id: messageId('assistant'),
          role: 'assistant',
          content: response.answer,
          response,
        })
      } else {
        next[index] = {
          ...next[index],
          content: response.answer,
          response,
        }
      }
      return next
    })
  }

  /**
   * Refreshes queue.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function refreshQueue() {
    await refreshRuntimeStatus()
  }

  /**
   * Handles refresh queue.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRefreshQueue() {
    setQueueAction(assistantT('loadingQueueAction'))
    setPageError(null)
    try {
      await refreshQueue()
    } catch (error) {
      setPageError(describeError(error, 'refresh_ai_queue'))
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles provider probe.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleProviderProbe(provider: AiProviderConfig) {
    setQueueAction(assistantT('testingProviderAction'))
    setPageError(null)
    try {
      const probe = await backend.testAiProviderConnection({
        providerId: provider.id,
        purpose: 'llm',
      })
      setProviderProbe(probe)
    } catch (error) {
      setPageError(describeError(error, 'test_ai_provider_connection'))
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles load queued job.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleLoadQueuedJob(jobId: number) {
    setQueueAction(assistantT('loadingQueuedAnswerAction'))
    setPageError(null)
    try {
      const response = await backend.loadAiAssistantJob(jobId)
      upsertAssistantMessage(jobId, response)
      await Promise.all([refreshQueue(), refreshAppData()])
    } catch (error) {
      setPageError(describeError(error, 'load_ai_assistant_job'))
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles drain queue.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleDrainQueue(jobId: number) {
    setQueueAction(assistantT('runningQueuedJobsAction'))
    setPageError(null)
    try {
      await backend.runAiQueueJobs(1)
      await refreshQueue()
      await handleLoadQueuedJob(jobId)
    } catch (error) {
      setPageError(describeError(error, 'run_ai_queue_jobs'))
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles cancel job.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleCancelJob(jobId: number) {
    setQueueAction(assistantT('cancellingAssistantJobAction'))
    setPageError(null)
    try {
      await backend.cancelAiJob(jobId)
      const response = await backend.loadAiAssistantJob(jobId)
      upsertAssistantMessage(jobId, response)
      await Promise.all([refreshQueue(), refreshAppData()])
    } catch (error) {
      setPageError(describeError(error, 'cancel_ai_job'))
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles send.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleSend() {
    const question = input.trim()
    if (!question) return
    setPageError(null)
    setInput('')
    setSearchParams((current) => {
      const next = new URLSearchParams(current)
      next.delete('question')
      return next
    })
    setMessages((current) => [
      ...current,
      { id: messageId('user'), role: 'user', content: question },
    ])
    setSending(true)
    try {
      const response = await backend.askAiAssistant({
        question,
        profileId: activeProfileId,
      })
      upsertAssistantMessage(response.jobId, response)
      await refreshQueue()
    } catch (error) {
      const message = describeError(error, 'ask_ai_assistant')
      setPageError(message)
      setMessages((current) => [
        ...current,
        {
          id: messageId('assistant'),
          role: 'assistant',
          content: message,
          response: {
            state: 'failed',
            answer: message,
            jobId: null,
            runId: null,
            providerId: llmProvider?.id ?? '',
            embeddingProviderId:
              embeddingProvider?.id ?? assistantT('lexicalFallback'),
            citations: [],
            notes: [],
          },
        },
      ])
    } finally {
      setSending(false)
    }
  }

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
            {optionalAiFeaturesAvailable ? (
              <div className="intelligence-job-list">
                {suggestedQuestions.map((question) => (
                  <div
                    key={question}
                    className="border-border-light bg-paper rounded-paper border px-4 py-3"
                  >
                    <strong className="text-ink">{question}</strong>
                  </div>
                ))}
              </div>
            ) : null}
          </PaperCardBody>
        </PaperCard>
      </div>
    )
  }

  // Active-AI conversation surface. This branch is unreachable while
  // `optionalAiFeaturesAvailable` is false (v0.2.0), and its runtime/queue
  // children still use v0.2 `.panel` chrome. It stays on the legacy
  // `page-shell` shell as one coherent block until the v0.3 AI sweep migrates
  // the whole conversation experience to the paper grammar together; see
  // F-LEGACY-CSS (docs/review/2026-06-14) for the deferred scope.
  return (
    <section className="page-shell assistant-page" data-testid="assistant-page">
      <AssistantRuntimePanels
        activeProfileLabel={
          activeProfileId ? profileIdLabel(activeProfileId) : null
        }
        aiMeta={aiMeta}
        assistantT={assistantT}
        llmProviderAvailable={Boolean(llmProvider)}
        llmProviderDisplay={
          llmProvider
            ? `${llmProvider.name} / ${llmProvider.defaultModel}`
            : assistantT('noLlmProviderSelected')
        }
        llmProviderId={llmProvider?.id ?? assistantT('unset')}
        embeddingProviderId={
          embeddingProvider?.id ?? assistantT('lexicalFallback')
        }
        language={language}
        onProviderProbe={
          llmProvider
            ? () => {
                void handleProviderProbe(llmProvider)
              }
            : undefined
        }
        onRefreshQueue={() => {
          void handleRefreshQueue()
        }}
        profileScopeLabel={t('common.profileScope')}
        profileScopeValue={
          activeProfileId
            ? profileIdLabel(activeProfileId)
            : t('common.profileAllProfiles')
        }
        providerProbe={providerProbe}
        queuedAssistantJobs={queuedAssistantJobs}
        queuedCount={queueStatus?.queued ?? snapshot.aiStatus.queuedJobs}
        queueAction={queueAction}
        runningCount={queueStatus?.running ?? snapshot.aiStatus.runningJobs}
      />

      {assistantAttention ? (
        <ErrorState
          title={assistantT('attentionTitle')}
          description={assistantAttention}
        />
      ) : null}

      <div className="assistant-layout">
        {searchParams.get('layout') === 'paper' ? (
          <PaperAssistantPanel
            assistantT={assistantT}
            input={input}
            messages={messages}
            onInputChange={setInput}
            onSend={() => {
              void handleSend()
            }}
            providerLabel={
              llmProvider
                ? `${llmProvider.name} / ${llmProvider.defaultModel}`
                : null
            }
            sending={sending}
            userByline={assistantT('paperUserByline')}
          />
        ) : (
          <AssistantConversationPanel
            assistantT={assistantT}
            handleCancelJob={handleCancelJob}
            handleDrainQueue={handleDrainQueue}
            handleLoadQueuedJob={handleLoadQueuedJob}
            input={input}
            language={language}
            messages={messages}
            onInputChange={setInput}
            onPromptPick={setInput}
            onSend={() => {
              void handleSend()
            }}
            queueAction={queueAction}
            responseMetaFor={(response) =>
              assistantResponseMeta(response, intelligenceT)
            }
            sending={sending}
            suggestedQuestions={suggestedQuestions}
            t={t}
          />
        )}

        <AssistantQueueSidebar
          assistantT={assistantT}
          queuedCount={queueStatus?.queued ?? snapshot.aiStatus.queuedJobs}
          queueAction={queueAction}
          runningCount={queueStatus?.running ?? snapshot.aiStatus.runningJobs}
        />
      </div>
    </section>
  )
}
