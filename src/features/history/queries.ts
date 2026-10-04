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
import { useMemo } from 'react'
import { annotationsClient } from '@/lib/backend-client/annotations'
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
  void client.invalidateQueries({ queryKey: key('list') })
  void client.invalidateQueries({ queryKey: key('search-total') })
}

const PAGE_SIZE = 100
const SEMANTIC_PAGE_SIZE = 50

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

function lexicalQuery(search: SearchSpec, filters: HistoryFilters) {
  return {
    q: search.text ? search.pattern : null,
    regexMode: search.mode === 'regex',
    sort:
      search.text && search.mode === 'full'
        ? ('relevance' as const)
        : ('newest' as const),
    ...filters,
  }
}

/** How much a search matched, once counted. */
export interface SearchTotals {
  pages: number
  /** Matching visits; unknown for semantic search, which returns pages only. */
  visits: number | null
  /**
   * The word matched more pages than one search ranks, so only the most
   * recently archived ones were ranked and counted: `pages` is "at least".
   */
  windowed: boolean
}

/** The pages of a visit list, flattened. */
export interface VisitList {
  items: VisitItem[]
  /** Search totals once counted; always null for the timeline. */
  totals: SearchTotals | null
  /** A loaded page of a full-text search came from a window of the matches. */
  windowed: boolean
  isPending: boolean
  isPlaceholder: boolean
  error: Error | null
  hasNextPage: boolean
  isFetchingNextPage: boolean
  fetchNextPage: () => void
  refetch: () => void
}

/**
 * The timeline and the search results share one list. The timeline lists
 * every visit; full-text and regex results list each page once
 * (`searchPages`), both read by cursor without an exact count. Semantic
 * results come from the AI search, which also returns one row per page.
 */
export function useVisitList(
  search: SearchSpec,
  filters: HistoryFilters,
  enabled: boolean,
): VisitList {
  const semantic = search.mode === 'semantic' && search.text !== ''
  const lexicalEnabled = enabled && !semantic && !search.regexError

  const lexical = useInfiniteQuery({
    enabled: lexicalEnabled,
    queryKey: key('list', search.text, search.mode, filters),
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) => {
      const query = {
        ...lexicalQuery(search, filters),
        limit: PAGE_SIZE,
        cursor: pageParam,
        includeTotal: false,
      }
      return search.text
        ? explorerClient.searchPages(query)
        : explorerClient.queryHistory(query)
    },
    getNextPageParam: (last: HistoryQueryResponse) =>
      last.hasNext ? (last.nextCursor ?? undefined) : undefined,
    placeholderData: keepPreviousData,
  })

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

  const totals = useSearchTotals(search, filters, lexicalEnabled)
  const active = semantic ? smart : lexical
  const { startTimeMs, endTimeMs, browserKind } = filters
  const clientFiltered =
    semantic && (startTimeMs !== null || browserKind !== null)

  const items = useMemo(() => {
    if (!semantic) {
      return (
        lexical.data?.pages.flatMap((page) => page.items.map(fromEntry)) ?? []
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
  }, [semantic, smart.data, lexical.data, startTimeMs, endTimeMs, browserKind])

  const semanticTotal = smart.data?.pages[0]?.total
  return {
    items,
    totals: semantic
      ? clientFiltered || semanticTotal === undefined
        ? null
        : { pages: semanticTotal, visits: null, windowed: false }
      : totals,
    windowed:
      !semantic &&
      (lexical.data?.pages.some((page) => page.windowed === true) ?? false),
    isPending: (lexicalEnabled || (enabled && semantic)) && active.isPending,
    isPlaceholder: active.isPlaceholderData,
    error: active.error,
    hasNextPage: active.hasNextPage,
    isFetchingNextPage: active.isFetchingNextPage,
    fetchNextPage: () => void active.fetchNextPage(),
    refetch: () => void active.refetch(),
  }
}

/** Counting matches is slower than fetching a page, so it runs beside the list, not before it. */
function useSearchTotals(
  search: SearchSpec,
  filters: HistoryFilters,
  enabled: boolean,
): SearchTotals | null {
  const query = useQuery({
    enabled: enabled && search.text !== '',
    queryKey: key('search-total', search.text, search.mode, filters),
    queryFn: () =>
      explorerClient.searchPages({
        ...lexicalQuery(search, filters),
        limit: 1,
        includeTotal: true,
      }),
  })
  const response = query.data
  const windowed = response?.windowed === true
  if (!response || !(response.totalExact || windowed)) return null
  return {
    pages: response.total,
    visits: response.totalVisits ?? null,
    windowed,
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

/** Top sites for the date filter, summed across the profiles of the chosen browser. */
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

  return useQuery({
    enabled: ready,
    queryKey: key('top-sites', start, end, profileIds),
    queryFn: async () => {
      const dateRange = { start, end }
      const sections = await Promise.all(
        (profileIds ?? [null]).map((profileId) =>
          insightsClient.topSites({ dateRange, profileId }, 200),
        ),
      )
      const merged = new Map<string, number>()
      for (const section of sections) {
        for (const site of section.data) {
          merged.set(
            site.registrableDomain,
            (merged.get(site.registrableDomain) ?? 0) + site.visitCount,
          )
        }
      }
      const sites: SiteCount[] = [...merged]
        .map(([domain, visits]) => ({ domain, visits }))
        .sort((a, b) => b.visits - a.visits)
      return {
        sites,
        stale: sections.some((section) => section.meta?.state === 'stale'),
      }
    },
    placeholderData: keepPreviousData,
    refetchInterval: (query) => (query.state.data?.stale ? 5_000 : false),
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

const STAR_LIST_LIMIT = 500

export function useStarList() {
  return useQuery({
    queryKey: ['stars', 'list'],
    queryFn: () =>
      starsClient.listStars(null, 'recently_starred', STAR_LIST_LIMIT),
  })
}

export function useStarCounts() {
  return useQuery({
    queryKey: ['stars', 'counts'],
    queryFn: starsClient.getStarCounts,
  })
}

/** Note text per page, for the Starred previews. */
export function useAnnotationList() {
  return useQuery({
    queryKey: ['annotations', 'list'],
    queryFn: () => annotationsClient.listUrlAnnotations(2000),
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
