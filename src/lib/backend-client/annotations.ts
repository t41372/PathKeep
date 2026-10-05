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

/**
 * One page of annotations. Pass `nextCursor` back unchanged to get the next
 * page; `null` means there is nothing more.
 */
export interface AnnotationPage {
  items: UrlAnnotation[]
  nextCursor: string | null
}

export const annotationsClient = {
  getUrlAnnotation: (url: string) =>
    call<UrlAnnotation | null>('get_url_annotation', { url }),
  setUrlNotes: (request: SetNotesRequest) =>
    call<UrlAnnotation>('set_url_notes', { request }),
  replaceUrlTags: (request: ReplaceTagsRequest) =>
    call<UrlAnnotation>('replace_url_tags', { request }),
  listUrlAnnotations: (cursor: string | null, limit?: number) =>
    call<AnnotationPage>('list_url_annotations', {
      cursor,
      limit: limit ?? null,
    }),
  searchUrlAnnotations: (
    query: string,
    cursor: string | null,
    limit?: number,
  ) =>
    call<AnnotationPage>('search_url_annotations', {
      query,
      cursor,
      limit: limit ?? null,
    }),
}
