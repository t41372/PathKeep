/**
 * This module renders one history favicon with a deterministic placeholder fallback.
 *
 * Why this file exists:
 * - Explorer and other recall surfaces should show stored page icons when the archive has them without turning broken image payloads into noisy UI failures.
 * - The fallback letter keeps the row layout stable and honest when a browser never captured an icon or the payload cannot be rendered.
 *
 * Main declarations:
 * - `HistoryFavicon`
 *
 * Source-of-truth notes:
 * - Explorer row/detail presentation follows `docs/features/recall.md`.
 * - Favicon availability remains evidence-based: show the stored icon when present, otherwise render the placeholder instead of inventing one.
 */

import { useState } from 'react'
import type { HistoryEntry } from '../../lib/types'

/**
 * Describes the props accepted by `HistoryFavicon`.
 *
 * A named contract keeps the archive read-model boundary explicit where routes consume it.
 */
interface HistoryFaviconProps {
  domain?: string | null
  favicon?: HistoryEntry['favicon']
}

/**
 * Renders one stored history favicon or a deterministic domain-initial fallback.
 *
 * The placeholder keeps Explorer stable even when favicon coverage is incomplete across browsers or individual rows.
 */
export function HistoryFavicon({ domain, favicon }: HistoryFaviconProps) {
  const [failedDataUrl, setFailedDataUrl] = useState<string | null>(null)

  // Stryker disable next-line OptionalChaining: trim always returns a string, so indexing its first character is safe even when it is empty.
  const fallbackLabel = domain?.trim()?.[0]?.toUpperCase() ?? '?'
  const dataUrl = favicon?.dataUrl ?? null
  const showImage = Boolean(dataUrl) && failedDataUrl !== dataUrl

  return (
    <span
      aria-hidden
      className={`favicon-placeholder ${showImage ? 'has-image' : ''}`}
    >
      {showImage ? (
        <img
          alt=""
          className="favicon-image"
          loading="lazy"
          src={dataUrl!}
          onError={() => setFailedDataUrl(dataUrl)}
        />
      ) : (
        fallbackLabel
      )}
    </span>
  )
}
