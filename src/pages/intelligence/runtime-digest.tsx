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

import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n/hooks'
import {
  summarizeRuntimeJob,
  summarizeRuntimeJobError,
} from '../../lib/intelligence-presentation'

interface IntelligenceRuntimeDigestProps {
  initialized: boolean
  unlocked: boolean
}

type DigestTone = 'info' | 'warning' | 'success'

export function IntelligenceRuntimeDigest({
  initialized,
  unlocked,
}: IntelligenceRuntimeDigestProps) {
  const {
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
  } = useShellData()
  const { language, ns } = useI18n()
  const intelligenceT = ns('intelligence')
  const jobsT = ns('jobs')

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

    if (runtimeStatus.error) {
      return {
        tone: 'warning' as DigestTone,
        title: intelligenceT('runtimeDigestUnavailableTitle'),
        body: intelligenceT('runtimeDigestUnavailableBody'),
        jobDetail: runtimeStatus.error,
        meta: null,
      }
    }

    const queued =
      (runtimeStatus.intelligence?.queue.queued ?? 0) +
      (runtimeStatus.aiQueue?.queued ?? 0)
    const running =
      (runtimeStatus.intelligence?.queue.running ?? 0) +
      (runtimeStatus.aiQueue?.running ?? 0)
    const failed =
      (runtimeStatus.intelligence?.queue.failed ?? 0) +
      (runtimeStatus.aiQueue?.failed ?? 0)
    const recentRuntimeJob = runtimeStatus.intelligence?.recentJobs[0] ?? null
    const lastActivityAt =
      runtimeStatus.intelligence?.queue.lastActivityAt ??
      recentRuntimeJob?.updatedAt ??
      null

    const baseMeta = lastActivityAt
      ? intelligenceT('runtimeDigestLastActivity', {
          relative: formatRelativeTime(lastActivityAt, language),
        })
      : runtimeStatus.loading
        ? jobsT('runningCount')
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
          : null,
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
          : null,
        meta: baseMeta,
      }
    }

    if (queued > 0) {
      return {
        tone: 'info' as DigestTone,
        title: intelligenceT('runtimeDigestQueuedTitle', { count: queued }),
        body: intelligenceT('runtimeDigestQueuedBody'),
        jobDetail: recentRuntimeJob?.title ?? null,
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
  }, [initialized, intelligenceT, jobsT, language, runtimeStatus, unlocked])

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
