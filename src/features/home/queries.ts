/**
 * Data for Home. Every query sits under the archive key, so a backup refreshes
 * it. The Insights reads are in Insights' time zone, like Insights itself.
 */
import { useQuery } from '@tanstack/react-query'
import { useInsightsZone } from '@/features/insights/zone'
import {
  calendarYear,
  insightsClient,
  lastDays,
  pollWhileStale,
} from '@/lib/backend-client/insights'
import { sourcesClient } from '@/lib/backend-client/sources'
import { queryKeys } from '@/lib/query'

const key = (...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'home',
  ...parts,
]

export function useThirtyDayTrend() {
  const zone = useInsightsZone().shown
  return useQuery({
    queryKey: key('trend-30', zone),
    queryFn: () =>
      insightsClient.discoveryTrend({ dateRange: lastDays(30, zone) }),
    refetchInterval: pollWhileStale,
  })
}

export function useThirtyDayDigest() {
  const zone = useInsightsZone().shown
  return useQuery({
    queryKey: key('digest-30', zone),
    queryFn: () => insightsClient.digest({ dateRange: lastDays(30, zone) }),
    refetchInterval: pollWhileStale,
  })
}

export function useYearTrend(year: number) {
  const zone = useInsightsZone().shown
  return useQuery({
    queryKey: key('year', year, zone),
    queryFn: () =>
      insightsClient.discoveryTrend({ dateRange: calendarYear(year) }),
    placeholderData: (previous) => previous,
    refetchInterval: pollWhileStale,
  })
}

export function useOnThisDay() {
  const zone = useInsightsZone().shown
  return useQuery({
    queryKey: key('on-this-day', zone),
    queryFn: () => insightsClient.onThisDay(),
    refetchInterval: pollWhileStale,
  })
}

export function useThreads() {
  const zone = useInsightsZone().shown
  return useQuery({
    queryKey: key('threads', zone),
    queryFn: () =>
      insightsClient.reopenedInvestigations({ dateRange: lastDays(90, zone) }),
    refetchInterval: pollWhileStale,
  })
}

export function useSourceStats() {
  return useQuery({
    queryKey: [...queryKeys.archiveData, 'source-stats'],
    queryFn: sourcesClient.stats,
  })
}
