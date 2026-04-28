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

import { useMemo } from 'react'
import { Link } from 'react-router-dom'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import type { ShellRuntimeStatus } from '../../app/shell-data-context'

interface SidebarBackgroundStatusProps {
  initialized: boolean
  unlocked: boolean
  runtimeStatus: ShellRuntimeStatus
}

export function SidebarBackgroundStatus({
  initialized,
  unlocked,
  runtimeStatus,
}: SidebarBackgroundStatusProps) {
  const { language, ns } = useI18n()
  const jobsT = ns('jobs')

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

    if (!unlocked) {
      return {
        label: jobsT('sidebarLocked'),
        detail: jobsT('sidebarLockedDetail'),
        tone: 'warning',
        width: '100%',
        indeterminate: false,
      }
    }

    if (runtimeStatus.error) {
      return {
        label: jobsT('sidebarUnavailable'),
        detail: runtimeStatus.error,
        tone: 'warning',
        width: '100%',
        indeterminate: false,
      }
    }

    const queued =
      (runtimeStatus.aiQueue?.queued ?? 0) +
      (runtimeStatus.intelligence?.queue.queued ?? 0)
    const running =
      (runtimeStatus.aiQueue?.running ?? 0) +
      (runtimeStatus.intelligence?.queue.running ?? 0)
    const failed =
      (runtimeStatus.aiQueue?.failed ?? 0) +
      (runtimeStatus.intelligence?.queue.failed ?? 0)
    const paused = runtimeStatus.aiQueue?.paused ?? false
    const runningRuntimeJob = runtimeStatus.intelligence?.recentJobs.find(
      (job) =>
        job.state === 'running' && typeof job.progressPercent === 'number',
    )
    const latestAiJobWithActivity = runtimeStatus.aiQueue?.recentJobs.find(
      (job) => job.finishedAt || job.startedAt,
    )
    const activityTime =
      runtimeStatus.intelligence?.queue.lastActivityAt ??
      latestAiJobWithActivity?.finishedAt ??
      latestAiJobWithActivity?.startedAt ??
      runtimeStatus.aiQueue?.recentJobs[0]?.queuedAt ??
      null

    if (
      runtimeStatus.loading &&
      queued === 0 &&
      running === 0 &&
      failed === 0
    ) {
      return {
        label: jobsT('sidebarOpenJobs'),
        detail: jobsT('runningCount'),
        tone: 'queued',
        width: '28%',
        indeterminate: true,
      }
    }

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
  }, [initialized, jobsT, language, runtimeStatus, unlocked])

  const actionTarget =
    initialized && !unlocked ? '/security#unlock-archive' : '/jobs'
  const actionLabel =
    initialized && !unlocked ? jobsT('sidebarOpenSecurity') : jobsT('openJobs')

  return (
    <div className="sidebar-background-status" data-tone={summary.tone}>
      <div className="sidebar-background-status__header">
        <span className="sidebar-background-status__label">
          {jobsT('sidebarTitle')}
        </span>
        <Link className="btn-tiny" to={actionTarget}>
          {actionLabel}
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
