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
 * Shows Rust's estimate. When a cached time has passed, move it forward only
 * by whole native wake intervals, preserving its phase without redoing the due rule.
 */
export function nextScheduledBackup(
  status: ScheduleStatus | undefined,
  now = Date.now(),
): Date | null {
  if (!scheduleInstalled(status) || !status?.nextScheduledBackupAt) return null
  const next = Date.parse(status.nextScheduledBackupAt)
  const step = (status.wakeIntervalSeconds ?? 0) * 1_000
  if (!Number.isFinite(next) || !Number.isFinite(step) || step <= 0) return null
  if (next > now) return new Date(next)
  return new Date(next + (Math.floor((now - next) / step) + 1) * step)
}
