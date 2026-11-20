/** Native scheduler status and presentation of its backend backup estimate. */
import { useQuery } from '@tanstack/react-query'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { queryKeys } from '@/lib/query'
import type { ScheduleStatus } from '@/lib/types'

export function useScheduleStatus() {
  return useQuery({
    queryKey: queryKeys.schedule,
    queryFn: () => scheduleClient.getStatus(),
    staleTime: 60_000,
  })
}

export function scheduleInstalled(status: ScheduleStatus | undefined) {
  return status?.installState === 'installed'
}

/**
 * Shows Rust's estimate for one kind's next scheduled backup: browsing
 * history's, or Agent conversations' (their own clock, from their last
 * finished backup; absent while they are not backed up on a schedule).
 * When a cached time has passed, move it forward only by whole native wake
 * intervals, preserving its phase without redoing the due rule.
 */
export function nextScheduledBackup(
  status: ScheduleStatus | undefined,
  now = Date.now(),
  kind: 'browsing' | 'agent-history' = 'browsing',
): Date | null {
  const estimate =
    kind === 'browsing'
      ? status?.nextScheduledBackupAt
      : status?.agentNextScheduledBackupAt
  if (!scheduleInstalled(status) || !estimate) return null
  const next = Date.parse(estimate)
  const step = (status?.wakeIntervalSeconds ?? 0) * 1_000
  if (!Number.isFinite(next) || !Number.isFinite(step) || step <= 0) return null
  if (next > now) return new Date(next)
  return new Date(next + (Math.floor((now - next) / step) + 1) * step)
}

/** The next scheduled backup of any kind: the earlier of the two. */
export function nextScheduledBackupOfAny(
  status: ScheduleStatus | undefined,
  now = Date.now(),
): Date | null {
  const times = [
    nextScheduledBackup(status, now, 'browsing'),
    nextScheduledBackup(status, now, 'agent-history'),
  ].filter((time): time is Date => time !== null)
  if (times.length === 0) return null
  return new Date(Math.min(...times.map((time) => time.getTime())))
}
