/**
 * Typed front-end client for the stars (favorites / 加星) commands.
 *
 * Why this file exists:
 * - PathKeep stores stars against canonical entities in the archive
 *   (migration 014 + `vault-core::stars`). Explorer rows, the detail panel,
 *   search results, and assistant evidence toggle them; the Starred hub reads
 *   them.
 * - Keeping the typed client here means routes never type raw command names
 *   ("set_star") and stay shielded against transport renames.
 *
 * Main declarations:
 * - `starsClient`
 *
 * Source-of-truth notes:
 * - Transport contract: `docs/architecture/desktop-command-surface.md`.
 * - Shape contract: `vault_core::models::stars` (StarEntityKind, StarSort,
 *   SetStarRequest, StarStatusRequest, StarListItem, StarCounts).
 */

import { call } from './shared'

/** The kind of entity a star points at. `queryFamily` is deferred. */
export type StarEntityKind = 'url' | 'domain'

/** Ordering for the Starred hub. */
export type StarSort = 'recently_starred' | 'most_revisited'

/** A request to add or remove a star. */
export interface SetStarRequest {
  entityKind: StarEntityKind
  entityKey: string
  sourceProfile?: string | null
}

/** A batched status request for the currently-visible rows only. */
export interface StarStatusRequest {
  entityKind: StarEntityKind
  entityKeys: string[]
}

/** One starred entity returned by `list_stars`, enriched for the hub. */
export interface StarListItem {
  entityKind: StarEntityKind
  entityKey: string
  starredAt: string
  domain: string
  title: string
  visitCount: number
}

/** Per-kind rollup of how many things the user has starred. */
export interface StarCounts {
  urls: number
  domains: number
}

export const starsClient = {
  setStar: (request: SetStarRequest) => call<void>('set_star', { request }),
  unsetStar: (request: SetStarRequest) => call<void>('unset_star', { request }),
  /** Map of the supplied raw keys → starred boolean. Visible rows only. */
  getStarStatus: (request: StarStatusRequest) =>
    call<Record<string, boolean>>('get_star_status', { request }),
  listStars: (kind: StarEntityKind | null, sort: StarSort, limit?: number) =>
    call<StarListItem[]>('list_stars', {
      kind: kind ?? null,
      sort,
      limit: limit ?? null,
    }),
  getStarCounts: () => call<StarCounts>('get_star_counts', {}),
}
