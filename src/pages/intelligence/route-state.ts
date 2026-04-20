/**
 * Route-level URL and scope state for Core Intelligence routes.
 *
 * Why this file exists:
 * - `/intelligence` and `/intelligence/domain/:domain` should share the same query-string contract for time range and profile scope.
 * - Centralizing that contract keeps deep links honest and prevents the dashboard and deep-dive routes from drifting apart.
 *
 * Main declarations:
 * - `useIntelligenceRouteState`
 * - `buildIntelligenceSearchParams`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for shared profile-scope and deep-link grammar.
 * - Keep time-range behavior aligned with `docs/features/core-intelligence-ultimate-design.md` §4.B.
 */

import { useCallback, useMemo } from 'react'
import { useSearchParams } from 'react-router-dom'
import {
  buildIntelligenceSearchParams,
  dateRangeFromPreset,
  parseInsightRouteFocus,
  type DateRange,
  type InsightRouteFocus,
  type TimeRangePreset,
} from '../../lib/core-intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'

const validPresets: TimeRangePreset[] = [
  'day',
  'week',
  'month',
  'quarter',
  'year',
  'custom',
]

/**
 * Provides the shared URL-backed state used by the Intelligence routes.
 */
export function useIntelligenceRouteState() {
  const [searchParams, setSearchParams] = useSearchParams()
  const { activeProfileId } = useProfileScope()
  const requestedPreset = searchParams.get('range')
  const explicitProfileId = searchParams.get('profileId')
  const effectiveProfileId = explicitProfileId ?? activeProfileId
  const focus = parseInsightRouteFocus(searchParams)
  const preset: TimeRangePreset = validPresets.includes(
    requestedPreset as TimeRangePreset,
  )
    ? (requestedPreset as TimeRangePreset)
    : searchParams.get('start') && searchParams.get('end')
      ? 'custom'
      : 'month'

  const dateRange = useMemo<DateRange>(() => {
    if (preset !== 'custom') {
      return dateRangeFromPreset(preset)
    }

    const start = searchParams.get('start')
    const end = searchParams.get('end')
    if (start && end) {
      return { start, end }
    }

    return dateRangeFromPreset('month')
  }, [preset, searchParams])

  const profileScopeLabel = effectiveProfileId
    ? profileIdLabel(effectiveProfileId)
    : null

  const setPreset = useCallback(
    (nextPreset: TimeRangePreset) => {
      setSearchParams((current) => {
        const next = new URLSearchParams(current)
        next.set('range', nextPreset)
        if (nextPreset !== 'custom') {
          next.delete('start')
          next.delete('end')
        }
        return next
      })
    },
    [setSearchParams],
  )

  const setCustomRange = useCallback(
    (nextRange: DateRange) => {
      setSearchParams((current) => {
        const next = new URLSearchParams(current)
        next.set('range', 'custom')
        next.set('start', nextRange.start)
        next.set('end', nextRange.end)
        return next
      })
    },
    [setSearchParams],
  )

  const withCurrentRouteSearch = useCallback(
    (
      overrides: Partial<{
        dateRange: DateRange
        preset: TimeRangePreset
        profileId: string | null
        focus: InsightRouteFocus | null
      }> = {},
    ) => {
      const nextParams = buildIntelligenceSearchParams({
        dateRange: overrides.dateRange ?? dateRange,
        preset: overrides.preset ?? preset,
        profileId:
          overrides.profileId === undefined
            ? effectiveProfileId
            : overrides.profileId,
        focus: overrides.focus === undefined ? focus : overrides.focus,
      })
      const query = nextParams.toString()
      return query ? `?${query}` : ''
    },
    [dateRange, effectiveProfileId, focus, preset],
  )

  return {
    dateRange,
    effectiveProfileId,
    explicitProfileId,
    focus,
    preset,
    profileScopeLabel,
    setCustomRange,
    setPreset,
    withCurrentRouteSearch,
  }
}
