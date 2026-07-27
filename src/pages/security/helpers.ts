/**
 * @file helpers.ts
 * @description Holds the pure helper contracts behind the Security route so the route shell can focus on state, effects, and action handlers.
 * @module pages/security
 *
 * ## Responsibilities
 * - Keep code-driven warning localization explicit and testable.
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
 * Maps `SECURITY_WARNING_*` and `REKEY_WARNING_*` codes from
 * `vault-core/src/models/security.rs` onto shipped Security copy.
 *
 * The lookup is keyed by stable code — never by backend prose — so editing an
 * English sentence in Rust can no longer silently degrade zh copy to English.
 */
const SECURITY_WARNING_KEY_BY_CODE: Record<string, string> = {
  'encrypted-needs-password': 'security.encryptedArchiveNeedsPasswordWarning',
  'remember-key-no-keyring': 'security.rememberKeyNeedsKeychainWarning',
  'remembered-key-missing': 'security.rememberedKeyMissingWarning',
  'archive-locked': 'security.rekeyArchiveLockedWarning',
  'new-key-required': 'security.rekeyNewKeyRequiredWarning',
  'same-mode-rewrite': 'security.rekeySameModeRewriteWarning',
}

/**
 * Converts one coded backend warning into shipped Security copy.
 *
 * This exists because the backend still reports warning prose in English for
 * diagnostics. `code` is the stable identity the route localizes against;
 * `rawWarning` stays the honest fallback so an uncoded or newer warning is
 * surfaced verbatim instead of disappearing.
 */
export function localizeSecurityWarning(
  code: string | undefined,
  rawWarning: string,
  t: SecurityTranslate,
): string {
  const key = code ? SECURITY_WARNING_KEY_BY_CODE[code] : undefined
  return key ? t(key) : rawWarning
}
