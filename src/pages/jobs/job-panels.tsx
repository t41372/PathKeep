/**
 * @file job-panels.tsx
 * @description Render-only recent AI job and runtime job panels for the Jobs route.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Render the recent AI queue job list with truthful retry/cancel affordances.
 * - Render the recent deterministic/runtime job list with progress and error details.
 * - Keep job-row status labeling close to the panels that actually display it.
 *
 * ## Not responsible for
 * - Fetching runtime or queue state.
 * - Saving config or mutating queue state directly.
 * - Rendering the runtime health / plugin / module summary cards.
 *
 * ## Dependencies
 * - Depends on shared `ReviewSection` review grammar and runtime presentation helpers.
 * - Depends on Jobs and Settings translators for truthful state labels.
 *
 * ## Performance notes
 * - Render-only panels that work from already-loaded queue snapshots; they do not trigger additional reads.
 */

import { ReviewSection } from '../../components/review'
import { formatDateTime, formatRelativeTime } from '../../lib/format'
import {
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
} from '../../lib/intelligence-runtime'
import { summarizeRuntimeJob } from '../../lib/intelligence-presentation'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { AiQueueJob, IntelligenceJobOverview } from '../../lib/types'
import { aiJobStateLabel, type JobsTranslator } from './job-panel-helpers'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

interface JobPanelProps {
  action: string | null
  emptyLabel: string
  jobs: AiQueueJob[]
  jobsT: JobsTranslator
  language: ResolvedLanguage
  noDetailsLabel: string
  onCancel: (jobId: number) => Promise<void>
  onRetry: (jobId: number) => Promise<void>
  title: string
}

/**
 * Renders the recent AI queue jobs panel without pulling queue mutations into
 * the route shell itself.
 */
export function JobPanel({
  action,
  emptyLabel,
  jobs,
  jobsT,
  language,
  noDetailsLabel,
  onCancel,
  onRetry,
  title,
}: JobPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{title}</span>
      </div>
      <div className="panel-body intelligence-job-list">
        {jobs.length === 0 ? (
          <p className="mono-support">{emptyLabel}</p>
        ) : (
          jobs.map((job) => (
            <ReviewSection
              key={job.id}
              headerMeta={
                <span className="mono-support">
                  {aiJobStateLabel(job.state, jobsT)}
                </span>
              }
              title={
                <>
                  {job.jobType} · #{job.id}
                </>
              }
            >
              <p>{job.summary ?? job.errorMessage ?? noDetailsLabel}</p>
              <div className="jobs-meta-grid mono-support">
                <span>
                  {jobsT('createdAt')}:{' '}
                  {formatDateTime(job.queuedAt, language) ?? job.queuedAt}
                </span>
                <span>
                  {jobsT('startedAt')}:{' '}
                  {job.startedAt
                    ? formatDateTime(job.startedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('finishedAt')}:{' '}
                  {job.finishedAt
                    ? formatDateTime(job.finishedAt, language)
                    : '—'}
                </span>
              </div>
              <div className="intelligence-actions">
                {['failed', 'cancelled', 'paused', 'stale'].includes(
                  job.state,
                ) ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onRetry(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('retryJob')}
                  </button>
                ) : null}
                {['queued', 'paused', 'stale'].includes(job.state) ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onCancel(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('cancelJob')}
                  </button>
                ) : null}
              </div>
            </ReviewSection>
          ))
        )}
      </div>
    </div>
  )
}

interface RuntimeJobPanelProps {
  action: string | null
  emptyLabel: string
  jobs: IntelligenceJobOverview[]
  jobsT: JobsTranslator
  language: ResolvedLanguage
  onCancel: (jobId: number) => Promise<void>
  onRetry: (jobId: number) => Promise<void>
  settingsT: Translator
  title: string
}

/**
 * Renders the recent deterministic/runtime jobs panel while keeping retry and
 * cancel affordances visible next to progress/error context.
 */
export function RuntimeJobPanel({
  action,
  emptyLabel,
  jobs,
  jobsT,
  language,
  onCancel,
  onRetry,
  settingsT,
  title,
}: RuntimeJobPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{title}</span>
      </div>
      <div className="panel-body intelligence-job-list">
        {jobs.length === 0 ? (
          <p className="mono-support">{emptyLabel}</p>
        ) : (
          jobs.map((job) => (
            <ReviewSection
              key={job.id}
              headerMeta={
                <span className="mono-support">
                  {intelligenceRuntimeJobStateLabel(job.state, settingsT)}
                </span>
              }
              title={
                <>
                  {(job.pluginId
                    ? enrichmentPluginLabel(job.pluginId, settingsT)
                    : job.jobType) || job.jobType}{' '}
                  · #{job.id}
                </>
              }
            >
              <p>{summarizeRuntimeJob(job, jobsT)}</p>
              {job.lastError &&
              summarizeRuntimeJob(job, jobsT) !== job.lastError ? (
                <p className="mono-support">{job.lastError}</p>
              ) : null}
              {typeof job.progressPercent === 'number' ? (
                <div className="jobs-progress">
                  <div aria-hidden="true" className="jobs-progress__track">
                    <span
                      className="jobs-progress__fill"
                      style={{
                        width: `${Math.max(
                          4,
                          Math.min(100, job.progressPercent),
                        )}%`,
                      }}
                    />
                  </div>
                  <div className="jobs-progress__meta mono-support">
                    <span>{Math.round(job.progressPercent)}%</span>
                    <span>{job.progressLabel ?? jobsT('runningCount')}</span>
                  </div>
                </div>
              ) : null}
              {job.progressDetail ? (
                <p className="mono-support">{job.progressDetail}</p>
              ) : null}
              <div className="jobs-meta-grid mono-support">
                <span>
                  {jobsT('createdAt')}:{' '}
                  {formatDateTime(job.createdAt, language) ?? job.createdAt}
                </span>
                <span>
                  {jobsT('startedAt')}:{' '}
                  {job.startedAt
                    ? formatDateTime(job.startedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('finishedAt')}:{' '}
                  {job.finishedAt
                    ? formatDateTime(job.finishedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('lastActivity')}:{' '}
                  {job.heartbeatAt
                    ? formatRelativeTime(job.heartbeatAt, language)
                    : formatRelativeTime(job.updatedAt, language)}
                </span>
              </div>
              <div className="intelligence-actions">
                {job.retryable ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onRetry(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('retryJob')}
                  </button>
                ) : null}
                {job.cancellable ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onCancel(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('cancelJob')}
                  </button>
                ) : null}
              </div>
            </ReviewSection>
          ))
        )}
      </div>
    </div>
  )
}
