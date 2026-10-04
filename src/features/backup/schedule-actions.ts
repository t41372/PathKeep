/**
 * Data behind the Automatic backup card: the plan for an interval before
 * anything is installed, and the three changes (install / update, turn off,
 * remove an old job). Each change applies the plan the user was shown.
 *
 * Not responsible for copy or layout (`schedule-card.tsx`,
 * `schedule-dialog.tsx`).
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { scheduleClient } from '@/lib/backend-client/schedule'
import { queryKeys } from '@/lib/query'
import { useSaveConfig } from '@/lib/queries/app'
import type { ApplyResult, SchedulePlan } from '@/lib/types'

/** The presets on the card. Onboarding uses the same four. */
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

/** Removal counts as done unless a step failed: "nothing was installed" is fine. */
function assertNoFailedStep(result: ApplyResult) {
  if (result.stepResults?.some((step) => step.status === 'error'))
    throw new Error(result.message)
  return result
}

/**
 * The exact plan the scheduler would get for `hours`, without saving
 * anything. Keyed by interval so switching back and forth reuses plans.
 */
export function useSchedulePlan(hours: number | null) {
  return useQuery({
    queryKey: ['schedule-plan', hours],
    queryFn: () => scheduleClient.previewInstall(undefined, hours ?? undefined),
    enabled: hours !== null,
    staleTime: 30_000,
  })
}

/** The plan for the saved interval, for turning off and removing old jobs. */
export function useCurrentSchedulePlan(enabled: boolean) {
  return useQuery({
    queryKey: ['schedule-plan', 'saved'],
    queryFn: () => scheduleClient.previewInstall(),
    enabled,
    staleTime: 0,
  })
}

/**
 * Saves the interval, then installs the plan that was shown for it. Where
 * PathKeep cannot install (Linux), only the interval is saved.
 */
export function useApplySchedule() {
  const save = useSaveConfig()
  const client = useQueryClient()
  return useMutation({
    mutationFn: async ({
      hours,
      plan,
    }: {
      hours: number
      plan: SchedulePlan
    }) => {
      await save.mutateAsync((config) => ({ ...config, dueAfterHours: hours }))
      if (plan.applySupported)
        assertApplied(await scheduleClient.applyInstall(plan))
    },
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.schedule }),
  })
}

/** Removes the installed job. The saved interval stays for next time. */
export function useRemoveSchedule() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: async (plan: SchedulePlan) =>
      assertNoFailedStep(await scheduleClient.removeInstall(plan)),
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.schedule }),
  })
}

/** Removes jobs left by earlier versions (macOS only; the backend checks). */
export function useRepairLegacySchedule() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: async (plan: SchedulePlan) =>
      assertApplied(await scheduleClient.repairInstall(plan)),
    onSettled: () => client.invalidateQueries({ queryKey: queryKeys.schedule }),
  })
}
