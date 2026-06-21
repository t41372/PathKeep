/**
 * Backend-backed visit-enrichment hook for the Explorer detail panel
 * (W-ENRICH-1, 06 §6).
 *
 * Owns the `list_visit_enrichment` read and the `content_fetch_now` PME write
 * for the one open visit, and projects them into the discriminated state the
 * presentational `PaperEnrichedContent` renders. Mirrors `useDesktopStars`: the
 * route picks the open entry, this hook does the off-thread I/O, and the
 * component stays a pure renderer.
 *
 * ## Responsibilities
 * - Load enrichment for the selected `historyId` (re-loading when it changes),
 *   surfacing loading / disabled / error / empty / ready states honestly.
 * - Never block the render path: the read is async and the panel shows a
 *   skeleton (`status: 'loading'`) until it resolves.
 * - Trigger a manual fetch (`content_fetch_now`); reflect the queued/disabled
 *   result honestly and re-load enrichment shortly after a successful enqueue so
 *   the panel picks up the new content without a manual refresh.
 *
 * ## Not responsible for
 * - Consent persistence — `fetchEnabled` is read from the shell snapshot; the
 *   Settings consent section owns writes.
 * - Rendering — the component owns layout + copy.
 *
 * ## Performance notes
 * - One bounded read per opened visit. No polling. A single short delayed
 *   re-load after a manual enqueue (not a tight poll loop) keeps the panel
 *   current without hammering the worker at 14.4M scale.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { backend } from '../../lib/backend-client'
import type { PaperEnrichedContentState } from '@/components/explorer-paper'
import {
  pickBestEnrichment,
  toEnrichmentView,
} from '@/components/explorer-paper'
import { describeError } from '../../lib/errors'

/** How long after a successful enqueue to re-poll once for the new content. */
const REFRESH_AFTER_ENQUEUE_MS = 2500

export interface VisitEnrichmentTarget {
  historyId: number
  profileId: string
  url: string
  title?: string | null
}

export interface UseVisitEnrichmentArgs {
  /** The open visit, or null when the panel is closed / has no entry. */
  target: VisitEnrichmentTarget | null
  /** Whether site content fetching is enabled (gates the Fetch-now CTA). */
  fetchEnabled: boolean
}

export interface VisitEnrichment {
  state: PaperEnrichedContentState
  fetchEnabled: boolean
  fetchPending: boolean
  fetchError: boolean
  fetchNow: () => void
}

export function useVisitEnrichment({
  target,
  fetchEnabled,
}: UseVisitEnrichmentArgs): VisitEnrichment {
  const historyId = target?.historyId ?? null
  // Initial state mirrors the first target: a real visit loads, no visit is
  // empty. Keeps the very first render honest before any effect runs.
  const [state, setState] = useState<PaperEnrichedContentState>(() =>
    historyId === null ? { status: 'empty' } : { status: 'loading' },
  )
  const [fetchPending, setFetchPending] = useState(false)
  const [fetchError, setFetchError] = useState(false)
  // Bumped to force a re-load (after a manual enqueue) without changing target.
  const [reloadToken, setReloadToken] = useState(0)
  const refreshTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Reset the per-entry view + transient flags the moment the open visit
  // changes, derived during render (React 19's blessed alternative to a
  // setState-in-effect) so a pending/error state or stale view from one record
  // never flashes onto the next. The load effect below then fills `state` with
  // the async truth. The guard makes this run exactly once per id change.
  const [trackedHistoryId, setTrackedHistoryId] = useState<number | null>(
    historyId,
  )
  if (historyId !== trackedHistoryId) {
    setTrackedHistoryId(historyId)
    setState(historyId === null ? { status: 'empty' } : { status: 'loading' })
    setFetchPending(false)
    setFetchError(false)
  }

  useEffect(() => {
    if (historyId === null) return
    let cancelled = false
    backend
      .listVisitEnrichment(historyId)
      .then((records) => {
        if (cancelled) return
        const best = pickBestEnrichment(records)
        if (!best) {
          // No enrichment row at all: honest empty if consent is on (never
          // fetched yet), or a "consent off" disabled note so the user knows
          // why nothing is here and how to enable it.
          setState(fetchEnabled ? { status: 'empty' } : { status: 'disabled' })
          return
        }
        setState({ status: 'ready', view: toEnrichmentView(best) })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        // Surface the read failure honestly; describeError keeps it bounded.
        void describeError(error, 'list_visit_enrichment')
        setState({ status: 'error' })
      })
    return () => {
      cancelled = true
    }
  }, [historyId, fetchEnabled, reloadToken])

  useEffect(
    () => () => {
      if (refreshTimer.current) {
        clearTimeout(refreshTimer.current)
        refreshTimer.current = null
      }
    },
    [],
  )

  const fetchNow = useCallback(() => {
    if (!target || !fetchEnabled || fetchPending) return
    setFetchError(false)
    setFetchPending(true)
    backend
      .contentFetchNow({
        historyId: target.historyId,
        profileId: target.profileId,
        url: target.url,
        title: target.title ?? null,
      })
      .then((result) => {
        if (result.state === 'disabled') {
          // The backend declined because consent is off for this URL — keep the
          // CTA honest rather than implying a fetch is in flight.
          setFetchPending(false)
          setFetchError(true)
          return
        }
        // Queued/running: re-load once shortly so the new content surfaces.
        if (refreshTimer.current) clearTimeout(refreshTimer.current)
        refreshTimer.current = setTimeout(() => {
          setFetchPending(false)
          setReloadToken((token) => token + 1)
        }, REFRESH_AFTER_ENQUEUE_MS)
      })
      .catch((error: unknown) => {
        void describeError(error, 'content_fetch_now')
        setFetchPending(false)
        setFetchError(true)
      })
  }, [fetchEnabled, fetchPending, target])

  return useMemo(
    () => ({ state, fetchEnabled, fetchPending, fetchError, fetchNow }),
    [state, fetchEnabled, fetchPending, fetchError, fetchNow],
  )
}
