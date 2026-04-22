/**
 * @file conversation-panel.tsx
 * @description Render-only conversation, evidence, and input shell for the Assistant route.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Render the assistant message timeline, evidence links, queued-job inline actions, and composer.
 * - Keep the empty-state prompt suggestions and sending skeleton in one owner.
 * - Hold the paragraph formatting helper close to the conversation surface that uses it.
 *
 * ## Not responsible for
 * - Sending assistant requests or mutating queue state.
 * - Fetching provider/runtime state.
 * - Rendering the Assistant route context/status panels.
 *
 * ## Dependencies
 * - Depends on shared loading and empty-state primitives plus evidence route helpers.
 * - Depends on translator callbacks and precomputed message state from the route owner.
 *
 * ## Performance notes
 * - Render-only conversation shell so the route can stage queue/runtime work without rebuilding formatting helpers every time.
 */

import { Link } from 'react-router-dom'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { formatDateTime } from '../../lib/format'
import { evidenceHref } from '../../lib/intelligence-links'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { AiAssistantResponse } from '../../lib/types'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Stable conversation message shape used by the Assistant route and extracted
 * conversation panel.
 */
export interface AssistantConversationMessage {
  content: string
  id: string
  response?: AiAssistantResponse
  role: 'user' | 'assistant'
}

interface AssistantConversationPanelProps {
  assistantT: Translator
  handleCancelJob: (jobId: number) => Promise<void>
  handleDrainQueue: (jobId?: number | null) => Promise<void>
  handleLoadQueuedJob: (jobId: number) => Promise<void>
  input: string
  language: ResolvedLanguage
  messages: AssistantConversationMessage[]
  onInputChange: (value: string) => void
  onPromptPick: (question: string) => void
  onSend: () => void
  queueAction: string | null
  responseMetaFor: (response: AiAssistantResponse) => {
    label: string
    tone: string
  }
  sending: boolean
  suggestedQuestions: string[]
  t: Translator
}

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
 * Renders the Assistant conversation shell from already-prepared message and
 * action state owned by the route.
 */
export function AssistantConversationPanel({
  assistantT,
  handleCancelJob,
  handleDrainQueue,
  handleLoadQueuedJob,
  input,
  language,
  messages,
  onInputChange,
  onPromptPick,
  onSend,
  queueAction,
  responseMetaFor,
  sending,
  suggestedQuestions,
  t,
}: AssistantConversationPanelProps) {
  return (
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
                    onClick={() => onPromptPick(question)}
                  >
                    {index === 0 ? assistantT('loadExamplePrompt') : question}
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
          const responseMeta = response ? responseMetaFor(response) : null
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
                        provider: response.providerId || t('common.pending'),
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
                    onClick={() => void handleLoadQueuedJob(response.jobId!)}
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

        {sending ? (
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
        ) : null}
      </div>

      <div className="assistant-input-area">
        <div className="assistant-input-wrapper">
          <input
            aria-label={assistantT('inputLabel')}
            type="text"
            className="assistant-input"
            placeholder={assistantT('inputPlaceholder')}
            value={input}
            onChange={(event) => onInputChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') onSend()
            }}
            disabled={sending}
          />
          <button
            aria-label={assistantT('sendAction')}
            className="assistant-send"
            type="button"
            onClick={onSend}
            disabled={sending}
          >
            {assistantT('sendAction')}
          </button>
        </div>
        <div className="assistant-hint dim">{assistantT('auditTraceHint')}</div>
      </div>
    </div>
  )
}
