/**
 * The plan the system scheduler would get for an interval, before anything
 * is installed. The backend builds plans from the saved config, so the
 * interval is saved first.
 */
import { useQuery } from '@tanstack/react-query'
import {
  frequencyHours,
  type Frequency,
} from '@/features/backup/schedule-actions'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { useSaveConfig } from '@/lib/queries/app'

/**
 * Keyed by the interval, so switching back and forth reuses earlier plans.
 * `active` keeps it from saving anything before the user reaches the step.
 */
export function useSchedulePreview(frequency: Frequency, active = true) {
  const save = useSaveConfig()
  const hours = frequency === 'off' ? null : frequencyHours[frequency]
  return useQuery({
    queryKey: ['onboarding', 'schedule-plan', hours],
    queryFn: async () => {
      await save.mutateAsync((config) => ({ ...config, dueAfterHours: hours! }))
      return scheduleClient.previewInstall()
    },
    enabled: active && hours !== null,
    staleTime: Infinity,
  })
}
