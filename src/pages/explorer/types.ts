/**
 * This module renders the History Explorer route and keeps the keyword-first, deep-linkable recall workflow honest even when optional AI features degrade.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ExplorerMode`
 * - `Translator`
 * - `ExplorerQueryState`
 * - `SemanticQueryState`
 * - `RecentSearchEntry`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import type { AiSearchResponse, HistoryQueryResponse } from '../../lib/types'

/**
 * Enumerates the supported explorer modes.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type ExplorerMode = 'keyword' | 'semantic' | 'hybrid'

/**
 * Defines the type-level contract for translator.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Captures the state shape used by `ExplorerQuery`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface ExplorerQueryState {
  requestKey: string | null
  results: HistoryQueryResponse | null
  error: string | null
}

/**
 * Captures the state shape used by `SemanticQuery`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface SemanticQueryState {
  requestKey: string | null
  results: AiSearchResponse | null
  error: string | null
}

/**
 * Defines the typed shape for recent search entry.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface RecentSearchEntry {
  label?: string
  params: {
    q?: string | null
    mode?: ExplorerMode | null
    domain?: string | null
    profileId?: string | null
    browserKind?: string | null
    start?: string | null
    end?: string | null
    regex?: '1' | null
    sort?: 'newest' | 'oldest'
  }
}
