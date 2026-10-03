/**
 * Mutations behind the Automatic backup card: pick a frequency, repair the
 * native scheduler. Both follow the same order — save config, preview the
 * plan, apply it — so the installed job always matches the saved interval.
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { queryKeys } from '@/lib/query'
import { useSaveConfig } from '@/lib/queries/app'
import type { ApplyResult } from '@/lib/types'

export type Frequency = 'hourly' | 'sixHours' | 'daily' | 'off'

export const frequencyHours: Record<Exclude<Frequency, 'off'>, number> = {
  hourly: 1,
  sixHours: 6,
  daily: 24,
}

export function frequencyForHours(
  hours: number,
): Exclude<Frequency, 'off'> | null {
  const match = Object.entries(frequencyHours).find(
    ([, value]) => value === hours,
  )
  return (match?.[0] as Exclude<Frequency, 'off'> | undefined) ?? null
}

function assertApplied(result: ApplyResult) {
  if (!result.applied) throw new Error(result.message)
  return result
}

/** The backend wakes the scheduler every min(dueAfterHours, checkIntervalHours), so only dueAfterHours is saved. */
export function useChangeFrequency(applySupported: boolean) {
  const save = useSaveConfig()
  const client = useQueryClient()
  return useMutation({
    mutationFn: async (frequency: Frequency) => {
      if (frequency !== 'off') {
        await save.mutateAsync((config) => ({
          ...config,
          dueAfterHours: frequencyHours[frequency],
        }))
      }
      if (!applySupported) return
      const plan = await scheduleClient.previewInstall()
      assertApplied(
        frequency === 'off'
          ? await scheduleClient.removeInstall(plan)
          : await scheduleClient.applyInstall(plan),
      )
    },
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.schedule }),
  })
}

export function useRepairSchedule() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: async () => {
      const plan = await scheduleClient.previewInstall()
      // Only macOS has a dedicated repair path; elsewhere a fresh install replaces the job.
      assertApplied(
        plan.platform === 'macos'
          ? await scheduleClient.repairInstall(plan)
          : await scheduleClient.applyInstall(plan),
      )
    },
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.schedule }),
  })
}
