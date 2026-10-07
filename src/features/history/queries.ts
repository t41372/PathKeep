/**
 * Data for the History screen. Everything derived from archive contents sits
 * under `queryKeys.archiveData` so a backup or import refreshes it; stars and
 * notes are user data and use their own keys.
 */
import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
  type QueryClient,
} from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { annotationsClient } from '@/lib/backend-client/annotations'
import type {
  IndexerState,
  SearchCount,
  SearchRequest,
} from '@/lib/backend-client/search'
import { explorerClient } from '@/lib/backend-client/explorer'
import { insightsClient, localDateKey } from '@/lib/backend-client/insights'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { sourcesClient } from '@/lib/backend-client/sources'
import { starsClient } from '@/lib/backend-client/stars'
import { urlDetailClient } from '@/lib/backend-client/url-detail'
import { supportedBrowsers } from '@/lib/browser-icons'
import { queryKeys } from '@/lib/query'
import { useDashboard, useSnapshot } from '@/lib/queries/app'
import type { HistoryEntry, HistoryQueryResponse } from '@/lib/types'
import type { DayRange, SearchSpec } from './history-params'
import {
  browserKindOf,
  type HistoryFilters,
  type VisitItem,
} from './history-types'
import { searchRevisionKey, useSearchStream } from './use-search-stream'

const key = (...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'history',
  ...parts,
]

/**
 * Search reads the user's notes and tags (full text, `note:`, `tag:`), so a
 * saved note or tag makes cached results stale; without this a `tag:` search
 * run before tagging would keep showing nothing for five minutes.
 */
export function invalidateSearches(client: QueryClient) {
  void client.invalidateQueries({ queryKey: searchRevisionKey })
}

const PAGE_SIZE = 100
const SEMANTIC_PAGE_SIZE = 50
const SITES_PAGE_SIZE = 200

function fromEntry(entry: HistoryEntry): VisitItem {
  return {
    id: entry.id,
    profileId: entry.profileId,
    url: entry.url,
    title: entry.title ?? null,
    domain: entry.domain,
    visitTime: entry.visitTime,
    visitCount: entry.visitCount,
  }
}

/** What a search knows so far. */
export interface SearchStatus {
  /** Matching pages; a lower bound until `exact`. */
  pages: SearchCount
  /** Matching visits; null for semantic search, which returns pages only. */
  visits: SearchCount | null
  /** The order of the rows shown can still change. */
  sorting: boolean
  /** False while the search index hasn't caught up with the archive: pages can be missing. */
  coverageComplete: boolean
  indexer: IndexerState | null
}

/** The pages of a visit list, flattened. */
export interface VisitList {
  items: VisitItem[]
  /** Searches only. */
  search: SearchStatus | null
  isPending: boolean
  isPlaceholder: boolean
  error: Error | null
  hasNextPage: boolean
  isFetchingNextPage: boolean
  fetchNextPage: () => void
  refetch: () => void
}

/**
 * The timeline and the search results share one list. The timeline lists every
 * visit, read by cursor. Full-text and regex searches stream from the search
 * index (one row per page): the first rows arrive while ranking and counting
 * continue, and the count is a lower bound until it is exact. Semantic results
 * come from the AI search, which also returns one row per page.
 */
