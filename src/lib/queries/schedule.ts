/** Native scheduler status and the "next backup" estimate built from it. */
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
 * When the scheduler will next run a backup. The scheduler wakes every
 * `checkIntervalHours` and backs up once the last success is older than
 * `dueAfterHours`, so this is an estimate, not a promise.
 */
export function nextScheduledBackup(
  status: ScheduleStatus | undefined,
  now = Date.now(),
): Date | null {
  if (!status || !scheduleInstalled(status)) return null
  const last = status.lastSuccessfulBackupAt
    ? Date.parse(status.lastSuccessfulBackupAt)
    : NaN
  const due = Number.isFinite(last) ? last + status.dueAfterHours * 3_600_000 : now
  if (due > now) return new Date(due)
  const step = Math.max(status.checkIntervalHours, 1) * 3_600_000
  return new Date(Math.ceil(now / step) * step)
}
