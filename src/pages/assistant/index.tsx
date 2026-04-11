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
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  assistantResponseMeta,
  evidenceHref,
  selectedAiProvider,
} from '../../lib/intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import type {
  AiAssistantResponse,
  AiProviderConnectionTestReport,
  AiQueueStatus,
} from '../../lib/types'

/**
 * Defines the typed shape for conversation message.
 *
 * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ConversationMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  response?: AiAssistantResponse
}

/**
 * Explains how message id works.
 *
 * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function messageId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

/**
 * Explains how render paragraphs works.
 *
 * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function renderParagraphs(content: string) {
  return content
    .split('\n')
    .filter(Boolean)
    .map((line, index) => (
      <p key={`${line}-${index}`}>
        {line.startsWith('- ') ? (
          <span>
            {'• '}
            {line.slice(2)}
          </span>
        ) : (
          line
        )}
      </p>
    ))
}

/**
 * Renders the assistant route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Assistant expectations in the design docs.
 */
export function AssistantPage() {
  const { language, ns, t } = useI18n()
  const { refreshAppData, refreshKey, snapshot } = useShellData()
  const { activeProfileId } = useProfileScope()
  const [searchParams, setSearchParams] = useSearchParams()
  const [messages, setMessages] = useState<ConversationMessage[]>([])
  const [input, setInput] = useState(searchParams.get('question') ?? '')
  const [sending, setSending] = useState(false)
  const [queueStatus, setQueueStatus] = useState<AiQueueStatus | null>(null)
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

  useEffect(() => {
    if (!snapshot?.config.initialized || !snapshot.archiveStatus.unlocked)
      return
    let cancelled = false
    void backend
      .loadAiQueueStatus()
      .then((status) => {
        if (!cancelled) setQueueStatus(status)
      })
      .catch((error) => {
        if (!cancelled) {
          setQueueStatus(null)
          setPageError(
            error instanceof Error
              ? error.message
              : assistantT('loadingQueueAction'),
          )
        }
      })
    return () => {
      cancelled = true
    }
  }, [
    assistantT,
    refreshKey,
    snapshot?.archiveStatus.unlocked,
    snapshot?.config.initialized,
  ])

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
    const status = await backend.loadAiQueueStatus()
    setQueueStatus(status)
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
      setPageError(
        error instanceof Error
          ? error.message
          : assistantT('loadingQueueAction'),
      )
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles provider probe.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleProviderProbe() {
    if (!llmProvider) return
    setQueueAction(assistantT('testingProviderAction'))
    setPageError(null)
    try {
      const probe = await backend.testAiProviderConnection({
        providerId: llmProvider.id,
        purpose: 'llm',
      })
      setProviderProbe(probe)
    } catch (error) {
      setPageError(
        error instanceof Error
          ? error.message
          : assistantT('testingProviderAction'),
      )
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
      setPageError(
        error instanceof Error
          ? error.message
          : assistantT('loadingQueuedAnswerAction'),
      )
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles drain queue.
   *
   * Keeping this as a named declaration makes the Assistant surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleDrainQueue(jobId?: number | null) {
    setQueueAction(assistantT('runningQueuedJobsAction'))
    setPageError(null)
    try {
      await backend.runAiQueueJobs(1)
      await refreshQueue()
      if (jobId) {
        await handleLoadQueuedJob(jobId)
      } else {
        await refreshAppData()
      }
    } catch (error) {
      setPageError(
        error instanceof Error
          ? error.message
          : assistantT('runningQueuedJobsAction'),
      )
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
      await refreshQueue()
    } catch (error) {
      setPageError(
        error instanceof Error
          ? error.message
          : assistantT('cancellingAssistantJobAction'),
      )
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
    if (!question || sending) return
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
      setPageError(
        error instanceof Error ? error.message : assistantT('failedResponse'),
      )
      setMessages((current) => [
        ...current,
        {
          id: messageId('assistant'),
          role: 'assistant',
          content:
            error instanceof Error
              ? error.message
              : assistantT('failedResponse'),
          response: {
            state: 'failed',
            answer:
              error instanceof Error
                ? error.message
                : assistantT('failedResponse'),
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
      <section className="page-shell">
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
      </section>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <section className="page-shell">
        <PermissionGate
          detail={assistantT('lockedDetail')}
          eyebrow={assistantT('lockedEyebrow')}
          title={assistantT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {assistantT('reviewSecurity')}
          </Link>
        </PermissionGate>
      </section>
    )
  }

  if (!snapshot.config.ai.enabled || !snapshot.config.ai.assistantEnabled) {
    return (
      <section className="page-shell assistant-page">
        <StatusCallout
          tone="info"
          eyebrow={assistantT('statusEyebrow')}
          title={assistantT('disabledTitle')}
          body={assistantT('disabledBody')}
          actions={
            <Link className="btn-secondary" to="/settings">
              {assistantT('openSettings')}
            </Link>
          }
        />
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">{assistantT('emptyEyebrow')}</span>
            <span className="panel-action">{assistantT('emptyTitle')}</span>
          </div>
          <div className="panel-body intelligence-stack">
            <p className="dashboard-next-action">
              {assistantT('emptyDescription')}
            </p>
            <div className="intelligence-job-list">
              {suggestedQuestions.map((question) => (
                <div key={question} className="result-row">
                  <div className="result-row__header">
                    <strong>{question}</strong>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>
    )
  }

  return (
    <section className="page-shell assistant-page" data-testid="assistant-page">
      {activeProfileId ? (
        <StatusCallout
          tone="info"
          eyebrow={t('common.profileScope')}
          title={assistantT('scopedViewTitle')}
          body={assistantT('scopedViewBody', {
            profile: profileIdLabel(activeProfileId),
          })}
        />
      ) : null}
      {aiMeta && (
        <div className="intelligence-grid intelligence-grid--assistant">
          <StatusCallout
            tone={aiMeta.tone}
            eyebrow={assistantT('statusEyebrow')}
            title={aiMeta.label}
            body={aiMeta.description}
            actions={
              <div className="intelligence-actions">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => void handleProviderProbe()}
                  disabled={Boolean(queueAction) || !llmProvider}
                >
                  {assistantT('testProvider')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => void handleRefreshQueue()}
                  disabled={Boolean(queueAction)}
                >
                  {assistantT('refreshQueue')}
                </button>
                <Link className="btn-secondary" to="/settings">
                  {assistantT('openSettings')}
                </Link>
              </div>
            }
          />
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {assistantT('runningContext')}
              </span>
              <span className="panel-action">
                {llmProvider
                  ? `${llmProvider.name} / ${llmProvider.defaultModel}`
                  : assistantT('noLlmProviderSelected')}
              </span>
            </div>
            <div className="panel-body intelligence-stack">
              <div className="intelligence-stat-row">
                <div className="summary-stat">
                  <span className="dim">{t('common.profileScope')}</span>
                  <span className="mono">
                    {activeProfileId
                      ? profileIdLabel(activeProfileId)
                      : t('common.profileAllProfiles')}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{assistantT('llm')}</span>
                  <span className="mono">
                    {llmProvider?.id ?? assistantT('unset')}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{assistantT('retrieval')}</span>
                  <span className="mono">
                    {embeddingProvider?.id ?? assistantT('lexicalFallback')}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{assistantT('queuedLabel')}</span>
                  <span className="mono">
                    {queueStatus?.queued ?? snapshot.aiStatus.queuedJobs}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{assistantT('runningLabel')}</span>
                  <span className="mono">
                    {queueStatus?.running ?? snapshot.aiStatus.runningJobs}
                  </span>
                </div>
              </div>

              {providerProbe && (
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>
                      {providerProbe.ok
                        ? assistantT('providerReachable')
                        : assistantT('providerNeedsAttention')}
                    </strong>
                    <span className="mono-support">
                      {assistantT('providerProbeLatency', {
                        model: providerProbe.model,
                        latency:
                          providerProbe.latencyMs.toLocaleString(language),
                      })}
                    </span>
                  </div>
                  <p>{providerProbe.message}</p>
                  {providerProbe.actionHint ? (
                    <p className="mono-support">{providerProbe.actionHint}</p>
                  ) : null}
                </div>
              )}

              {queuedAssistantJobs.length > 0 && (
                <div className="intelligence-job-list">
                  {queuedAssistantJobs.map((job) => (
                    <div key={job.id} className="result-row">
                      <div className="result-row__header">
                        <strong>
                          {assistantT('queuedJobLabel', { id: job.id })}
                        </strong>
                        <span className="mono-support">{job.state}</span>
                      </div>
                      <p>
                        {job.summary ??
                          job.errorMessage ??
                          assistantT('queuedAssistantRequest')}
                      </p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {pageError && (
        <ErrorState
          title={assistantT('attentionTitle')}
          description={pageError}
        />
      )}

      <div className="assistant-layout">
        <div className="assistant-container">
          <div className="assistant-messages">
            {messages.length === 0 && !sending ? (
              <EmptyState
                action={
                  <div className="intelligence-actions">
                    {suggestedQuestions.map((question, index) => (
                      <button
                        key={question}
                        className="btn-secondary"
                        type="button"
                        onClick={() => setInput(question)}
                      >
                        {index === 0
                          ? assistantT('loadExamplePrompt')
                          : question}
                      </button>
                    ))}
                  </div>
                }
                description={assistantT('emptyDescription')}
                eyebrow={assistantT('emptyEyebrow')}
                title={assistantT('emptyTitle')}
              />
            ) : null}

            {messages.map((message) => {
              const response = message.response
              const responseMeta = response
                ? assistantResponseMeta(response, intelligenceT)
                : null
              return (
                <div
                  key={message.id}
                  className={`msg ${message.role === 'user' ? 'msg-user' : 'msg-ai'}`}
                >
                  <div className="msg-content">
                    {responseMeta && response ? (
                      <div className="assistant-message-head">
                        <span
                          className={`status-badge status-${responseMeta.tone}`}
                        >
                          {responseMeta.label}
                        </span>
                        <span className="mono-support">
                          {assistantT('responseMeta', {
                            jobId: String(response.jobId ?? '—'),
                            runId: String(response.runId ?? '—'),
                            provider:
                              response.providerId || t('common.pending'),
                          })}
                        </span>
                      </div>
                    ) : null}
                    {message.content ? renderParagraphs(message.content) : null}
                    {response?.notes?.length ? (
                      <div className="intelligence-note-list">
                        {response.notes.map((note) => (
                          <p key={note} className="mono-support">
                            {note}
                          </p>
                        ))}
                      </div>
                    ) : null}
                  </div>

                  {response?.citations?.length ? (
                    <div className="msg-evidence">
                      <div className="evidence-label">
                        {assistantT('evidenceLabel', {
                          count: response.citations.length,
                        })}
                      </div>
                      {response.citations.map((citation) => (
                        <Link
                          key={`${citation.historyId}-${citation.url}`}
                          className="evidence-item"
                          to={evidenceHref(citation)}
                        >
                          <span className="mono dim">
                            {formatDateTime(citation.visitedAt, language) ??
                              citation.visitedAt}
                          </span>
                          <span>{citation.title ?? citation.url}</span>
                        </Link>
                      ))}
                    </div>
                  ) : null}

                  {response?.state === 'queued' && response.jobId ? (
                    <div className="assistant-inline-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() =>
                          void handleLoadQueuedJob(response.jobId!)
                        }
                        disabled={Boolean(queueAction)}
                      >
                        {assistantT('checkStatus')}
                      </button>
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => void handleDrainQueue(response.jobId)}
                        disabled={Boolean(queueAction)}
                      >
                        {assistantT('runQueuedJob')}
                      </button>
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => void handleCancelJob(response.jobId!)}
                        disabled={Boolean(queueAction)}
                      >
                        {assistantT('cancel')}
                      </button>
                    </div>
                  ) : null}
                </div>
              )
            })}

            {sending && (
              <div className="msg msg-ai">
                <div className="msg-content">
                  <LoadingState
                    compact
                    label={assistantT('preparingAnswer')}
                    detail={assistantT('searchingArchive')}
                    progressLabel="1 / 3"
                    progressValue={33}
                  />
                </div>
              </div>
            )}
          </div>

          <div className="assistant-input-area">
            <div className="assistant-input-wrapper">
              <input
                aria-label={assistantT('inputLabel')}
                type="text"
                className="assistant-input"
                placeholder={assistantT('inputPlaceholder')}
                value={input}
                onChange={(event) => setInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') void handleSend()
                }}
                disabled={sending}
              />
              <button
                aria-label={assistantT('sendAction')}
                className="assistant-send"
                type="button"
                onClick={() => {
                  void handleSend()
                }}
                disabled={sending}
              >
                {assistantT('sendAction')}
              </button>
            </div>
            <div className="assistant-hint dim">
              {assistantT('auditTraceHint')}
            </div>
          </div>
        </div>

        <aside className="assistant-sidebar">
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">{assistantT('queueBoundary')}</span>
            </div>
            <div className="panel-body intelligence-stack">
              <p className="dashboard-next-action">
                {assistantT('queueBoundaryBody')}
              </p>
              {queueAction ? (
                <LoadingState
                  compact
                  label={queueAction}
                  detail={assistantT('queueBoundaryBody')}
                  progressLabel={assistantT('queueProgressLabel', {
                    queued: (
                      queueStatus?.queued ?? snapshot.aiStatus.queuedJobs
                    ).toLocaleString(language),
                    running: (
                      queueStatus?.running ?? snapshot.aiStatus.runningJobs
                    ).toLocaleString(language),
                  })}
                  progressValue={67}
                />
              ) : null}
            </div>
          </div>
        </aside>
      </div>
    </section>
  )
}
