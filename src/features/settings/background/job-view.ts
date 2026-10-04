/**
 * One shape for a background job from either queue, so a single list can
 * show Insights work and search-index work the same way.
 *
 * Responsibilities: map backend job types, states and profile ids to
 * localized names; decide which actions the backend allows (from its own
 * `retryable` / `cancellable` flags for Insights jobs, and from the AI
 * queue's state rules for the others).
 *
 * Not responsible for rendering or for running the actions.
 */
import { useCallback } from 'react'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AiQueueJob, IntelligenceJobOverview } from '@/lib/types'

export const jobStates = [
  'queued',
  'running',
  'succeeded',
  'failed',
  'cancelled',
  'paused',
  'stale',
] as const
export type JobState = (typeof jobStates)[number]

const jobTypes = [
  'visit-derive',
  'daily-rollup',
  'structural-rebuild',
  'full-rebuild',
  'enrichment-plugin',
  'content-fetch',
  'index-build',
  'index-clear',
  'assistant',
] as const
type JobType = (typeof jobTypes)[number]

/** Job types that work on single pages and carry the page title. */
const pageJobTypes: readonly string[] = ['enrichment-plugin', 'content-fetch']

export interface JobView {
  key: string
  id: number
  name: string
  state: JobState | 'unknown'
  rawState: string
  /** The time that matches the state: when it was added, started or ended. */
  at: string | null
  progress: { percent: number | null; current?: number; total?: number } | null
  error: string | null
  canRetry: boolean
  canCancel: boolean
}

function isJobState(value: string): value is JobState {
  return (jobStates as readonly string[]).includes(value)
}

function isJobType(value: string): value is JobType {
  return (jobTypes as readonly string[]).includes(value)
}

function percentOf(current?: number | null, total?: number | null) {
  if (current == null || !total) return null
  return Math.min(100, Math.max(0, (current / total) * 100))
}

export function useJobViews() {
  const { t } = useI18n()
  const profiles = useSnapshot().browserProfiles

  const typeName = useCallback(
    (jobType: string) =>
      isJobType(jobType)
        ? t(`settingsBackground.job.${jobType}`)
        : t('settingsBackground.job.unknown'),
    [t],
  )

  const profileName = useCallback(
    (profileId?: string | null) => {
      if (!profileId) return t('settingsBackground.job.allBrowsers')
      const profile = profiles.find((item) => item.profileId === profileId)
      return profile
        ? `${profile.browserName} ${profile.profileName}`
        : profileId
    },
    [profiles, t],
  )

  const fromRuntime = useCallback(
    (job: IntelligenceJobOverview): JobView => {
      const name = pageJobTypes.includes(job.jobType)
        ? `${typeName(job.jobType)}: ${job.title ?? job.url ?? ''}`
        : `${profileName(job.profileId)} · ${typeName(job.jobType)}`
      const running = job.state === 'running'
      return {
        key: `runtime-${job.id}`,
        id: job.id,
        name,
        state: isJobState(job.state) ? job.state : 'unknown',
        rawState: job.state,
        at: running
          ? (job.startedAt ?? job.updatedAt)
          : job.state === 'queued'
            ? job.createdAt
            : (job.finishedAt ?? job.updatedAt),
        progress: running
          ? {
              percent:
                job.progressPercent ??
                percentOf(job.progressCurrent, job.progressTotal),
              current: job.progressCurrent ?? undefined,
              total: job.progressTotal ?? undefined,
            }
          : null,
        error: job.state === 'failed' ? (job.lastError ?? null) : null,
        canRetry: job.retryable,
        canCancel: job.cancellable,
      }
    },
    [profileName, typeName],
  )

  const fromAiQueue = useCallback(
    (job: AiQueueJob): JobView => {
      const running = job.state === 'running'
      const target = job.progressEmbedTarget ?? 0
      return {
        key: `ai-${job.id}`,
        id: job.id,
        name: typeName(job.jobType),
        state: isJobState(job.state) ? job.state : 'unknown',
        rawState: job.state,
        at: running
          ? (job.startedAt ?? job.queuedAt)
          : job.state === 'queued' || job.state === 'paused'
            ? job.queuedAt
            : (job.finishedAt ?? job.heartbeatAt ?? job.queuedAt),
        progress: running
          ? target > 0
            ? {
                percent: percentOf(job.progressEmbedded, target),
                current: job.progressEmbedded,
                total: target,
              }
            : { percent: null }
          : null,
        error:
          job.state === 'failed'
            ? (job.errorMessage ?? job.errorCode ?? null)
            : null,
        // `replay_ai_job` / `cancel_ai_job` state rules (vault-core ai_queue).
        canRetry: ['failed', 'cancelled', 'stale', 'paused'].includes(
          job.state,
        ),
        canCancel: ['queued', 'paused', 'stale', 'running'].includes(job.state),
      }
    },
    [typeName],
  )

  return { fromRuntime, fromAiQueue }
}
