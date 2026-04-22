/**
 * @file runtime-panels.tsx
 * @description Render-only runtime status and queue chrome for the Assistant route.
 * @module pages/assistant
 *
 * ## Responsibilities
 * - Render the scoped-view callout, AI status callout, running-context summary, provider probe results, and queued-job preview rows.
 * - Render the queue sidebar loading surface from already-derived route state.
 *
 * ## Not responsible for
 * - Fetching runtime status, mutating queue state, or testing providers directly.
 * - Owning conversation state, prompt input, or route-level gating.
 *
 * ## Dependencies
 * - Depends on shared primitives for callouts and loading states.
 * - Depends on the route owner for all already-derived labels, counts, and callbacks.
 *
 * ## Performance notes
 * - Render-only owner that works from already-loaded route state to avoid extra queue or provider reads while the user types.
 */

import { Link } from 'react-router-dom'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import type {
  AiProviderConnectionTestReport,
  AiQueueJob,
} from '../../lib/types'
import type { IntelligenceTone } from '../../lib/intelligence-ai-presentation'

type Translate = (key: string, vars?: Record<string, string | number>) => string

interface AssistantStatusMeta {
  label: string
  tone: IntelligenceTone
  description: string
}

interface AssistantRuntimePanelsProps {
  activeProfileLabel: string | null
  aiMeta: AssistantStatusMeta | null
  assistantT: Translate
  llmProviderAvailable: boolean
  llmProviderDisplay: string
  llmProviderId: string
  embeddingProviderId: string
  language: string
  onProviderProbe: () => void
  onRefreshQueue: () => void
  profileScopeLabel: string
  profileScopeValue: string
  providerProbe: AiProviderConnectionTestReport | null
  queuedAssistantJobs: AiQueueJob[]
  queuedCount: number
  queueAction: string | null
  runningCount: number
}

/**
 * Renders the Assistant runtime status, provider probe, and queued-job overview.
 *
 * The route keeps ownership of queue mutations and state refresh; this component
 * only turns those derived values into the status chrome shown above the
 * conversation surface.
 */
export function AssistantRuntimePanels({
  activeProfileLabel,
  aiMeta,
  assistantT,
  llmProviderAvailable,
  llmProviderDisplay,
  llmProviderId,
  embeddingProviderId,
  language,
  onProviderProbe,
  onRefreshQueue,
  profileScopeLabel,
  profileScopeValue,
  providerProbe,
  queuedAssistantJobs,
  queuedCount,
  queueAction,
  runningCount,
}: AssistantRuntimePanelsProps) {
  if (!aiMeta) return null

  return (
    <>
      {activeProfileLabel ? (
        <StatusCallout
          tone="info"
          eyebrow={assistantT('statusEyebrow')}
          title={assistantT('scopedViewTitle')}
          body={assistantT('scopedViewBody', {
            profile: activeProfileLabel,
          })}
        />
      ) : null}
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
                onClick={onProviderProbe}
                disabled={Boolean(queueAction) || !llmProviderAvailable}
              >
                {assistantT('testProvider')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={onRefreshQueue}
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
            <span className="panel-title">{assistantT('runningContext')}</span>
            <span className="panel-action">{llmProviderDisplay}</span>
          </div>
          <div className="panel-body intelligence-stack">
            <div className="intelligence-stat-row">
              <div className="summary-stat">
                <span className="dim">{profileScopeLabel}</span>
                <span className="mono">{profileScopeValue}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{assistantT('llm')}</span>
                <span className="mono">{llmProviderId}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{assistantT('retrieval')}</span>
                <span className="mono">{embeddingProviderId}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{assistantT('queuedLabel')}</span>
                <span className="mono">{queuedCount}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{assistantT('runningLabel')}</span>
                <span className="mono">{runningCount}</span>
              </div>
            </div>

            {providerProbe ? (
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
                      latency: providerProbe.latencyMs.toLocaleString(language),
                    })}
                  </span>
                </div>
                <p>{providerProbe.message}</p>
                {providerProbe.actionHint ? (
                  <p className="mono-support">{providerProbe.actionHint}</p>
                ) : null}
              </div>
            ) : null}

            {queuedAssistantJobs.length > 0 ? (
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
            ) : null}
          </div>
        </div>
      </div>
    </>
  )
}

interface AssistantQueueSidebarProps {
  assistantT: Translate
  queuedCount: number
  queueAction: string | null
  runningCount: number
}

/**
 * Renders the compact queue sidebar for the Assistant route.
 *
 * This keeps the route shell focused on orchestration while preserving the
 * existing queue progress grammar and copy.
 */
export function AssistantQueueSidebar({
  assistantT,
  queuedCount,
  queueAction,
  runningCount,
}: AssistantQueueSidebarProps) {
  return (
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
                queued: queuedCount,
                running: runningCount,
              })}
              progressValue={67}
            />
          ) : null}
        </div>
      </div>
    </aside>
  )
}