export function useVisitList(
  search: SearchSpec,
  filters: HistoryFilters,
  enabled: boolean,
): VisitList {
  const semantic = search.mode === 'semantic' && search.text !== ''
  // An invalid regex keeps the last valid search and its rows (dimmed) on screen.
  const streamed = search.text !== '' && !semantic

  const timeline = useInfiniteQuery({
    enabled: enabled && search.text === '',
    queryKey: key('list', 'visits', filters),
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      explorerClient.queryHistory({
        q: null,
        regexMode: false,
        sort: 'newest',
        ...filters,
        limit: PAGE_SIZE,
        cursor: pageParam,
        includeTotal: false,
      }),
    getNextPageParam: (last: HistoryQueryResponse) =>
      last.hasNext ? (last.nextCursor ?? undefined) : undefined,
    placeholderData: keepPreviousData,
  })

  const { startTimeMs, endTimeMs, browserKind, domain } = filters
  const typed = useMemo<SearchRequest | null>(
    () =>
      enabled && streamed && !search.regexError
        ? {
            query: search.pattern,
            regex: search.mode === 'regex',
            sort: search.mode === 'full' ? 'relevance' : 'newest',
            pageSize: PAGE_SIZE,
            startTimeMs,
            endTimeMs,
            browserKind,
            domain,
          }
        : null,
    [
      enabled,
      streamed,
      search.regexError,
      search.pattern,
      search.mode,
      startTimeMs,
      endTimeMs,
      browserKind,
      domain,
    ],
  )
  const [lastValid, setLastValid] = useState(typed)
  if (!search.regexError && lastValid !== typed) setLastValid(typed)
  const stream = useSearchStream(search.regexError ? lastValid : typed)

  const smart = useInfiniteQuery({
    enabled: enabled && semantic,
    queryKey: key('semantic', search.text, filters.domain),
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      intelligenceClient.searchHistory({
        query: search.text,
        domain: filters.domain,
        limit: SEMANTIC_PAGE_SIZE,
        cursor: pageParam,
      }),
    getNextPageParam: (last) => last.nextCursor ?? undefined,
    placeholderData: keepPreviousData,
  })

  const shown = stream.current ?? stream.placeholder
  const items = useMemo(() => {
    if (streamed) {
      return (shown?.pages ?? []).flatMap((page) =>
        page.items.map((hit) => fromEntry(hit.visit)),
      )
    }
    if (!semantic) {
      return (
        timeline.data?.pages.flatMap((page) => page.items.map(fromEntry)) ?? []
      )
    }
    // The AI search cannot filter by date or browser, so narrow its results here.
    return (smart.data?.pages ?? [])
      .flatMap((page) => page.items)
      .map<VisitItem>((item) => ({
        id: item.historyId,
        profileId: item.profileId,
        url: item.url,
        title: item.title ?? null,
        domain: item.domain,
        visitTime: Date.parse(item.visitedAt),
        score: item.score,
      }))
      .filter(
        (item) =>
          (startTimeMs === null || item.visitTime >= startTimeMs) &&
          (endTimeMs === null || item.visitTime <= endTimeMs) &&
          (browserKind === null ||
            browserKindOf(item.profileId) === browserKind),
      )
  }, [
    streamed,
    shown,
    semantic,
    smart.data,
    timeline.data,
    startTimeMs,
    endTimeMs,
    browserKind,
  ])

  if (streamed) {
    const current = stream.current
    const last = current?.pages.at(-1)
    return {
      items,
      search: last
        ? {
            pages: last.count,
            visits: last.visits,
            sorting: current?.pages.some((page) => page.provisional) ?? false,
            coverageComplete: last.coverageComplete,
            indexer: current?.indexer ?? last.indexer,
          }
        : null,
      isPending: enabled && shown === null,
      isPlaceholder: current === null && shown !== null,
      error: current?.error ? new Error(current.error) : null,
      hasNextPage: stream.hasNextPage,
      isFetchingNextPage: current?.loadingNext ?? false,
      fetchNextPage: stream.fetchNextPage,
      refetch: stream.refetch,
    }
  }

  const active = semantic ? smart : timeline
  const first = smart.data?.pages[0]
  // The AI search cannot filter by date or browser itself; a count would describe other rows.
  const clientFiltered = startTimeMs !== null || browserKind !== null
  return {
    items,
    search:
      semantic && first && !clientFiltered
        ? {
            pages: { value: first.total, exact: true },
            visits: null,
            sorting: false,
            coverageComplete: first.searchState?.coverageComplete ?? true,
            indexer: first.searchState?.indexer ?? null,
          }
        : null,
    isPending: enabled && (semantic || search.text === '') && active.isPending,
    isPlaceholder: active.isPlaceholderData,
    error: active.error,
    hasNextPage: active.hasNextPage,
    isFetchingNextPage: active.isFetchingNextPage,
    fetchNextPage: () => void active.fetchNextPage(),
    refetch: () => void active.refetch(),
  }
}

export interface BrowserOption {
  kind: string
  name: string
  profileIds: string[]
}

