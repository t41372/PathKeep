/**
 * @file preview-entry-list.tsx
 * @description Canonical preview-row evidence list for import and review surfaces.
 * @module components/review
 *
 * ## Responsibilities
 * - Render imported preview entries with stable time, status, and source-path grammar.
 * - Keep import-oriented evidence rows out of generic shell primitive buckets.
 *
 * ## Not responsible for
 * - Loading preview entries or deciding empty/loading/error states.
 * - Owning route-specific action bars or follow-through mutations.
 *
 * ## Dependencies
 * - Depends on `StatusTag` for status tone rendering.
 * - Depends on shared datetime formatting and review callers for localized status labels.
 *
 * ## Performance notes
 * - Pure render-only list; callers should continue to bound the preview entry count upstream.
 */

import type { ResolvedLanguage } from '../../lib/i18n'
import { formatDateTime } from '../../lib/format'
import type { TakeoutPreviewEntry } from '../../lib/types'
import { StatusTag } from '../ui'

interface PreviewEntryListProps {
  entries: TakeoutPreviewEntry[]
  language: ResolvedLanguage
  statusLabel?: (status: string) => string
  statusTone?: (status: string) => 'info' | 'success' | 'danger' | 'neutral'
}

/**
 * Renders the canonical import preview-entry list.
 *
 * This component exists so import-oriented evidence rows stay in one review
 * owner instead of being scattered across route-local panels and generic UI
 * buckets.
 */
export function PreviewEntryList({
  entries,
  language,
  statusLabel,
  statusTone,
}: PreviewEntryListProps) {
  return (
    <div className="previewList">
      {entries.map((entry) => (
        <article
          className="previewEntry"
          key={`${entry.sourcePath}:${entry.sourceVisitId}`}
        >
          <div className="previewMeta">
            <span>{formatDateTime(entry.visitedAt, language)}</span>
            <StatusTag
              ariaLabel={statusLabel?.(entry.status)}
              tone={
                statusTone?.(entry.status) ??
                (entry.status === 'imported' ? 'success' : 'info')
              }
            >
              {statusLabel?.(entry.status) ?? entry.status}
            </StatusTag>
          </div>
          <strong>{entry.title || entry.url}</strong>
          <p>{entry.url}</p>
          <small>
            {entry.sourcePath} · #{entry.sourceVisitId}
          </small>
        </article>
      ))}
    </div>
  )
}
