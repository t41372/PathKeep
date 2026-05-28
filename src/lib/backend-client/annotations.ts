/**
 * Typed front-end client for the per-URL annotations commands.
 *
 * Why this file exists:
 * - PathKeep stores notes and tags against URLs in the canonical archive
 *   (migration 011 + `vault-core::annotations`). The Browse detail panel
 *   writes them; the future Recall surface reads them.
 * - Keeping the typed client here means routes never type raw command
 *   names ("set_url_notes") and shield against transport renames.
 *
 * Main declarations:
 * - `annotationsClient`
 *
 * Source-of-truth notes:
 * - Transport contract: `docs/architecture/desktop-command-surface.md`.
 * - Shape contract: `vault_core::models::annotations` (UrlAnnotation,
 *   SetNotesRequest, ReplaceTagsRequest).
 */

import { call } from './shared'

/**
 * Per-URL annotation bundle returned by the backend. `notes` is the empty
 * string when no note has been written; `tags` is an empty array when no
 * tag is attached.
 */
export interface UrlAnnotation {
  url: string
  notes: string
  tags: string[]
  updatedAt: string
  createdAt: string
  sourceProfile?: string | null
}

export interface SetNotesRequest {
  url: string
  notes: string
  sourceProfile?: string | null
}

export interface ReplaceTagsRequest {
  url: string
  tags: string[]
  sourceProfile?: string | null
}

export const annotationsClient = {
  getUrlAnnotation: (url: string) =>
    call<UrlAnnotation | null>('get_url_annotation', { url }),
  setUrlNotes: (request: SetNotesRequest) =>
    call<UrlAnnotation>('set_url_notes', { request }),
  replaceUrlTags: (request: ReplaceTagsRequest) =>
    call<UrlAnnotation>('replace_url_tags', { request }),
  listUrlAnnotations: (limit?: number) =>
    call<UrlAnnotation[]>('list_url_annotations', { limit: limit ?? null }),
  searchUrlAnnotations: (query: string, limit?: number) =>
    call<UrlAnnotation[]>('search_url_annotations', {
      query,
      limit: limit ?? null,
    }),
}
