/**
 * Backup intervals as the user sees them. The config stores hours as a
 * float (`dueAfterHours`); the scheduler works in whole minutes, so every
 * value here is rounded to a minute, between one minute and 30 days.
 */
import type { Translator } from '@/lib/i18n'

export type IntervalUnit = 'minutes' | 'hours' | 'days'

export const MIN_INTERVAL_MINUTES = 1
/** Matches the backend's `MAX_DUE_AFTER_HOURS` (30 days). */
export const MAX_INTERVAL_MINUTES = 30 * 24 * 60

export const unitMinutes: Record<IntervalUnit, number> = {
  minutes: 1,
  hours: 60,
  days: 24 * 60,
}

export function hoursToMinutes(hours: number) {
  return Math.round(hours * 60)
}

/** The largest unit that divides the interval evenly: 90 min stays minutes, 48 h becomes 2 days. */
export function splitInterval(minutes: number): {
  amount: number
  unit: IntervalUnit
} {
  if (minutes >= unitMinutes.days && minutes % unitMinutes.days === 0)
    return { amount: minutes / unitMinutes.days, unit: 'days' }
  if (minutes >= unitMinutes.hours && minutes % unitMinutes.hours === 0)
    return { amount: minutes / unitMinutes.hours, unit: 'hours' }
  return { amount: minutes, unit: 'minutes' }
}

/** "90 minutes", "6 hours", "2 days". */
export function formatInterval(hours: number, t: Translator) {
  const { amount, unit } = splitInterval(hoursToMinutes(hours))
  return t(`backupSchedule.interval.${unit}`, { count: amount })
}

/** A valid interval in hours, or null when the input is out of range or not a whole number. */
export function intervalHours(amount: number, unit: IntervalUnit) {
  if (!Number.isInteger(amount)) return null
  const minutes = amount * unitMinutes[unit]
  if (minutes < MIN_INTERVAL_MINUTES || minutes > MAX_INTERVAL_MINUTES)
    return null
  return minutes / 60
}

/**
 * How often the system scheduler wakes PathKeep: the backup interval, capped
 * at the config's check interval (the same rule as the backend's
 * `native_schedule_interval_hours`).
 */
export function wakeHours(dueAfterHours: number, checkIntervalHours: number) {
  return Math.min(dueAfterHours, checkIntervalHours)
}
