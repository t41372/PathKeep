/**
 * The time zone Insights put days and hours in
 * (`docs/architecture/insights-time-zone.md`): their own, chosen once and
 * kept when the computer's zone changes, so a trip does not move the days.
 *
 * Responsible for: reading the zone (and polling while a rebuild in a newly
 * chosen one runs), formatters that show Insights' times in it, whether to
 * tell the user that the computer is now in another zone, and choosing or
 * keeping a zone, a zone in words. Not responsible for: the day math (`lib/time-zone.ts`),
 * the notice and the Settings row themselves (`zone-notice.tsx`).
 */
import {
  queryOptions,
  useQueryClient,
  useSuspenseQuery,
} from '@tanstack/react-query'
import { useCallback } from 'react'
import { insightsClient } from '@/lib/backend-client/insights'
import type { InsightsTimeZone } from '@/lib/core-intelligence/types'
import { useZonedFormat, type Formatters, type Translator } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import { computerTimeZone, sameZone, zoneLabel } from '@/lib/time-zone'
import { useSaveSetting } from '@/features/settings/use-save-setting'

/** The zone, or the computer's while it could not be read (`unknown`). */
type ZoneState = InsightsTimeZone & { unknown?: true }

/** Whether a rebuild in a new zone has not yet reached every profile. */
export function zoneRebuilding(zone: InsightsTimeZone) {
  return zone.shown !== zone.next || zone.rebuildingFrom !== null
}

/**
 * Read once when the archive opens (`prefetchInsightsZone`), and again every
 * few seconds while a rebuild in a new zone runs. A failed read stands in the
 * computer's zone (an archive without Insights uses it) and is tried again
 * shortly; the cards show their own errors meanwhile.
 */
export const insightsZoneQuery = queryOptions({
  queryKey: queryKeys.insightsZone,
  queryFn: async (): Promise<ZoneState> => {
    const before = queryClient.getQueryData<ZoneState>(queryKeys.insightsZone)
    let zone: ZoneState
    try {
      zone = await insightsClient.timeZone()
    } catch {
      const computer = computerTimeZone()
      zone = {
        shown: computer,
        next: computer,
        rebuildingFrom: null,
        unknown: true,
      }
    }
    // Reads moved to another zone, or a rebuild in it reached the last
    // profile: cached Insights reads hold the old days.
    if (
      before &&
      (before.shown !== zone.shown ||
        (zoneRebuilding(before) && !zoneRebuilding(zone)))
    ) {
      void queryClient.invalidateQueries({
        predicate: ({ queryKey }) =>
          queryKey[0] === queryKeys.archiveData[0] &&
          queryKey[1] !== queryKeys.insightsZone[1],
      })
    }
    return zone
  },
  staleTime: Infinity,
  refetchInterval: (query) => {
    const zone = query.state.data
    if (zone?.unknown) return 15_000
    return zone && zoneRebuilding(zone) ? 5_000 : false
  },
})

/** Starts reading the zone as the archive opens, so screens rarely wait for it. */
export function prefetchInsightsZone() {
  void queryClient.prefetchQuery(insightsZoneQuery)
}

/**
 * The zone of the Insights on screen and of the next build. Suspends the
 * screen (its skeleton shows) until the zone is known, since every date range
 * depends on it.
 */
export function useInsightsZone(): InsightsTimeZone {
  return useSuspenseQuery(insightsZoneQuery).data
}

/** Formatters for Insights: times in Insights' zone, labelled when it is not the computer's. */
export function useInsightsFormat(): Formatters {
  const { shown } = useInsightsZone()
  return useZonedFormat({
    timeZone: shown,
    label: !sameZone(shown, computerTimeZone()),
  })
}

export interface ZoneMismatch {
  /** Insights' zone (the next build's). */
  insights: string
  /** The computer's zone now. */
  computer: string
}

/**
 * The computer is in another zone than Insights, and the user has not said to
 * keep Insights' zone while it is there.
 */
export function useZoneMismatch(): ZoneMismatch | null {
  const { next } = useInsightsZone()
  const kept = useSnapshot().config.insightsZoneKeptFor
  const computer = computerTimeZone()
  if (sameZone(next, computer)) return null
  if (kept && sameZone(kept, computer)) return null
  return { insights: next, computer }
}

/**
 * Puts Insights in `zone` (they are rebuilt in it) or keeps them in theirs.
 * Either way the computer's current zone is remembered, so the notice stays
 * away until the computer moves again.
 */
export function useChooseInsightsZone() {
  const client = useQueryClient()
  const { save, saving } = useSaveSetting()
  const choose = useCallback(
    async (zone: string) => {
      const saved = await save((config) => ({
        ...config,
        insightsTimeZone: zone,
        insightsZoneKeptFor: computerTimeZone(),
      }))
      if (saved) await client.invalidateQueries(insightsZoneQuery)
      return saved
    },
    [client, save],
  )
  return { choose, saving }
}

/**
 * A zone in words: "Pacific Time (GMT-7)". `others` are zones shown next to
 * it; when one has the same name, the place tells them apart.
 */
export function zoneText(
  zone: string,
  locale: string,
  t: Translator,
  others: string[] = [],
) {
  const label = zoneLabel(zone, locale)
  const name = label.name || label.place
  const clash = others.some((other) => {
    const theirs = zoneLabel(other, locale)
    return other !== zone && (theirs.name || theirs.place) === name
  })
  return t(
    clash && label.name
      ? 'insights.zone.labelWithPlace'
      : 'insights.zone.label',
    { name, place: label.place, offset: label.offset },
  )
}
