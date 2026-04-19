/**
 * Staged `/intelligence` overview loader.
 *
 * Why this file exists:
 * - The route should paint skeletons immediately and avoid mounting a large
 *   number of foreground data hooks before the first visible band is ready.
 * - This hook owns the "primary first, secondary after first paint / idle"
 *   policy without forcing every section component to know about batching.
 */

import { useEffect, useState } from 'react'
import type { DateRange } from '../../lib/core-intelligence'
import {
  loadIntelligencePrimaryOverview,
  loadIntelligenceSecondaryOverview,
  peekIntelligencePrimaryOverview,
  peekIntelligenceSecondaryOverview,
} from '../../lib/core-intelligence'

interface StagedOverviewState {
  scopeKey: string
  primaryReady: boolean
  primaryError: string | null
  secondaryReady: boolean
  secondaryLoading: boolean
  secondaryError: string | null
}

function createOverviewState(
  dateRange: DateRange,
  profileId: string | null,
): StagedOverviewState {
  const scopeKey = `${dateRange.start}:${dateRange.end}:${profileId ?? 'archive-wide'}`

  return {
    scopeKey,
    primaryReady: Boolean(
      peekIntelligencePrimaryOverview(dateRange, profileId),
    ),
    primaryError: null,
    secondaryReady: Boolean(
      peekIntelligenceSecondaryOverview(dateRange, profileId),
    ),
    secondaryLoading: false,
    secondaryError: null,
  }
}

function scheduleIdleLoad(callback: () => void) {
  if (typeof window === 'undefined') {
    return () => {}
  }

  const idleWindow = window as Window & {
    requestIdleCallback?: (
      cb: IdleRequestCallback,
      options?: IdleRequestOptions,
    ) => number
    cancelIdleCallback?: (handle: number) => void
  }

  if (typeof idleWindow.requestIdleCallback === 'function') {
    const handle = idleWindow.requestIdleCallback(() => callback(), {
      timeout: 1200,
    })
    return () => idleWindow.cancelIdleCallback?.(handle)
  }

  const handle = window.setTimeout(callback, 160)
  return () => window.clearTimeout(handle)
}

export function useStagedIntelligenceOverview(
  dateRange: DateRange,
  profileId: string | null,
): StagedOverviewState {
  const scopeKey = `${dateRange.start}:${dateRange.end}:${profileId ?? 'archive-wide'}`
  const cachedState = createOverviewState(dateRange, profileId)
  const [state, setState] = useState<StagedOverviewState>(() => cachedState)

  useEffect(() => {
    let cancelled = false
    let cancelIdle = () => {}
    let frameId: number | null = null
    const nextDateRange = {
      start: dateRange.start,
      end: dateRange.end,
    }
    const nextCachedState = createOverviewState(nextDateRange, profileId)
    const nextScopeKey = nextCachedState.scopeKey

    void loadIntelligencePrimaryOverview(nextDateRange, profileId)
      .then(() => {
        if (cancelled) return
        setState((current) => ({
          ...(current.scopeKey === nextScopeKey ? current : nextCachedState),
          primaryReady: true,
          primaryError: null,
        }))

        if (typeof window === 'undefined') {
          return
        }

        frameId = window.requestAnimationFrame(() => {
          cancelIdle = scheduleIdleLoad(() => {
            if (cancelled) return
            setState((current) => ({
              ...(current.scopeKey === nextScopeKey
                ? current
                : nextCachedState),
              secondaryLoading: true,
            }))
            void loadIntelligenceSecondaryOverview(nextDateRange, profileId)
              .then(() => {
                if (cancelled) return
                setState((current) => ({
                  ...(current.scopeKey === nextScopeKey
                    ? current
                    : nextCachedState),
                  secondaryReady: true,
                  secondaryLoading: false,
                  secondaryError: null,
                }))
              })
              .catch((error: unknown) => {
                if (cancelled) return
                setState((current) => ({
                  ...(current.scopeKey === nextScopeKey
                    ? current
                    : nextCachedState),
                  secondaryReady: true,
                  secondaryLoading: false,
                  secondaryError:
                    error instanceof Error ? error.message : String(error),
                }))
              })
          })
        })
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setState({
          ...nextCachedState,
          primaryReady: true,
          primaryError: error instanceof Error ? error.message : String(error),
          secondaryReady: true,
          secondaryLoading: false,
          secondaryError: null,
        })
      })

    return () => {
      cancelled = true
      cancelIdle()
      if (frameId !== null && typeof window !== 'undefined') {
        window.cancelAnimationFrame(frameId)
      }
    }
  }, [dateRange.end, dateRange.start, profileId])

  return state.scopeKey === scopeKey ? state : cachedState
}
