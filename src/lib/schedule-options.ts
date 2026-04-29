/**
 * Shared scheduled-backup configuration options.
 *
 * ## Responsibilities
 * - Own the preset interval shortcuts shown by Schedule and Onboarding.
 * - Own custom minute-interval parsing so both routes reject the same unsafe values.
 * - Keep route UI from hard-coding separate preset arrays.
 *
 * ## Not responsible for
 * - Validating arbitrary persisted config values.
 * - Determining whether a platform can install a native schedule.
 * - Persisting config or applying scheduler artifacts.
 *
 * ## Dependencies
 * - None.
 *
 * ## Performance notes
 * - Static constants only; safe for hot render paths.
 */

/**
 * Provides the small set of user-facing scheduled-backup intervals shared by
 * setup and settings.
 *
 * The values are hours. Persisted configs may still contain other values,
 * including fractional hours entered through the minute-level custom input,
 * but these presets remain the quick choices beside that custom path.
 */
export const scheduledBackupIntervalOptions = [6, 12, 24, 72] as const

/**
 * Converts persisted hour values into whole minutes for the user-facing input.
 *
 * `dueAfterHours` remains the persisted app-config field, but the interaction
 * contract is minute-based so users can choose intervals smaller than one hour.
 *
 * @param hours persisted interval value in hours.
 * @returns a positive whole-minute representation for controls and labels.
 */
export function backupIntervalHoursToMinutes(hours: number): number {
  const minutes = Math.round(hours * 60)
  return Number.isSafeInteger(minutes) && minutes >= 1 ? minutes : 1
}

/**
 * Converts the custom minute input back to the legacy hour-valued config field.
 *
 * Keeping this conversion in one module makes it explicit that fractional hour
 * values are intentional and represent minute-level backup cadence.
 *
 * @param minutes validated positive whole minutes.
 * @returns the equivalent value for `dueAfterHours`.
 */
export function backupIntervalMinutesToHours(minutes: number): number {
  return minutes / 60
}

/**
 * Normalizes the custom interval text before schedule routes persist it.
 *
 * The values are whole minutes. Keeping this parser beside the preset list makes
 * the interval contract explicit without making React components own validation policy.
 *
 * @param rawValue user-entered minute text from the custom interval input.
 * @returns a positive whole number of minutes, or `null` when the text is not safe to persist.
 */
export function parseCustomBackupIntervalMinutes(
  rawValue: string,
): number | null {
  const trimmedValue = rawValue.trim()
  if (!/^\d+$/.test(trimmedValue)) return null
  const parsedValue = Number(trimmedValue)
  if (!Number.isSafeInteger(parsedValue) || parsedValue < 1) return null
  return parsedValue
}
