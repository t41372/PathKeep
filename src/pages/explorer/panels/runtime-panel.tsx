/**
 * This module renders a focused panel inside the Explorer route.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ExplorerRuntimePanel`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { Link } from 'react-router-dom'
import { ErrorState } from '../../../components/primitives/error-state'
import { LoadingState } from '../../../components/primitives/loading-state'
import { StatusCallout } from '../../../components/primitives/status-callout'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { IntelligenceTone } from '../../../lib/intelligence'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiQueueStatus,
} from '../../../lib/types'
import type { Translator } from '../types'

/**
 * Describes the props accepted by `ExplorerRuntimePanel`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ExplorerRuntimePanelProps {
  aiMeta: {
    description: string
    label: string
    tone: IntelligenceTone
  }
  embeddingProvider: AiProviderConfig | null
  explorerT: Translator
  indexAction: string | null
  intelligenceError: string | null
  language: ResolvedLanguage
  onBuildIndex: () => void
  onCancelJob: (jobId: number) => void
  onClearIndex: () => void
  onDrainQueue: () => void
  onFullRebuild: () => void
  onReplayJob: (jobId: number) => void
  onRefreshQueue: () => void
  onTestProvider: () => void
  providerProbe: AiProviderConnectionTestReport | null
  queueAction: string | null
  queueStatus: AiQueueStatus | null
  snapshotAiStatus: AiIndexStatus
}

/**
 * Renders the explorer runtime panel.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function ExplorerRuntimePanel({
  aiMeta,
  embeddingProvider,
  explorerT,
  indexAction,
  intelligenceError,
  language,
  onBuildIndex,
  onCancelJob,
  onClearIndex,
  onDrainQueue,
  onFullRebuild,
  onReplayJob,
  onRefreshQueue,
  onTestProvider,
  providerProbe,
  queueAction,
  queueStatus,
  snapshotAiStatus,
}: ExplorerRuntimePanelProps) {
  return (
    <>
      {intelligenceError ? (
        <ErrorState
          title={explorerT('semanticRecallDegradedTitle')}
          description={intelligenceError}
        />
      ) : null}

      <div className="intelligence-grid intelligence-grid--explorer">
        <StatusCallout
          tone={aiMeta.tone}
          eyebrow={explorerT('semanticStatusEyebrow')}
          title={aiMeta.label}
          body={aiMeta.description}
          actions={
            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={onBuildIndex}
                disabled={Boolean(indexAction) || !embeddingProvider}
              >
                {explorerT('buildIndex')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={onFullRebuild}
                disabled={Boolean(indexAction) || !embeddingProvider}
              >
                {explorerT('fullRebuild')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={onClearIndex}
                disabled={Boolean(indexAction) || !embeddingProvider}
              >
                {explorerT('clearIndex')}
              </button>
              <Link className="btn-secondary" to="/settings">
                {explorerT('openSettings')}
              </Link>
            </div>
          }
        />

        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">
              {explorerT('providerQueueTitle')}
            </span>
            <span className="panel-action">
              {embeddingProvider
                ? `${embeddingProvider.name} / ${embeddingProvider.defaultModel}`
                : explorerT('noEmbeddingProviderSelected')}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            <div className="intelligence-stat-row">
              <div className="summary-stat">
                <span className="dim">{explorerT('queueQueued')}</span>
                <span className="mono">
                  {queueStatus?.queued ?? snapshotAiStatus.queuedJobs}
                </span>
              </div>
              <div className="summary-stat">
                <span className="dim">{explorerT('queueRunning')}</span>
                <span className="mono">
                  {queueStatus?.running ?? snapshotAiStatus.runningJobs}
                </span>
              </div>
              <div className="summary-stat">
                <span className="dim">{explorerT('queueFailed')}</span>
                <span className="mono">
                  {queueStatus?.failed ?? snapshotAiStatus.failedJobs}
                </span>
              </div>
              <div className="summary-stat">
                <span className="dim">{explorerT('queueState')}</span>
                <span className="mono">
                  {(queueStatus?.paused ?? snapshotAiStatus.queuePaused)
                    ? explorerT('queueStatePaused')
                    : explorerT('queueStateLive')}
                </span>
              </div>
            </div>

            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={onRefreshQueue}
                disabled={Boolean(queueAction)}
              >
                {explorerT('refreshQueue')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={onDrainQueue}
                disabled={Boolean(queueAction)}
              >
                {explorerT('drainQueue')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={onTestProvider}
                disabled={Boolean(queueAction) || !embeddingProvider}
              >
                {explorerT('testProvider')}
              </button>
            </div>

            {indexAction || queueAction ? (
              <LoadingState
                compact
                label={
                  indexAction ?? queueAction ?? explorerT('preparingRecall')
                }
                detail={explorerT('semanticRecallNeedsAttentionBody')}
                progressLabel={explorerT('queueProgressLabel', {
                  queued: (
                    queueStatus?.queued ?? snapshotAiStatus.queuedJobs
                  ).toLocaleString(language),
                  running: (
                    queueStatus?.running ?? snapshotAiStatus.runningJobs
                  ).toLocaleString(language),
                })}
                progressValue={indexAction ? 50 : 75}
              />
            ) : null}

            {providerProbe && (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>
                    {providerProbe.ok
                      ? explorerT('providerReachable')
                      : explorerT('providerNeedsAttention')}
                  </strong>
                  <span className="mono-support">
                    {explorerT('providerProbeLatency', {
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
            )}

            <div className="intelligence-job-list">
              {(queueStatus?.recentJobs ?? snapshotAiStatus.recentJobs).map(
                (job) => (
                  <div key={job.id} className="result-row">
                    <div className="result-row__header">
                      <strong>
                        {job.jobType} · #{job.id}
                      </strong>
                      <span className="mono-support">{job.state}</span>
                    </div>
                    <p>
                      {job.summary ??
                        job.errorMessage ??
                        explorerT('noJobSummary')}
                    </p>
                    <div className="intelligence-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => onReplayJob(job.id)}
                        disabled={
                          Boolean(queueAction) ||
                          !['failed', 'cancelled', 'stale', 'paused'].includes(
                            job.state,
                          )
                        }
                      >
                        {explorerT('replayJob')}
                      </button>
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => onCancelJob(job.id)}
                        disabled={
                          Boolean(queueAction) || job.state === 'running'
                        }
                      >
                        {explorerT('cancelJob')}
                      </button>
                    </div>
                  </div>
                ),
              )}
            </div>
          </div>
        </div>
      </div>
    </>
  )
}
