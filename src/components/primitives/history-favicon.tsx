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

import { useEffect, useState } from 'react'
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
  const [loadFailed, setLoadFailed] = useState(false)

  useEffect(() => {
    setLoadFailed(false)
  }, [favicon?.dataUrl])

  const fallbackLabel = domain?.trim()?.[0]?.toUpperCase() ?? '?'
  const showImage = Boolean(favicon?.dataUrl) && !loadFailed

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
          src={favicon!.dataUrl}
          onError={() => setLoadFailed(true)}
        />
      ) : (
        fallbackLabel
      )}
    </span>
  )
}
