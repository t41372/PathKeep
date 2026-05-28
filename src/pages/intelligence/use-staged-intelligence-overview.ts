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
import { describeError } from '../../lib/errors'
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
    primaryReady: false,
    primaryError: null,
    secondaryReady: false,
    secondaryLoading: false,
    secondaryError: null,
  }
}

function stateForScope(
  current: StagedOverviewState,
  nextScopeKey: string,
  nextCachedState: StagedOverviewState,
) {
  return current.scopeKey === nextScopeKey ? current : nextCachedState
}

function scheduleIdleLoad(callback: () => void) {
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
    const hasPrimaryCache = Boolean(
      peekIntelligencePrimaryOverview(nextDateRange, profileId),
    )
    const hasSecondaryCache = Boolean(
      peekIntelligenceSecondaryOverview(nextDateRange, profileId),
    )

    const startSecondaryLoad = () => {
      frameId = window.requestAnimationFrame(() => {
        cancelIdle = scheduleIdleLoad(() => {
          if (cancelled) return
          setState((current) => ({
            ...stateForScope(current, nextScopeKey, nextCachedState),
            secondaryReady: hasSecondaryCache,
            secondaryLoading: !hasSecondaryCache,
            secondaryError: null,
          }))
          void loadIntelligenceSecondaryOverview(nextDateRange, profileId, {
            force: hasSecondaryCache,
          })
            .then(() => {
              if (cancelled) return
              setState((current) => ({
                ...stateForScope(current, nextScopeKey, nextCachedState),
                secondaryReady: true,
                secondaryLoading: false,
                secondaryError: null,
              }))
            })
            .catch((error: unknown) => {
              if (cancelled) return
              setState((current) => ({
                ...stateForScope(current, nextScopeKey, nextCachedState),
                secondaryReady: hasSecondaryCache,
                secondaryLoading: false,
                secondaryError: describeError(
                  error,
                  'load_intelligence_secondary',
                ),
              }))
            })
        })
      })
    }

    if (hasPrimaryCache) {
      frameId = window.requestAnimationFrame(() => {
        if (cancelled) return
        setState((current) => ({
          ...stateForScope(current, nextScopeKey, nextCachedState),
          primaryReady: true,
          primaryError: null,
        }))
        startSecondaryLoad()
      })

      void loadIntelligencePrimaryOverview(nextDateRange, profileId, {
        force: true,
      }).catch((error: unknown) => {
        if (cancelled) return
        setState((current) => ({
          ...stateForScope(current, nextScopeKey, nextCachedState),
          primaryReady: true,
          primaryError: describeError(error, 'load_intelligence_primary'),
        }))
      })

      return () => {
        cancelled = true
        cancelIdle()
        if (
          frameId !== null &&
          typeof window.cancelAnimationFrame === 'function'
        ) {
          window.cancelAnimationFrame(frameId)
        }
      }
    }

    void loadIntelligencePrimaryOverview(nextDateRange, profileId, {
      force: false,
    })
      .then(() => {
        if (cancelled) return
        setState((current) => ({
          ...stateForScope(current, nextScopeKey, nextCachedState),
          primaryReady: true,
          primaryError: null,
        }))
        startSecondaryLoad()
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setState({
          ...nextCachedState,
          primaryReady: true,
          primaryError: describeError(error, 'load_intelligence_primary'),
          secondaryReady: true,
          secondaryLoading: false,
          secondaryError: null,
        })
      })

    return () => {
      cancelled = true
      cancelIdle()
      if (
        frameId !== null &&
        typeof window.cancelAnimationFrame === 'function'
      ) {
        window.cancelAnimationFrame(frameId)
      }
    }
  }, [dateRange.end, dateRange.start, profileId])

  return state.scopeKey === scopeKey ? state : cachedState
}
