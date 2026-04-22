/**
 * @file helpers.ts
 * @description Holds the pure helper contracts behind the Security route so the route shell can focus on state, effects, and action handlers.
 * @module pages/security
 *
 * ## Responsibilities
 * - Keep warning localization explicit and testable.
 * - Define the small route-local types reused by split Security owners.
 *
 * ## Not responsible for
 * - Fetching security posture or executing unlock/rekey actions
 * - Rendering Security route panels
 *
 * ## Dependencies
 * - Depends only on Security front-end contract types.
 *
 * ## Performance notes
 * - Pure helper module only; keeping these transforms side-effect free makes unlock/rekey flows cheap to recompute.
 */

import type { SecurityStatus } from '../../lib/types'

/**
 * Captures the shell-facing load state for the Security route without forcing the route to juggle raw nullable fields everywhere.
 */
export interface SecurityLoadState {
  status: SecurityStatus | null
  error: string | null
}

/**
 * Documents the translator shape reused by Security helper and panel owners.
 */
export type SecurityTranslate = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Converts backend-originated warning strings into shipped Security copy so users never see raw English transport text.
 *
 * This exists because the backend still reports a few stable warning messages in English. The route needs one
 * honest mapping layer that preserves unknown warnings instead of hiding them, while keeping the known trust-
 * critical cases localized and user-facing.
 */
export function localizeSecurityWarning(
  warning: string,
  t: SecurityTranslate,
): string {
  const normalizedWarning = warning.trim()

  switch (normalizedWarning) {
    case 'database key is required for encrypted archives':
      return t('security.encryptedArchiveNeedsPasswordWarning')
    case 'Archive is configured to remember the database key, but no native keyring backend is available on this machine.':
      return t('security.rememberKeyNeedsKeychainWarning')
    case 'Archive is encrypted, but the database key is not currently stored in the system keyring.':
      return t('security.rememberedKeyMissingWarning')
  }

  if (
    normalizedWarning.includes(
      'database key is required for encrypted archives',
    )
  ) {
    return t('security.encryptedArchiveNeedsPasswordWarning')
  }
  if (
    normalizedWarning.includes(
      'no native keyring backend is available on this machine',
    )
  ) {
    return t('security.rememberKeyNeedsKeychainWarning')
  }
  if (
    normalizedWarning.includes(
      'database key is not currently stored in the system keyring',
    )
  ) {
    return t('security.rememberedKeyMissingWarning')
  }

  return warning
}
