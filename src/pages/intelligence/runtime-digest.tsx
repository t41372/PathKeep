/**
 * Compact runtime digest for the Intelligence route.
 *
 * Why this file exists:
 * - The main `/intelligence` route should show a small review surface for queue truth without duplicating the full Jobs page.
 * - Keeping the digest separate preserves the route shell's focus on scope/query state and keeps queue wording aligned with Jobs/sidebar.
 *
 * Main declarations:
 * - `IntelligenceRuntimeDigest`
 *
 * Source-of-truth notes:
 * - Keep the digest aligned with `docs/design/screens-and-nav.md` and `docs/features/intelligence.md`.
 * - This surface should stay a digest only; full retry/cancel/recovery review belongs on `/jobs`.
 */

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { backend } from '../../lib/backend-client'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n/hooks'
import {
  summarizeRuntimeJob,
  summarizeRuntimeJobError,
} from '../../lib/intelligence-presentation'
import type {
  AiQueueStatus,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'

interface IntelligenceRuntimeDigestProps {
  initialized: boolean
  unlocked: boolean
}

type DigestTone = 'info' | 'warning' | 'success'

function loadDelay(
  aiQueue: AiQueueStatus | null,
  runtime: IntelligenceRuntimeSnapshot | null,
) {
  const activeJobs =
    (aiQueue?.queued ?? 0) +
    (aiQueue?.running ?? 0) +
    (runtime?.queue.queued ?? 0) +
    (runtime?.queue.running ?? 0)

  return activeJobs > 0 ? 3000 : 15000
}

export function IntelligenceRuntimeDigest({
  initialized,
  unlocked,
}: IntelligenceRuntimeDigestProps) {
  const { language, ns } = useI18n()
  const intelligenceT = ns('intelligence')
  const jobsT = ns('jobs')
  const commonT = ns('common')
  const [aiQueue, setAiQueue] = useState<AiQueueStatus | null>(null)
  const [runtime, setRuntime] = useState<IntelligenceRuntimeSnapshot | null>(
    null,
  )
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!initialized || !unlocked) {
      return
    }

    let cancelled = false
    let timeoutId: number | null = null

    const scheduleNext = (delayMs: number) => {
      if (cancelled || typeof window === 'undefined') return
      timeoutId = window.setTimeout(() => {
        void load()
      }, delayMs)
    }

    const load = async () => {
      try {
        const [nextAiQueue, nextRuntime] = await Promise.all([
          backend.loadAiQueueStatus(),
          backend.loadIntelligenceRuntime(),
        ])

        if (cancelled) return
        setAiQueue(nextAiQueue)
        setRuntime(nextRuntime)
        setError(null)
        scheduleNext(loadDelay(nextAiQueue, nextRuntime))
      } catch (nextError) {
        if (cancelled) return
        setAiQueue(null)
        setRuntime(null)
        setError(
          nextError instanceof Error
            ? nextError.message
            : commonT('notAvailable'),
        )
        scheduleNext(15000)
      }
    }

    void load()

    return () => {
      cancelled = true
      if (timeoutId !== null && typeof window !== 'undefined') {
        window.clearTimeout(timeoutId)
      }
    }
  }, [commonT, initialized, unlocked])

  const digest = useMemo(() => {
    if (!initialized || !unlocked) {
      return {
        tone: 'info' as DigestTone,
        title: intelligenceT('runtimeDigestNeedsArchiveTitle'),
        body: intelligenceT('runtimeDigestNeedsArchiveBody'),
        jobDetail: null,
        meta: null,
      }
    }

    if (error) {
      return {
        tone: 'warning' as DigestTone,
        title: intelligenceT('runtimeDigestUnavailableTitle'),
        body: intelligenceT('runtimeDigestUnavailableBody'),
        jobDetail: error,
        meta: null,
      }
    }

    const queued = (aiQueue?.queued ?? 0) + (runtime?.queue.queued ?? 0)
    const running = (aiQueue?.running ?? 0) + (runtime?.queue.running ?? 0)
    const failed = (aiQueue?.failed ?? 0) + (runtime?.queue.failed ?? 0)
    const recentRuntimeJob = runtime?.recentJobs[0] ?? null
    const recentAiJob = aiQueue?.recentJobs[0] ?? null
    const lastActivityAt =
      runtime?.queue.lastActivityAt ??
      recentRuntimeJob?.updatedAt ??
      recentAiJob?.finishedAt ??
      recentAiJob?.startedAt ??
      recentAiJob?.queuedAt ??
      null

    const baseMeta = lastActivityAt
      ? intelligenceT('runtimeDigestLastActivity', {
          relative: formatRelativeTime(lastActivityAt, language),
        })
      : intelligenceT('runtimeDigestIdleMeta')

    if (failed > 0) {
      return {
        tone: 'warning' as DigestTone,
        title: intelligenceT('runtimeDigestFailedTitle', { count: failed }),
        body: intelligenceT('runtimeDigestFailedBody', {
          queued,
          running,
        }),
        jobDetail: recentRuntimeJob?.lastError
          ? summarizeRuntimeJobError(recentRuntimeJob, jobsT)
          : (recentAiJob?.summary ?? null),
        meta: baseMeta,
      }
    }

    if (running > 0) {
      return {
        tone: 'info' as DigestTone,
        title: intelligenceT('runtimeDigestRunningTitle', { count: running }),
        body: intelligenceT('runtimeDigestRunningBody', {
          queued,
        }),
        jobDetail: recentRuntimeJob
          ? summarizeRuntimeJob(recentRuntimeJob, jobsT)
          : (recentAiJob?.summary ?? null),
        meta: baseMeta,
      }
    }

    if (queued > 0) {
      return {
        tone: 'info' as DigestTone,
        title: intelligenceT('runtimeDigestQueuedTitle', { count: queued }),
        body: intelligenceT('runtimeDigestQueuedBody'),
        jobDetail: recentRuntimeJob?.title ?? recentAiJob?.summary ?? null,
        meta: baseMeta,
      }
    }

    return {
      tone: 'success' as DigestTone,
      title: intelligenceT('runtimeDigestReadyTitle'),
      body: intelligenceT('runtimeDigestReadyBody'),
      jobDetail: null,
      meta: baseMeta,
    }
  }, [
    aiQueue,
    error,
    initialized,
    intelligenceT,
    jobsT,
    language,
    runtime,
    unlocked,
  ])

  return (
    <section
      className="intelligence-section intelligence-runtime-digest"
      data-testid="intelligence-runtime-digest"
    >
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">
          {intelligenceT('runtimeDigestTitle')}
        </h2>
        <Link className="btn-tiny" to="/jobs">
          {jobsT('openJobs')}
        </Link>
      </div>
      <div className="intelligence-runtime-digest__summary">
        <div>
          <div
            className={`status-badge intelligence-runtime-digest__badge intelligence-runtime-digest__badge--${digest.tone}`}
          >
            {digest.title}
          </div>
          <p className="intelligence-runtime-digest__body">{digest.body}</p>
        </div>
        {digest.jobDetail ? (
          <p className="intelligence-runtime-digest__detail mono-support">
            {digest.jobDetail}
          </p>
        ) : null}
        {digest.meta ? (
          <p className="intelligence-runtime-digest__meta mono-support">
            {digest.meta}
          </p>
        ) : null}
      </div>
    </section>
  )
}
