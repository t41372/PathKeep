/**
 * This module centralizes the archive-access refusal grammar used across the shell.
 *
 * Why this file exists:
 * - Multiple shell surfaces need to distinguish "archive exists but needs an unlock step" from true generic failures.
 * - Keeping the message matching in one place prevents the dashboard, settings, and shell chrome from drifting apart.
 *
 * Main declarations:
 * - `isArchiveUnlockRequiredError`
 * - `isArchiveUnlockRequiredMessage`
 *
 * Source-of-truth notes:
 * - These helpers should stay aligned with the backend error language used by the archive and worker layers.
 * - If the refusal grammar changes, update this helper and the route tests together.
 */

const ARCHIVE_UNLOCK_REQUIRED_PATTERN =
  /database key is required for encrypted archives|encrypted archive requires an active session key/i

/**
 * Returns whether an error object represents a missing archive session key.
 *
 * The shell uses this to route users toward the Security unlock surface
 * instead of pretending the archive is uninitialized or permanently broken.
 */
export function isArchiveUnlockRequiredError(error: unknown) {
  return error instanceof Error && isArchiveUnlockRequiredMessage(error.message)
}

/**
 * Returns whether a message string represents a missing archive session key.
 *
 * This string-level variant lets already-rendered shell state reuse the same
 * detection logic without reconstructing an `Error` instance.
 */
export function isArchiveUnlockRequiredMessage(
  message: string | null | undefined,
) {
  return (
    typeof message === 'string' && ARCHIVE_UNLOCK_REQUIRED_PATTERN.test(message)
  )
}
