/**
 * Shared scheduled-backup configuration options.
 *
 * ## Responsibilities
 * - Own the canonical interval options shown by Schedule and Onboarding.
 * - Keep route UI from hard-coding separate interval arrays.
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
 * The values are hours. Persisted configs may still contain other values, but
 * these are the supported first-class choices in the UI.
 */
export const scheduledBackupIntervalOptions = [6, 12, 24, 72] as const
