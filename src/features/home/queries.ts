/** Data for Home. Every query sits under the archive key, so a backup refreshes it. */
import { useQuery } from '@tanstack/react-query'
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
  return useQuery({
    queryKey: key('trend-30'),
    queryFn: () => insightsClient.discoveryTrend({ dateRange: lastDays(30) }),
    refetchInterval: pollWhileStale,
  })
}

export function useThirtyDayDigest() {
  return useQuery({
    queryKey: key('digest-30'),
    queryFn: () => insightsClient.digest({ dateRange: lastDays(30) }),
    refetchInterval: pollWhileStale,
  })
}

export function useYearTrend(year: number) {
  return useQuery({
    queryKey: key('year', year),
    queryFn: () =>
      insightsClient.discoveryTrend({ dateRange: calendarYear(year) }),
    placeholderData: (previous) => previous,
    refetchInterval: pollWhileStale,
  })
}

export function useOnThisDay() {
  return useQuery({
    queryKey: key('on-this-day'),
    queryFn: () => insightsClient.onThisDay(),
  })
}

export function useThreads() {
  return useQuery({
    queryKey: key('threads'),
    queryFn: () =>
      insightsClient.reopenedInvestigations({ dateRange: lastDays(90) }),
    refetchInterval: pollWhileStale,
  })
}

export function useSourceStats() {
  return useQuery({
    queryKey: [...queryKeys.archiveData, 'source-stats'],
    queryFn: sourcesClient.stats,
  })
}
