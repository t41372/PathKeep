/**
 * This module explains browser-managed retention boundaries so the UI can stay honest about what PathKeep has or has not backed up yet.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `browserRetentionMeta`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { BrowserProfile } from './types'

/**
 * Defines the type-level contract for translate.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type Translate = (
  key: string,
  values?: Record<string, number | string>,
) => string

/**
 * Explains how browser retention meta works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function browserRetentionMeta(
  profile: BrowserProfile,
  t: Translate,
): {
  label: string
  body: string
} {
  if (profile.retentionBoundary.kind === 'macos-safari') {
    return {
      label: t('browserRetentionSafariLabel', {
        days: profile.retentionBoundary.localDays ?? 365,
      }),
      body: t('browserRetentionSafariBody'),
    }
  }

  return {
    label: t('browserRetentionManagedLabel'),
    body: t('browserRetentionManagedBody'),
  }
}
