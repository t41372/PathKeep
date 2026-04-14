/**
 * This module renders the shell-level loading surface used during lazy route hydration.
 *
 * Why this file exists:
 * - React Router expects a dedicated hydrate fallback when the first matched route is lazy-loaded.
 * - Keeping the fallback in its own component file preserves Fast Refresh rules in the route registry module.
 *
 * Main declarations:
 * - `RouteHydrateFallback`
 *
 * Source-of-truth notes:
 * - Loading copy remains part of the i18n shipping contract and should stay aligned with `docs/design/ux-principles.md`.
 */

import { LoadingState } from '../components/primitives/loading-state'
import { useI18n } from '../lib/i18n'

/**
 * Renders the shell-level fallback used while lazy route modules hydrate.
 */
export function RouteHydrateFallback() {
  const { t } = useI18n()

  return <LoadingState compact label={t('common.loading')} />
}