/** The browsers present in the archive, falling back to the ones discovered on this machine. */
export function useBrowserCatalog() {
  const snapshot = useSnapshot()
  const stats = useQuery({
    queryKey: [...queryKeys.archiveData, 'source-stats'],
    queryFn: sourcesClient.stats,
  })

  return useMemo(() => {
    const names = new Map<string, string>()
    const profiles = new Map<string, Set<string>>()
    const add = (profileId: string, browserName: string) => {
      const kind = browserKindOf(profileId)
      names.set(
        kind,
        supportedBrowsers.find((browser) => browser.key === kind)?.name ??
          browserName,
      )
      profiles.set(kind, (profiles.get(kind) ?? new Set()).add(profileId))
    }
    const archived = stats.data ?? []
    if (archived.length > 0) {
      for (const stat of archived) add(stat.profileId, stat.browserName)
    } else {
      for (const profile of snapshot.browserProfiles) {
        if (profile.historyExists) add(profile.profileId, profile.browserName)
      }
    }
    const options: BrowserOption[] = [...profiles].map(([kind, ids]) => ({
      kind,
      name: names.get(kind) ?? kind,
      profileIds: [...ids],
    }))
    const nameOf = (profileId: string) => {
      const kind = browserKindOf(profileId)
      return (
        names.get(kind) ??
        supportedBrowsers.find((browser) => browser.key === kind)?.name ??
        kind
      )
    }
    return { options, nameOf }
  }, [stats.data, snapshot.browserProfiles])
}

export interface SiteCount {
  domain: string
  visits: number
}

/**
 * Every site visited in the date filter, most visited first, a page at a time.
 * The backend sums the profiles of the chosen browser; no site is cut off.
 */
export function useTopSites(
  range: DayRange | null,
  profileIds: string[] | null,
) {
  const dashboard = useDashboard()
  const earliest = dashboard.data?.earliestVisitAt
  const start =
    range?.start ?? (earliest ? localDateKey(new Date(earliest)) : '2000-01-01')
  const end = range?.end ?? localDateKey(new Date())
  const ready = range !== null || !dashboard.isPending

  return useInfiniteQuery({
    enabled: ready,
    queryKey: key('top-sites', start, end, profileIds),
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }) => {
      const section = await insightsClient.topSites(
        { dateRange: { start, end }, profileIds: profileIds ?? [] },
        SITES_PAGE_SIZE,
        pageParam,
      )
      return {
        sites: section.data.items.map<SiteCount>((site) => ({
          domain: site.registrableDomain,
          visits: site.visitCount,
        })),
        nextCursor: section.data.nextCursor,
        stale: section.meta?.state === 'stale',
      }
    },
    getNextPageParam: (last) => last.nextCursor ?? undefined,
    placeholderData: keepPreviousData,
    refetchInterval: (query) =>
      query.state.data?.pages.some((page) => page.stale) ? 5_000 : false,
  })
}

/** The newest visit to a site: gives Sites rows a subtitle and an icon to look up. */
export function useLatestVisit(domain: string, filters: HistoryFilters | null) {
  return useQuery({
    queryKey: key('latest-visit', domain, filters),
    queryFn: async () => {
      const response = await explorerClient.queryHistory({
        ...(filters ?? {}),
        domain,
        sort: 'newest',
        limit: 1,
        includeTotal: false,
      })
      return response.items[0] ? fromEntry(response.items[0]) : null
    },
    staleTime: 10 * 60_000,
  })
}

/** Every star, newest first, a page at a time as the list scrolls. */
export function useStarList() {
  return useInfiniteQuery({
    queryKey: ['stars', 'list'],
    queryFn: ({ pageParam }) =>
      starsClient.listStars(null, 'recently_starred', pageParam),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.nextCursor,
  })
}

export function useStarCounts() {
  return useQuery({
    queryKey: ['stars', 'counts'],
    queryFn: starsClient.getStarCounts,
  })
}

export function useUrlDetail(url: string) {
  return useQuery({
    queryKey: key('url-detail', url),
    queryFn: () => urlDetailClient.get(url),
  })
}

export function useUrlAnnotation(url: string) {
  return useQuery({
    queryKey: ['annotations', 'url', url],
    queryFn: () => annotationsClient.getUrlAnnotation(url),
    staleTime: 0,
  })
}
