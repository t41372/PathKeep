/**
 * One shape for a background job from either queue, so a single list can
 * show Insights work and search-index work the same way.
 *
 * Responsibilities: map backend job types, states and profile ids to
 * localized names; say who paused a paused index build; decide which actions
 * the backend allows (from its own `retryable` / `cancellable` flags for
 * Insights jobs, and from the AI queue's state rules for the others, where an
 * index build can also be paused and a paused one continued).
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

/**
 * Why a question an older Ask left in the queue ended unanswered
 * (`vault_core::ai_queue::ASSISTANT_RETIRED`).
 */
const ASSISTANT_RETIRED = 'assistant-retired'

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
  /** Who paused it and why, for a paused index build. */
  note: { text: string; problem: boolean } | null
  canRetry: boolean
  /** Retrying it goes on from where it stopped (a paused index build). */
  resumes: boolean
  canCancel: boolean
  canPause: boolean
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
        note: null,
        canRetry: job.retryable,
        resumes: false,
        canCancel: job.cancellable,
        canPause: false,
      }
    },
    [profileName, typeName],
  )

  const fromAiQueue = useCallback(
    (job: AiQueueJob): JobView => {
      const running = job.state === 'running'
      const target = job.progressEmbedTarget ?? 0
      const build = job.jobType === 'index-build'
      const pausedBy = job.state === 'paused' ? job.pausedBy : null
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
        progress:
          running || (build && job.state === 'paused')
            ? target > 0
              ? {
                  percent: percentOf(job.progressEmbedded, target),
                  current: job.progressEmbedded,
                  total: target,
                }
              : { percent: null }
            : null,
        error:
          job.errorCode === ASSISTANT_RETIRED
            ? t('settingsBackground.assistantRetired')
            : job.state === 'failed'
              ? (job.errorMessage ?? job.errorCode ?? null)
              : null,
        note: pausedBy
          ? {
              text: t(`settingsBackground.ai.pausedBy.${pausedBy}`, {
                message: job.errorMessage ?? '',
              }),
              problem: pausedBy === 'service',
            }
          : null,
        // `replay_ai_job` / `cancel_ai_job` / `pause_ai_job` state rules
        // (vault-core ai_queue); a question an older Ask queued is never run
        // again, and one paused with all background work goes on with it.
        canRetry:
          job.jobType !== 'assistant' &&
          pausedBy !== 'queue' &&
          ['failed', 'cancelled', 'stale', 'paused'].includes(job.state),
        resumes: build && job.state === 'paused',
        canCancel: ['queued', 'paused', 'stale', 'running'].includes(job.state),
        canPause: build && ['queued', 'stale', 'running'].includes(job.state),
      }
    },
    [t, typeName],
  )

  return { fromRuntime, fromAiQueue }
}
