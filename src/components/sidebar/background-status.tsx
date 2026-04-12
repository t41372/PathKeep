/**
 * This module renders the compact background-work status strip shown in the sidebar footer.
 *
 * Why this file exists:
 * - Shared shell chrome should make long-running queue state visible without forcing users to hunt through Settings or Insights.
 * - Background work can outlive the foreground action that spawned it, so the sidebar needs a small always-on honesty surface.
 *
 * Main declarations:
 * - `SidebarBackgroundStatus`
 *
 * Source-of-truth notes:
 * - Keep queue language and recovery semantics aligned with `docs/features/intelligence.md`.
 * - Keep compact shell status behavior aligned with `docs/design/screens-and-nav.md`.
 */

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { backend } from '../../lib/backend-client'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type {
  AiQueueStatus,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'

interface SidebarBackgroundStatusProps {
  initialized: boolean
  refreshKey: number
}

export function SidebarBackgroundStatus({
  initialized,
  refreshKey,
}: SidebarBackgroundStatusProps) {
  const { language, ns } = useI18n()
  const jobsT = ns('jobs')
  const commonT = ns('common')
  const [aiQueue, setAiQueue] = useState<AiQueueStatus | null>(null)
  const [runtime, setRuntime] = useState<IntelligenceRuntimeSnapshot | null>(
    null,
  )
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!initialized) {
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
        const activeJobs =
          nextAiQueue.queued +
          nextAiQueue.running +
          nextRuntime.queue.queued +
          nextRuntime.queue.running
        scheduleNext(activeJobs > 0 ? 3000 : 15000)
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
  }, [commonT, initialized, refreshKey])

  const summary = useMemo(() => {
    if (!initialized) {
      return {
        label: jobsT('sidebarNeedsSetup'),
        detail: null,
        tone: 'idle',
        width: '0%',
        indeterminate: false,
      }
    }

    if (error) {
      return {
        label: jobsT('sidebarUnavailable'),
        detail: error,
        tone: 'warning',
        width: '100%',
        indeterminate: false,
      }
    }

    const queued = (aiQueue?.queued ?? 0) + (runtime?.queue.queued ?? 0)
    const running = (aiQueue?.running ?? 0) + (runtime?.queue.running ?? 0)
    const failed = (aiQueue?.failed ?? 0) + (runtime?.queue.failed ?? 0)
    const paused = aiQueue?.paused ?? false
    const runningRuntimeJob = runtime?.recentJobs.find(
      (job) =>
        job.state === 'running' && typeof job.progressPercent === 'number',
    )
    const activityTime =
      runtime?.queue.lastActivityAt ??
      aiQueue?.recentJobs.find((job) => job.finishedAt || job.startedAt)
        ?.finishedAt ??
      aiQueue?.recentJobs[0]?.queuedAt ??
      null

    if (paused && queued > 0) {
      return {
        label: jobsT('sidebarPaused', { queued }),
        detail: jobsT('sidebarOpenJobs'),
        tone: 'paused',
        width: '24%',
        indeterminate: false,
      }
    }

    if (running > 0) {
      if (
        runningRuntimeJob &&
        typeof runningRuntimeJob.progressPercent === 'number'
      ) {
        return {
          label: jobsT('sidebarRunning', { running, queued }),
          detail:
            runningRuntimeJob.progressDetail ??
            runningRuntimeJob.progressLabel ??
            jobsT('sidebarOpenJobs'),
          tone: 'running',
          width: `${Math.max(8, Math.min(100, runningRuntimeJob.progressPercent))}%`,
          indeterminate: false,
        }
      }
      return {
        label: jobsT('sidebarRunning', { running, queued }),
        detail: jobsT('sidebarOpenJobs'),
        tone: 'running',
        width: '55%',
        indeterminate: true,
      }
    }

    if (failed > 0) {
      return {
        label: jobsT('sidebarFailed', { failed }),
        detail: jobsT('sidebarOpenJobs'),
        tone: 'warning',
        width: '100%',
        indeterminate: false,
      }
    }

    if (queued > 0) {
      return {
        label: jobsT('sidebarQueued', { queued }),
        detail: jobsT('sidebarOpenJobs'),
        tone: 'queued',
        width: '28%',
        indeterminate: false,
      }
    }

    return {
      label: jobsT('sidebarIdle'),
      detail: activityTime
        ? jobsT('sidebarLastActivity', {
            relative: formatRelativeTime(activityTime, language),
          })
        : jobsT('sidebarIdleDetail'),
      tone: 'idle',
      width: '100%',
      indeterminate: false,
    }
  }, [aiQueue, error, initialized, jobsT, language, runtime])

  return (
    <div className="sidebar-background-status" data-tone={summary.tone}>
      <div className="sidebar-background-status__header">
        <span className="sidebar-background-status__label">
          {jobsT('sidebarTitle')}
        </span>
        <Link className="btn-tiny" to="/jobs">
          {jobsT('openJobs')}
        </Link>
      </div>
      <div className="sidebar-background-status__summary">{summary.label}</div>
      <div
        aria-hidden="true"
        className={`sidebar-background-status__track ${
          summary.indeterminate
            ? 'sidebar-background-status__track--indeterminate'
            : ''
        }`}
      >
        <span
          className="sidebar-background-status__fill"
          style={{ width: summary.width }}
        />
      </div>
      <div className="sidebar-background-status__detail">{summary.detail}</div>
    </div>
  )
}
