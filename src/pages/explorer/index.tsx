/**
 * This module renders the History Explorer route and keeps the keyword-first, deep-linkable recall workflow honest even when optional AI features degrade.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ExplorerPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useLocation, useNavigate } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { SkeletonExplorer } from '../../components/primitives/skeleton'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { defaultExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  selectedAiProvider,
} from '../../lib/intelligence-ai-presentation'
import { evaluateOptionalAiAvailability } from '../../lib/optional-ai-availability'
import { optionalAiFeaturesAvailable } from '../../lib/release-capabilities'
import {
  browserLabel,
  historyFaviconLookupKey,
  historyOgImageLookupKey,
  isSearchResultUrl,
} from './helpers'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { useBrowseDayInsightsCache } from './hooks/use-browse-day-insights-cache'
import { useExplorerArchiveDensity } from './hooks/use-explorer-archive-density'
import { useExplorerData } from './hooks/use-explorer-data'
import { useExplorerFavicons } from './hooks/use-explorer-favicons'
import { useExplorerInfinitePages } from './hooks/use-explorer-infinite-pages'
import { useScrollDirection } from './hooks/use-scroll-direction'
import { useExplorerOgImages } from './hooks/use-explorer-og-images'
import { useExplorerUrlState } from './hooks/use-explorer-url-state'
import { ExplorerDetailPanel } from './panels/detail-panel'
import { ExplorerRuntimePanel } from './panels/runtime-panel'
import { SmartIndexStatusCallout } from './panels/smart-index-status'
import { SessionGroupPanel } from './panels/session-group'
import { TrailGroupPanel } from './panels/trail-group'
import {
  buildPaperExplorerCopy,
  buildPaperStarredViewCopy,
} from './paper-explorer-copy'
import {
  PaperFilterStrip,
  PaperStarredView,
  type PaperFilterStripFormState,
} from '../../components/explorer-paper'
import { PaperExplorerView } from './paper-view'
import { PaperSearchPanel } from './paper-search-panel'
import {
  buildPaperSearchRelevanceList,
  buildSmartScopeLine,
  deriveSmartIndexProgress,
} from './paper-search-helpers'
import { assistantHref } from '../../lib/intelligence-links'
import { PaperDetailPanelMount } from './paper-detail-panel-mount'
import { useVisitEnrichment } from './use-visit-enrichment'
import { useLocalAnnotations } from './use-local-annotations'
import { useDesktopAnnotations } from './use-desktop-annotations'
import { useDesktopStars } from './use-desktop-stars'
import { useStarredHub } from './use-starred-hub'
import { useStarredCount } from './use-starred-count'
import { hasStarredFacet } from './paper-search-filters'
import { hasDesktopCommandTransport } from '../../lib/runtime'
import type { StarListItem } from '../../lib/backend-client'
import type { HistoryEntry } from '../../lib/types/archive'
import type { AiSearchResultItem } from '../../lib/types/intelligence'
import type { ExplorerVisitSelection } from './types'

/**
 * Adapts a starred page (`list_stars` `url` item) into a `HistoryEntry` so the
 * Search surface can render the `is:starred` facet from the true starred set.
 * The synthetic `id` is negative + index-derived so it never collides with a
 * real history rowid; `visitedAt`/`visitTime` reuse `starredAt` (a real RFC3339
 * timestamp) so the day grouping stays honest. Selecting one opens via
 * `handleVisit(url)`, not the detail-panel id lookup.
 */
function starredItemToHistoryEntry(
  item: StarListItem,
  index: number,
): HistoryEntry {
  const visitTime = Date.parse(item.starredAt)
  return {
    id: -1 - index,
    profileId: '',
    url: item.entityKey,
    title: item.title || item.entityKey,
    domain: item.domain || item.entityKey,
    visitedAt: item.starredAt,
    visitTime: Number.isNaN(visitTime) ? 0 : visitTime,
    sourceVisitId: -1 - index,
  }
}

/**
 * Adapt one Smart (hybrid) search result into a `HistoryEntry` so selecting a
 * ranked AI row opens the SAME paper detail panel a keyword row opens. The AI
 * row carries a real `historyId`, so the panel binds to a real record; the
 * synthesized entry only fills the fields the panel header reads (no transition
 * / duration metadata is available from the search result, which the panel
 * already renders as "—" when absent).
 */
function aiItemToHistoryEntry(item: AiSearchResultItem): HistoryEntry {
  const visitTime = Date.parse(item.visitedAt)
  return {
    id: item.historyId,
    profileId: item.profileId,
    url: item.url,
    title: item.title ?? item.url,
    domain: item.domain,
    visitedAt: item.visitedAt,
    visitTime: Number.isNaN(visitTime) ? 0 : visitTime,
    sourceVisitId: item.historyId,
  }
}

/**
 * How often (ms) the in-surface Smart-index status re-reads the queue while a
 * backfill is active (REACH-B B1). 4s is frequent enough to feel live for a
 * minutes-long backfill without hammering the IPC bridge; the poll is bounded —
 * it only runs while a build is active and is cleared on completion/unmount.
 */
const SMART_INDEX_POLL_INTERVAL_MS = 4000

/**
 * Renders the explorer route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
 */
export function ExplorerPage() {
  const {
    error: shellError,
    loading: shellLoading,
    refreshKey,
    refreshAppData,
    refreshRuntimeStatus,
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
    snapshot,
  } = useShellData()
  const { language, t, ns } = useI18n()
  const { activeProfileId } = useProfileScope()
  const commonT = ns('common')
  const explorerT = ns('explorer')
  const intelligenceT = ns('intelligence')
  // `/search` mounts the same ExplorerPage component as `/explorer`, but
  // we want it to land on the paper Search surface by default without
  // requiring the URL to carry `?surface=search`. The two should be
  // interchangeable signals: the route pathname or the explicit param.
  const routeIsSearchPath = useLocation().pathname === '/search'
  const navigate = useNavigate()

  const {
    activeFilters,
    browserKinds,
    clearAllFilters,
    currentQuery,
    end,
    groupedDateRange,
    handleNextSemanticPage,
    handlePreviousSemanticPage,
    mode,
    persistRecentSearch,
    profileId,
    queryInput,
    regexMode,
    regexValid,
    searchParams,
    semanticQuery,
    semanticTrail,
    setQueryInput,
    setRecentSearches,
    setSearchParams,
    start,
    updateParam,
    view,
  } = useExplorerUrlState({
    activeProfileId,
    explorerT,
    language,
    selectedProfileIds: snapshot?.config.selectedProfileIds ?? [],
  })

  const archiveReady = Boolean(
    snapshot?.config.initialized && snapshot.archiveStatus.unlocked,
  )
  const aiMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const queueStatus = runtimeStatus.aiQueue
  const embeddingProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'embedding')
    : null
  const optionalAiAvailability = useMemo(
    () =>
      evaluateOptionalAiAvailability({
        releaseEnabled: optionalAiFeaturesAvailable,
        aiEnabled:
          snapshot?.config.ai.enabled &&
          snapshot.config.ai.semanticIndexEnabled,
        embeddingProviderId: embeddingProvider?.id ?? null,
        aiStatusState: snapshot?.aiStatus.state ?? null,
      }),
    [
      embeddingProvider?.id,
      snapshot?.aiStatus.state,
      snapshot?.config.ai.enabled,
      snapshot?.config.ai.semanticIndexEnabled,
    ],
  )
  const requestKey = useMemo(
    () => JSON.stringify({ currentQuery, refreshKey }),
    [currentQuery, refreshKey],
  )
  const semanticRequestKey = useMemo(
    () => JSON.stringify({ semanticQuery, mode, refreshKey }),
    [mode, refreshKey, semanticQuery],
  )
  const historyBlockedByInvalidRegex =
    archiveReady && regexMode && Boolean(queryInput.trim()) && !regexValid
  const groupedSelectionKey = `${view}:${groupedDateRange.start}:${groupedDateRange.end}:${profileId ?? 'all'}`
  const [selectedGroupedVisitState, setSelectedGroupedVisitState] = useState<{
    key: string
    visit: ExplorerVisitSelection
  } | null>(null)
  const localAnnotations = useLocalAnnotations()
  const desktopAnnotations = useDesktopAnnotations()
  const annotations = hasDesktopCommandTransport()
    ? desktopAnnotations
    : localAnnotations
  const stars = useDesktopStars()
  // STAR-1: a bounded aggregate count behind the discoverable "Open starred"
  // entry badge. The token bumps on every star toggle so the badge stays honest
  // after the user stars/un-stars without ever listing the archive.
  const [starCountToken, setStarCountToken] = useState(0)
  const starredCount = useStarredCount(starCountToken)
  // Single chokepoint for star toggles so the count badge re-reads its bounded
  // aggregate after any star change, regardless of which surface fired it.
  const toggleStar = useCallback(
    (kind: 'url' | 'domain', key: string) => {
      stars.toggle(kind, key)
      setStarCountToken((token) => token + 1)
    },
    [stars],
  )
  // Starred hub: a focused Explorer mode (NOT a 4th nav item) entered via
  // `?surface=starred`. The hub owns its own paginated read model.
  const surfaceIsStarred = searchParams.get('surface') === 'starred'
  // The `is:starred` Search facet reads the SAME `list_stars` model the hub
  // uses (not the loaded keyword page), so it can report a TRUE starred set with
  // an honest total instead of an in-memory page slice + wrong count.
  const queryHasStarredFacet = hasStarredFacet(queryInput)
  // REACH-B: Smart (relevance) search is the unified AI tab — it covers both the
  // real `hybrid` URL mode and the legacy `semantic` alias. `is:starred` always
  // stays on the keyword (day-grouped) layout, so a starred-facet query never
  // flips into the ranked view even while in Smart mode.
  //
  // M-1: regex takes precedence in the panel/mapper
  // (`paperSearchModeFromExplorerState`: `if (regexMode) return 'regex'`), so a
  // `?mode=hybrid&regex=1` (or legacy `semantic&regex=1`) deep link renders the
  // day-grouped regex layout. The route must agree, otherwise it resolves the
  // detail selection against `semanticResults` and hydrates stars from semantic
  // items while the rendered layout is keyword/regex. Requiring `!regexMode` here
  // keeps the hero's selected tab and the rendered layout in lockstep.
  const smartSearchActive =
    mode !== 'keyword' && !regexMode && !queryHasStarredFacet
  const starredHub = useStarredHub(surfaceIsStarred || queryHasStarredFacet)
  const [paperDetailOpen, setPaperDetailOpen] = useState(false)
  const labels = useMemo(
    () => ({
      exportFailed: explorerT('exportFailed'),
      loadingArchive: explorerT('loadingArchive'),
      queryFailedTitle: explorerT('queryFailedTitle'),
      semanticRecallDegradedTitle: explorerT('semanticRecallDegradedTitle'),
      visitFailed: explorerT('visitFailed'),
    }),
    [explorerT],
  )
  const {
    cachedHistoryResults,
    handleIndexAction,
    handleProviderProbe,
    handleQueueAction,
    handleVisit,
    indexAction,
    intelligenceError,
    providerProbe,
    queryState,
    queueAction,
    selectedId,
    semanticState,
    setQueueAction,
    setSelectedId,
  } = useExplorerData({
    archiveReady,
    backgroundPrefetchPages:
      snapshot?.config.explorerBackgroundPrefetchPages ??
      defaultExplorerBackgroundPrefetchPages,
    cacheToken: refreshKey,
    currentQuery,
    embeddingProviderId: embeddingProvider?.id ?? null,
    end,
    historyBlockedByInvalidRegex,
    labels,
    mode,
    optionalAiAvailable: optionalAiAvailability.available,
    view,
    persistRecentSearch,
    refreshAppData,
    refreshRuntimeStatus,
    requestKey,
    semanticQuery,
    semanticRequestKey,
    setRecentSearches,
    start,
  })

  const results =
    archiveReady &&
    (historyBlockedByInvalidRegex || queryState.requestKey === requestKey)
      ? queryState.results
      : cachedHistoryResults
  const semanticResults =
    archiveReady && semanticState.requestKey === semanticRequestKey
      ? semanticState.results
      : null
  const semanticError =
    archiveReady && semanticState.requestKey === semanticRequestKey
      ? semanticState.error
      : null
  const semanticLoading =
    archiveReady &&
    optionalAiAvailability.available &&
    mode !== 'keyword' &&
    Boolean(semanticQuery.query) &&
    semanticState.requestKey !== semanticRequestKey
  const error =
    archiveReady &&
    (historyBlockedByInvalidRegex || queryState.requestKey === requestKey)
      ? queryState.error
      : null
  const loading =
    archiveReady &&
    !historyBlockedByInvalidRegex &&
    queryState.requestKey !== requestKey &&
    !cachedHistoryResults
  const stagedResults =
    archiveReady && !historyBlockedByInvalidRegex && view === 'time'
      ? queryState.results
      : null
  const visibleTimeResults = results ?? (loading ? stagedResults : null)
  const archiveDensity = useExplorerArchiveDensity({
    archiveReady,
    profileId,
    refreshKey,
  })
  // Backend-backed per-day insights cache. Replaces the previous
  // scroll-coupled `aggregateDayInsights(day)` client aggregation —
  // see feedback-2026-05-25 §3.1. Each visible day in the contact
  // sheet triggers a single backend fetch (deduped per refreshKey +
  // profile) so the sparkline / top-domains / top-URLs strip reflects
  // the FULL day rather than just the cards already scrolled into
  // view. The contact sheet keeps the client-side aggregator as a
  // fallback while the backend reply is in flight.
  const browseDayInsightsCache = useBrowseDayInsightsCache({
    profileId,
    refreshKey,
  })
  // Infinite scroll is the default for the paper Browse time view in both
  // card and list mode, with or without a `?date=` filter — landing on a
  // single day should still keep scrolling through that day's pages
  // without forcing the user to discover the "Older" footer button. Search
  // surface, grouped views, and an invalid-regex block still fall back to
  // pagination because they have a fundamentally different shape (one-off
  // filtered set / pre-grouped panels / blocked-with-error).
  const surfaceIsSearch =
    routeIsSearchPath || searchParams.get('surface') === 'search'
  const infiniteDisabled =
    !archiveReady ||
    view !== 'time' ||
    surfaceIsSearch ||
    historyBlockedByInvalidRegex
  // BROWSE-VIRT §1.2: directional prefetch. The hook samples
  // window.scrollY per RAF and only emits 'down' when the user
  // sustains a downward scroll, so a quick wobble won't flip the
  // prefetch budget.
  const scrollDirection = useScrollDirection()
  const {
    extraItems: infiniteExtraItems,
    loadedPageCount: infiniteLoadedPageCount,
    loadingMore: infiniteLoadingMore,
    canLoadMore: infiniteCanLoadMore,
    capReached: infiniteCapReached,
    error: infiniteError,
    loadMore: infiniteLoadMore,
  } = useExplorerInfinitePages({
    query: currentQuery,
    headResults: visibleTimeResults,
    disabled: infiniteDisabled,
    cacheToken: refreshKey,
    scrollDirection,
  })
  // Combine the head page + every accumulated infinite-scroll page into one
  // synthetic `HistoryQueryResponse` so the lazy favicon / og:image hooks
  // see every row the contact sheet is about to render, not just page 1.
  // Without this, rows past page 1 never trigger an icon lookup and the
  // list mode drowns in colored swatches.
  const combinedTimeResults = useMemo(() => {
    if (!visibleTimeResults) return null
    if (infiniteDisabled) return visibleTimeResults
    return {
      ...visibleTimeResults,
      items: [...visibleTimeResults.items, ...infiniteExtraItems],
    }
  }, [infiniteDisabled, infiniteExtraItems, visibleTimeResults])
  const { faviconCache } = useExplorerFavicons({
    cacheToken: refreshKey,
    loading,
    results: combinedTimeResults,
  })
  // Fold the legacy `fetchEnabled` kill switch into the effective fetch
  // mode so the hook only ever sees the modern tri-state. Off (either by
  // explicit mode choice or the legacy switch) blocks the implicit on-
  // demand enqueue; cached rows still hydrate. Defaults to 'background'
  // when the config field hasn't been hydrated yet so first paint doesn't
  // suppress the fetch path on a brand-new install.
  const ogImageFetchMode = useMemo(() => {
    const settings = snapshot?.config.ogImage
    if (!settings) return 'background' as const
    if (!settings.fetchEnabled) return 'off' as const
    return settings.fetchMode
  }, [snapshot?.config.ogImage])
  const { ogImageCache } = useExplorerOgImages({
    cacheToken: refreshKey,
    loading,
    results: combinedTimeResults,
    fetchMode: ogImageFetchMode,
  })
  const renderedTimeResults = useMemo(() => {
    if (!combinedTimeResults) {
      return null
    }
    return {
      ...combinedTimeResults,
      items: combinedTimeResults.items.map((item) => {
        // Search-engine result pages legitimately advertise a knowledge-
        // panel `<meta og:image>` that describes the top entity, not the
        // SERP itself. Suppressing the og:image for those rows keeps the
        // Browse row's icon honest (Google's favicon or the swatch, never
        // a wrong "Yoshinoya logo on a Google search" hand-off).
        const suppressOgImage = isSearchResultUrl(item.url)
        const hydratedOgImage = suppressOgImage
          ? null
          : (item.ogImage ??
            ogImageCache.get(historyOgImageLookupKey(item.url)) ??
            null)
        return {
          ...item,
          favicon:
            item.favicon ??
            faviconCache.get(
              historyFaviconLookupKey(item.profileId, item.url, item.visitTime),
            ) ??
            null,
          ogImage: hydratedOgImage,
        }
      }),
    }
  }, [combinedTimeResults, faviconCache, ogImageCache])
  // Build the paper Browse filter strip from the URL state. The chip list
  // and `clearAllFilters` already exist on the hook; the popover form
  // mirrors the active URL params so the strip + chips edit the same
  // dimensions the route already understands. Snapshot-derived browser /
  // profile dropdowns let the user pick from the same scope as the rest
  // of the route, not free-text.
  const filterStripFormState = useMemo<PaperFilterStripFormState>(
    () => ({
      domain: searchParams.get('domain') ?? '',
      browserKind: searchParams.get('browserKind') ?? '',
      profileId: searchParams.get('profileId') ?? '',
      start: searchParams.get('start') ?? '',
      end: searchParams.get('end') ?? '',
      regexMode,
    }),
    [regexMode, searchParams],
  )
  const browserOptions = useMemo(
    () =>
      browserKinds.map((kind: string) => ({
        value: kind,
        label: browserLabel(kind),
      })),
    [browserKinds],
  )
  const profileOptions = useMemo(
    () =>
      (snapshot?.config.selectedProfileIds ?? []).map((id) => ({
        value: id,
        label: `${browserLabel(id.split(':')[0])} · ${profileIdLabel(id)}`,
      })),
    [snapshot?.config.selectedProfileIds],
  )
  const filterStripCopy = useMemo(
    () => ({
      addFilter: explorerT('paperBrowse.filterStripAdd'),
      clearAll: explorerT('paperBrowse.filterStripClearAll'),
      emptyHint: explorerT('paperBrowse.filterStripEmptyHint'),
      removeFilterAria: explorerT('paperBrowse.filterStripRemoveAria'),
      popoverTitle: explorerT('paperBrowse.filterPopoverTitle'),
      fieldDomain: explorerT('paperBrowse.filterPopoverFieldDomain'),
      fieldBrowser: explorerT('paperBrowse.filterPopoverFieldBrowser'),
      fieldProfile: explorerT('paperBrowse.filterPopoverFieldProfile'),
      fieldStart: explorerT('paperBrowse.filterPopoverFieldStart'),
      fieldEnd: explorerT('paperBrowse.filterPopoverFieldEnd'),
      fieldRegex: explorerT('paperBrowse.filterPopoverFieldRegex'),
      selectAllBrowsers: explorerT(
        'paperBrowse.filterPopoverSelectAllBrowsers',
      ),
      selectAllProfiles: explorerT(
        'paperBrowse.filterPopoverSelectAllProfiles',
      ),
      applyLabel: explorerT('paperBrowse.filterPopoverApply'),
      closeLabel: explorerT('paperBrowse.filterPopoverClose'),
    }),
    [explorerT],
  )
  const filterStripChips = useMemo(
    () => activeFilters.filter((f): f is NonNullable<typeof f> => Boolean(f)),
    [activeFilters],
  )
  const handleFilterStripApply = useCallback(
    (next: PaperFilterStripFormState) => {
      const params = new URLSearchParams(searchParams)
      const setOrDelete = (key: string, value: string) => {
        const trimmed = value.trim()
        if (trimmed.length === 0) params.delete(key)
        else params.set(key, trimmed)
      }
      setOrDelete('domain', next.domain)
      setOrDelete('browserKind', next.browserKind)
      setOrDelete('profileId', next.profileId)
      setOrDelete('start', next.start)
      setOrDelete('end', next.end)
      if (next.regexMode) params.set('regex', '1')
      else params.delete('regex')
      params.delete('page')
      setSearchParams(params)
    },
    [searchParams, setSearchParams],
  )
  const handleFilterStripRemove = useCallback(
    (id: string) => updateParam(id, null),
    [updateParam],
  )
  // Visible entry point into the Starred hub. Without this the hub was only
  // reachable by hand-typing `?surface=starred`. Lives in the Browse filter
  // strip so it sits in the toolbar the user already scans, and writes the
  // surface param via the same URL state the rest of the route uses.
  const openStarredHub = useCallback(() => {
    const next = new URLSearchParams(searchParams)
    next.set('surface', 'starred')
    next.delete('q')
    next.delete('page')
    setSearchParams(next)
  }, [searchParams, setSearchParams])
  // Leave the hub for the Browse contact sheet (back affordance + empty CTA).
  const closeStarredHub = useCallback(() => {
    const next = new URLSearchParams(searchParams)
    next.delete('surface')
    setSearchParams(next)
  }, [searchParams, setSearchParams])
  const paperFilterStrip = useMemo(
    () => (
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
        <PaperFilterStrip
          chips={filterStripChips}
          copy={filterStripCopy}
          formState={filterStripFormState}
          browserOptions={browserOptions}
          profileOptions={profileOptions}
          onRemove={handleFilterStripRemove}
          onClearAll={clearAllFilters}
          onApply={handleFilterStripApply}
          testId="paper-filter-strip"
        />
        <button
          type="button"
          onClick={openStarredHub}
          data-testid="explorer-open-starred"
          className="border-border-light text-ink-muted hover:text-accent hover:border-accent ml-auto inline-flex items-center gap-1.5 rounded-paper border px-2.5 py-0.5 font-mono text-[10.5px] uppercase tracking-[0.08em] transition-colors"
        >
          <svg
            viewBox="0 0 24 24"
            width={12}
            height={12}
            fill="currentColor"
            stroke="currentColor"
            strokeWidth={1.8}
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M12 3.5l2.6 5.3 5.9.9-4.25 4.15 1 5.85L12 17.9l-5.25 2.65 1-5.85L3.5 9.7l5.9-.9z" />
          </svg>
          {explorerT('star.hubOpen')}
          {/*
            STAR-1: honest count badge. Renders only once a real bounded count
            lands AND there is something starred — a fresh/empty archive shows
            no badge instead of a misleading "0".
          */}
          {starredCount.loaded && starredCount.total > 0 ? (
            <span
              data-testid="explorer-starred-count"
              aria-label={explorerT('star.hubCountAria', {
                count: starredCount.total,
              })}
              className="bg-accent-soft text-accent-text rounded-pill ml-0.5 inline-flex min-w-[16px] items-center justify-center px-1 py-px text-[9.5px] font-semibold tabular-nums"
            >
              {starredCount.total.toLocaleString(language)}
            </span>
          ) : null}
        </button>
      </div>
    ),
    [
      browserOptions,
      clearAllFilters,
      explorerT,
      filterStripChips,
      filterStripCopy,
      filterStripFormState,
      handleFilterStripApply,
      handleFilterStripRemove,
      language,
      openStarredHub,
      profileOptions,
      starredCount.loaded,
      starredCount.total,
    ],
  )

  // Search the rendered (head + infinite-scroll-accumulated) item list,
  // not just `visibleTimeResults` page 1 — otherwise selecting a row from
  // page 2+ opens the detail panel against the first page-1 entry.
  //
  // Returns `null` when the previously-selected id no longer exists in
  // the rendered pool (e.g. the user just applied a filter that dropped
  // the row). Falling back to `items[0]` would silently rebind the open
  // detail panel to a DIFFERENT record under the same `paperDetailOpen`,
  // and the mount's `onUpdateNotes` would then write any in-flight
  // debounce flush against the wrong URL.
  const selectedEntryPool = !loading ? renderedTimeResults : null
  // Smart (ranked) rows are NOT in the keyword pool, but they carry a real
  // historyId so selecting one must still open the detail panel. Resolve the
  // selection against the ranked AI items first when in Smart search, then fall
  // back to the keyword pool — keyword selection is byte-for-byte unchanged.
  const selectedAiEntry =
    smartSearchActive && selectedId != null
      ? (semanticResults?.items.find((item) => item.historyId === selectedId) ??
        null)
      : null
  const selectedEntry =
    (selectedEntryPool?.items.find((item) => item.id === selectedId) ?? null) ||
    (selectedAiEntry ? aiItemToHistoryEntry(selectedAiEntry) : null)
  const selectedGroupedVisit =
    selectedGroupedVisitState?.key === groupedSelectionKey
      ? selectedGroupedVisitState.visit
      : null

  // Auto-close the detail panel when the selected entry vanishes from
  // the rendered pool (e.g. a filter dropped the row). Without this,
  // `paperDetailOpen` stays true while `selectedEntry` is null and the
  // mount renders nothing — the user sees the backdrop / overlay shell
  // with no content.
  //
  // Done as a render-time setState (React 19's blessed derived-state
  // pattern) rather than an effect so we don't pay an extra commit cycle
  // for what's a pure synchronous derivation; the guard ensures it never
  // loops because the second render observes `paperDetailOpen === false`
  // and skips the branch.
  if (paperDetailOpen && !selectedEntry && !loading) {
    setPaperDetailOpen(false)
  }

  // Lazily hydrate the star status for the single page the detail panel has
  // open. Batched + deduped inside the hook, so this never fans out across the
  // archive — it asks about exactly one URL when the panel opens.
  const selectedEntryUrl = selectedEntry?.url ?? null
  useEffect(() => {
    if (paperDetailOpen && selectedEntryUrl) {
      stars.hydrate('url', [selectedEntryUrl])
    }
  }, [paperDetailOpen, selectedEntryUrl, stars])

  // Site-content enrichment for the open visit (W-ENRICH-1). The target is only
  // built while the panel is open so the read is bounded to exactly the one
  // visit the user opened — never the whole rendered pool. Consent is read from
  // the shell snapshot (hard-default-OFF); the hook never fetches on its own.
  const enrichmentTarget = useMemo(
    () =>
      paperDetailOpen && selectedEntry
        ? {
            historyId: selectedEntry.id,
            profileId: selectedEntry.profileId,
            url: selectedEntry.url,
            title: selectedEntry.title,
          }
        : null,
    [paperDetailOpen, selectedEntry],
  )
  const contentFetchEnabled = snapshot?.config.ai.contentFetchEnabled ?? false
  const visitEnrichment = useVisitEnrichment({
    target: enrichmentTarget,
    fetchEnabled: contentFetchEnabled,
  })

  // Batch-hydrate star status for the URLs currently rendered in the Browse
  // time view or the Search results (both read `renderedTimeResults`). Bounded
  // by the render window (one page of rows), deduped inside the hook — never a
  // full-archive scan.
  const visibleTimeUrls = useMemo(() => {
    if (view !== 'time' && !surfaceIsSearch) return []
    // In Smart mode the search surface renders the RANKED AI rows, not the
    // day-grouped keyword pool, so hydrate stars for exactly those ranked urls
    // (bounded to the current page of ranked results — never a full scan). The
    // keyword/Browse path keeps hydrating the rendered time results unchanged.
    const source =
      surfaceIsSearch && smartSearchActive
        ? semanticResults?.items
        : renderedTimeResults?.items
    return (
      source
        ?.map((item) => item.url)
        .filter((url): url is string => Boolean(url)) ?? []
    )
  }, [
    view,
    surfaceIsSearch,
    smartSearchActive,
    semanticResults,
    renderedTimeResults,
  ])
  const visibleTimeUrlsKey = visibleTimeUrls.join('\n')
  useEffect(() => {
    if (visibleTimeUrls.length > 0) {
      stars.hydrate('url', visibleTimeUrls)
    }
    // visibleTimeUrlsKey collapses the array identity into a stable string so
    // the effect only re-runs when the actual URL set changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visibleTimeUrlsKey, stars])

  // `is:starred` advanced-search facet. Earlier this filtered only the loaded
  // keyword page in memory and reported `length` as the total — wrong on both
  // counts (later-page starred pages were missed, and a bare `is:starred`
  // keyword search returns garbage rows to filter). It now renders the TRUE
  // starred page set from `list_stars` (the hub's read model) with an honest
  // total, so the facet means "every page you've starred", not "starred rows
  // that happen to be on page 1".
  const starredSearchEntries = useMemo<HistoryEntry[]>(() => {
    if (!queryHasStarredFacet) return []
    return starredHub.items
      .filter((item) => item.entityKind === 'url')
      .map((item, index) => starredItemToHistoryEntry(item, index))
  }, [queryHasStarredFacet, starredHub.items])
  const searchEntries = useMemo(
    () =>
      queryHasStarredFacet
        ? starredSearchEntries
        : (renderedTimeResults?.items ?? []),
    [queryHasStarredFacet, starredSearchEntries, renderedTimeResults],
  )
  // Map the backend hybrid results into the shared paper result-row shape. The
  // adapter stamps `matchReason` + a `scoreBand`-derived relevance pill and the
  // real `historyId` so selecting a row opens the detail panel exactly like a
  // keyword row. Pure/cheap (no network), so it can live in render.
  const rankedSearchEntries = useMemo(
    () =>
      semanticResults
        ? buildPaperSearchRelevanceList(
            semanticResults.items,
            intelligenceT,
            explorerT('paperSearchView.enrichmentSourceGeneric'),
          )
        : [],
    [semanticResults, intelligenceT, explorerT],
  )
  const optionalAiReason = optionalAiAvailability.reason
  // The release flag is a hard-coded `true` const in this build, so
  // `evaluateOptionalAiAvailability` can never return `release-deferred` here —
  // every reachable unavailable reason is a user-fixable configuration gap. The
  // helper keeps `release-deferred` as a defensive path (covered by its own unit
  // tests), but this surface only renders the three actionable repair states.
  const optionalAiFixableReason =
    optionalAiReason === 'ai-disabled' ||
    optionalAiReason === 'no-embedding-provider' ||
    optionalAiReason === 'embedding-provider-error'
  const optionalAiUnavailableTitle =
    optionalAiReason === 'ai-disabled'
      ? explorerT('optionalAiDisabledTitle')
      : optionalAiReason === 'no-embedding-provider'
        ? explorerT('optionalAiNoProviderTitle')
        : explorerT('optionalAiProviderErrorTitle')
  const optionalAiUnavailableBody =
    optionalAiReason === 'ai-disabled'
      ? explorerT('optionalAiDisabledBody')
      : optionalAiReason === 'no-embedding-provider'
        ? explorerT('optionalAiNoProviderBody')
        : explorerT('optionalAiProviderErrorBody')

  const paperSearchSurface =
    routeIsSearchPath || searchParams.get('surface') === 'search'

  // REACH-B Smart-search surface plumbing. All gated behind `smartSearchActive`
  // so the keyword/regex/`is:starred` paths read `null`/defaults and stay
  // byte-for-byte unchanged.
  const smartAvailable = optionalAiAvailability.available
  // Prev/next cursor pagination for the ranked list — reuses the route's
  // existing `semanticTrail` + `handleNext/PreviousSemanticPage`. The page
  // number is the trail depth + 1 (page 1 has an empty trail). The next cursor
  // is normalized to `string | null` once here so the `onNext` closure stays a
  // single straight call (no inline coalescing branch to leave uncovered).
  const smartNextCursor = semanticResults?.nextCursor ?? null
  const smartPagination =
    smartSearchActive && semanticResults
      ? {
          prevDisabled: semanticTrail.length === 0,
          nextDisabled: !smartNextCursor,
          onPrev: handlePreviousSemanticPage,
          onNext: () => handleNextSemanticPage(smartNextCursor),
          page: semanticTrail.length + 1,
          // I2: the honest total ranked count from `AiSearchResponse.total`, so
          // the summary can say "Page N · M ranked" instead of a bare ordinal.
          total: semanticResults.total,
        }
      : null
  // REACH-B B1: the in-surface CTA must reflect the LIVE queue truth, not a
  // blink. `buildAiIndex` only ENQUEUES a background backfill (the real work
  // runs minutes-to-tens-of-minutes on the worker), so we derive the build phase
  // from the queue read model — refreshed by the bounded poll below while a build
  // is active — never from the instantly-resolved enqueue call. `pendingAction`
  // carries the local "just clicked Build" intent across the gap before the
  // first poll observes the new job. `snapshot` is guaranteed present here (the
  // search surface renders only after the `!snapshot` early returns), but read it
  // defensively via `?? null`/the AiIndexStatus fields so this stays render-safe.
  const smartIndexProgress = deriveSmartIndexProgress({
    queueStatus,
    snapshotAiStatus: snapshot?.aiStatus ?? null,
    pendingAction: Boolean(indexAction),
  })
  // I3: scope / freshness micro-line for the ranked header — index coverage +
  // last-indexed, from real status fields only (omitted when unavailable). Gated
  // to Smart search so the keyword path never computes it.
  const smartScopeLine = smartSearchActive
    ? buildSmartScopeLine({
        indexedItems: smartIndexProgress.indexedItems,
        lastIndexedAt: snapshot?.aiStatus.lastIndexedAt,
        language,
        explorerT,
      })
    : null
  // REACH-B B1: keep the in-surface index status fresh while a backfill is in
  // flight. The shell has no standing poll for `runtimeStatus.aiQueue`, so we add
  // a BOUNDED one here: it ticks `refreshRuntimeStatus()` every few seconds ONLY
  // while the Smart surface is showing AND a build is active, and the cleanup
  // clears the interval the instant the build finishes (the effect re-runs with
  // `shouldPoll === false`) or the route unmounts. No perpetual polling, no
  // main-thread block — `refreshRuntimeStatus` is an async IPC read that resolves
  // off the render path.
  const shouldPollQueue =
    paperSearchSurface &&
    smartSearchActive &&
    smartAvailable &&
    smartIndexProgress.active
  useEffect(() => {
    if (!shouldPollQueue) return
    const intervalId = window.setInterval(() => {
      void refreshRuntimeStatus()
    }, SMART_INDEX_POLL_INTERVAL_MS)
    return () => {
      window.clearInterval(intervalId)
    }
  }, [shouldPollQueue, refreshRuntimeStatus])
  // Smart-mode error routes through the same `aboveResultsCallout` slot the
  // keyword path uses, so the composer never unmounts mid-error.
  const smartAboveResultsCallout =
    smartSearchActive && semanticError
      ? {
          tone: 'blocked' as const,
          eyebrow: explorerT('semanticStatusEyebrow'),
          title: explorerT('semanticRecallDegradedTitle'),
          body: semanticError,
        }
      : null

  // The full-page EmptyState below should only fire when the zero-result
  // came from an active filter on the *Browse* surface — a palette / hash
  // search query, a domain facet, a browserKind filter, etc. For pure
  // date browsing (Calendar, year rail, day-nav arrows) we want the
  // contact-sheet shell to stay visible so users can keep navigating;
  // the inner "Nothing here yet" copy already handles the "this day has
  // no visits" case without hijacking the whole route.
  //
  // The Search surface is excluded entirely: PaperSearchView owns its
  // own in-place "no matches" state (italic-serif "memory is patient")
  // and an editable hero composer. Letting the full-page EmptyState fire
  // there would unmount the composer and trap the user — exactly the
  // bug captured in feedback-2026-05-25 §3.2 B.
  const hasActiveQueryOrFilter =
    !paperSearchSurface &&
    (queryInput.trim() !== '' ||
      (searchParams.get('q') ?? '').trim() !== '' ||
      Boolean(searchParams.get('domain')) ||
      Boolean(searchParams.get('browserKind')))

  if (shellLoading && !snapshot) {
    return <SkeletonExplorer label={t('common.loadingExplorer')} />
  }

  if (shellError && !snapshot) {
    return (
      <section className="page-shell">
        <ErrorState
          title={explorerT('couldNotLoadTitle')}
          description={shellError}
        />
      </section>
    )
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('common.initializeFirst')}
            </Link>
          }
          description={explorerT('uninitializedDescription')}
          eyebrow={explorerT('eyebrow')}
          title={explorerT('uninitializedTitle')}
        />
      </section>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <section className="page-shell">
        <PermissionGate
          detail={explorerT('lockedDescription')}
          eyebrow={explorerT('lockedEyebrow')}
          title={explorerT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {t('dashboard.reviewSecurity')}
          </Link>
        </PermissionGate>
      </section>
    )
  }

  // L-3: `snapshot` is narrowed non-null past the early returns above, so derive
  // the ready-state title from a guaranteed `AiIndexStatus` here — a plain
  // `string`, no `aiMeta?.label ?? …` coalescing and no `v8 ignore`. (The old
  // standalone `smartIndexReadyTitle` was computed before these guards, so during
  // loading its `?? eyebrow` fallback executed with its output discarded — the
  // exact "reachable but never displayed" mismatch the v8-ignore comment denied.)
  const smartIndexReadyTitle = aiStatusMeta(
    snapshot.aiStatus,
    intelligenceT,
  ).label

  return (
    <section className="page-shell explorer-page" data-testid="explorer-page">
      {/*
        Star write failures must not revert silently. The optimistic toggle in
        `useDesktopStars` rolls the cache back on a failed set/unset and records
        `lastError`; surface it here as a `role="alert"` callout (mirroring the
        notes save-error pattern in paper-detail-panel.tsx) so the user knows the
        archive did not save their star.
      */}
      {stars.lastError ? (
        <div role="alert" data-testid="explorer-star-error">
          <StatusCallout
            tone="blocked"
            eyebrow={explorerT('star.hubEyebrow')}
            title={explorerT('star.saveError')}
            body={stars.lastError}
          />
        </div>
      ) : null}

      {optionalAiFixableReason ? (
        <StatusCallout
          tone={
            optionalAiReason === 'embedding-provider-error' ? 'blocked' : 'info'
          }
          eyebrow={explorerT('semanticStatusEyebrow')}
          title={optionalAiUnavailableTitle}
          body={optionalAiUnavailableBody}
          actions={
            <Link className="btn-secondary" to="/settings#settings-ai">
              {explorerT('optionalAiOpenSettings')}
            </Link>
          }
        />
      ) : null}

      {aiMeta && mode !== 'keyword' && optionalAiAvailability.available && (
        <ExplorerRuntimePanel
          aiMeta={aiMeta}
          embeddingProvider={embeddingProvider}
          explorerT={explorerT}
          indexAction={indexAction}
          intelligenceError={intelligenceError}
          language={language}
          onBuildIndex={() =>
            void handleIndexAction(explorerT('buildingIndexAction'), {
              fullRebuild: false,
              clearOnly: false,
            })
          }
          onCancelJob={(jobId) =>
            void handleQueueAction(explorerT('cancellingJobAction'), () =>
              backend.cancelAiJob(jobId),
            )
          }
          onClearIndex={() =>
            void handleIndexAction(explorerT('clearingIndexAction'), {
              fullRebuild: false,
              clearOnly: true,
            })
          }
          onDrainQueue={() =>
            void handleQueueAction(explorerT('runningQueueAction'), () =>
              backend.runAiQueueJobs(2),
            )
          }
          onFullRebuild={() =>
            void handleIndexAction(explorerT('rebuildingIndexAction'), {
              fullRebuild: true,
              clearOnly: false,
            })
          }
          onRefreshQueue={() =>
            void handleQueueAction(explorerT('refreshingQueueAction'), () =>
              refreshRuntimeStatus(),
            )
          }
          onReplayJob={(jobId) =>
            void handleQueueAction(explorerT('replayingJobAction'), () =>
              backend.replayAiJob(jobId),
            )
          }
          onTestProvider={() => {
            setQueueAction(explorerT('testingProviderAction'))
            void handleProviderProbe()
          }}
          providerProbe={providerProbe}
          queueAction={queueAction}
          queueStatus={queueStatus}
          snapshotAiStatus={snapshot.aiStatus}
        />
      )}

      {surfaceIsStarred ? (
        // Starred hub: a focused Explorer mode (not a 4th nav item). Reuses
        // the contact-sheet card renderer for pages and a source-chip row.
        <div className="flex flex-col gap-3">
          <button
            type="button"
            onClick={closeStarredHub}
            data-testid="explorer-starred-back"
            className="border-border-light text-ink-muted hover:text-accent hover:border-accent self-start inline-flex items-center gap-1.5 rounded-paper border px-2.5 py-0.5 font-mono text-[10.5px] uppercase tracking-[0.08em] transition-colors"
          >
            ← {explorerT('star.hubBack')}
          </button>
          {starredHub.lastError ? (
            <StatusCallout
              tone="blocked"
              eyebrow={explorerT('star.hubEyebrow')}
              title={explorerT('star.saveError')}
              body={starredHub.lastError}
            />
          ) : null}
          <PaperStarredView
            items={starredHub.items}
            loading={starredHub.loading}
            sort={starredHub.sort}
            onSortChange={starredHub.setSort}
            onSelect={(item) => {
              if (item.entityKind === 'url') {
                // A starred page may not be in the current Browse pool, so open
                // it directly via the same visit flow the detail panel uses.
                void handleVisit(item.entityKey)
              } else {
                const next = new URLSearchParams()
                next.set('domain', item.entityKey)
                setSearchParams(next)
              }
            }}
            onToggleStar={(item) => {
              toggleStar(item.entityKind, item.entityKey)
              // Removing a star should drop it from the hub on the next read.
              starredHub.reload()
            }}
            onBrowseHistory={closeStarredHub}
            copy={buildPaperStarredViewCopy(explorerT)}
            testId="explorer-starred-view"
          />
        </div>
      ) : paperSearchSurface ? (
        // Search surface: the composer must always remain mounted so a
        // user who typed a misspelt query can keep editing. Render any
        // regex-block / backend-error / empty-result state as a
        // StatusCallout above the composer (via `belowHeroSlot`),
        // never as a full-screen takeover.
        <PaperSearchPanel
          query={queryInput}
          mode={mode}
          regexMode={regexMode}
          entries={searchEntries}
          totalResults={
            queryHasStarredFacet
              ? starredSearchEntries.length
              : (renderedTimeResults?.total ?? 0)
          }
          language={language}
          explorerT={explorerT}
          rankedEntries={rankedSearchEntries}
          aiLoading={semanticLoading}
          aiError={semanticError}
          aiNotes={semanticResults?.notes}
          pagination={smartPagination}
          relevanceScopeLine={smartScopeLine}
          smartAvailable={smartAvailable}
          onAskAssistant={(entry) => {
            // Hand the assistant only the explain prompt — no profile scope. The agent chat path
            // searches the whole archive (the backend request carries no profile filter), so
            // passing a `?profileId=` here would imply a scope the assistant cannot honor.
            const href = assistantHref(
              explorerT('assistantExplainPrompt', {
                item: entry.title,
                query: queryInput,
              }),
            )
            // assistantHref is an in-app route; navigate via react-router so we
            // hand the explain prompt to the assistant without a shell hard-reload.
            void navigate(href)
          }}
          relevanceHeaderSlot={
            smartSearchActive && smartAvailable ? (
              <SmartIndexStatusCallout
                progress={smartIndexProgress}
                readyTitle={smartIndexReadyTitle}
                hasEmbeddingProvider={Boolean(embeddingProvider)}
                language={language}
                explorerT={explorerT}
                onBuild={() =>
                  void handleIndexAction(explorerT('buildingIndexAction'), {
                    fullRebuild: false,
                    clearOnly: false,
                  })
                }
              />
            ) : null
          }
          aboveResultsCallout={
            historyBlockedByInvalidRegex
              ? {
                  tone: 'blocked',
                  eyebrow: explorerT('regexEyebrow'),
                  title: explorerT('regexInvalid'),
                  body: explorerT('regexInvalidDetail'),
                }
              : smartAboveResultsCallout
                ? smartAboveResultsCallout
                : error
                  ? {
                      tone: 'blocked',
                      eyebrow: explorerT('noMatchesEyebrow'),
                      title: explorerT('queryFailedTitle'),
                      body: error,
                    }
                  : null
          }
          onQueryChange={(next) => {
            setQueryInput(next)
            updateParam('q', next)
          }}
          onModeChange={(next) => {
            updateParam('mode', next.mode === 'keyword' ? null : next.mode)
            updateParam('regex', next.regexMode ? '1' : null)
          }}
          onSubmit={(query) => {
            setQueryInput(query)
            updateParam('q', query)
          }}
          onSelectEntry={(id) => {
            if (queryHasStarredFacet) {
              // `is:starred` rows are synthetic (sourced from `list_stars`, not
              // the history pool), so the detail panel can't bind to their id.
              // Open the starred page directly via the same visit flow the hub
              // uses instead of opening a stale/empty panel.
              const entry = starredSearchEntries.find((row) => row.id === id)
              if (entry) void handleVisit(entry.url)
              return
            }
            setSelectedId(id)
            setPaperDetailOpen(true)
          }}
          onSeeInContext={(entry, dayDate) => {
            const next = new URLSearchParams(searchParams)
            next.set('date', dayDate)
            next.set('source', 'search')
            // Drop the search-mode params so the user actually lands in
            // the Browse day-context view instead of "search results
            // within that day". Keeping `q` / `mode` / `regex` here
            // would leave the user staring at the same filtered list
            // and call the jump a no-op.
            next.delete('surface')
            next.delete('q')
            next.delete('mode')
            next.delete('regex')
            next.delete('page')
            setQueryInput('')
            setSearchParams(next)
            setSelectedId(Number(entry.id))
          }}
          entryStar={{
            isStarred: (url) => stars.isStarred('url', url),
            onToggle: (url) => toggleStar('url', url),
            starLabel: explorerT('star.starPageAria'),
            unstarLabel: explorerT('star.unstarPageAria'),
          }}
        />
      ) : historyBlockedByInvalidRegex ? (
        <StatusCallout
          tone="blocked"
          eyebrow={explorerT('regexEyebrow')}
          title={explorerT('regexInvalid')}
          body={explorerT('regexInvalidDetail')}
        />
      ) : error ? (
        <ErrorState title={explorerT('queryFailedTitle')} description={error} />
      ) : !loading &&
        results &&
        results.total === 0 &&
        hasActiveQueryOrFilter ? (
        <EmptyState
          action={
            <button
              className="btn-secondary"
              type="button"
              onClick={clearAllFilters}
            >
              {explorerT('clearFilters')}
            </button>
          }
          description={explorerT('noMatchesDescription')}
          eyebrow={explorerT('noMatchesEyebrow')}
          title={explorerT('noMatchesTitle')}
        />
      ) : view === 'time' && (loading || visibleTimeResults) ? (
        <PaperExplorerView
          entries={renderedTimeResults?.items ?? []}
          loading={loading}
          archiveBounds={archiveDensity.bounds ?? undefined}
          additionalDensity={{
            perDay: archiveDensity.perDay,
            perYear: archiveDensity.perYear,
          }}
          targetDate={searchParams.get('date')}
          targetSource={
            (searchParams.get('source') as
              | 'on-this-day'
              | 'search'
              | 'intelligence'
              | null) ?? null
          }
          targetQuery={searchParams.get('q') ?? null}
          selectedEntryId={selectedEntry?.id ?? null}
          onSelectEntry={(entry) => {
            setSelectedId(entry.id)
            setPaperDetailOpen(true)
          }}
          entryStar={{
            isStarred: (url) => stars.isStarred('url', url),
            onToggle: (url) => toggleStar('url', url),
            starLabel: explorerT('star.starPageAria'),
            unstarLabel: explorerT('star.unstarPageAria'),
          }}
          onJumpToDate={(iso) => {
            const next = new URLSearchParams(searchParams)
            next.set('date', iso)
            // Picking a date inline (calendar, year rail, day-nav arrows) is
            // an unfiltered jump — drop any sticky `source` (and the search
            // query that pinned it) so the user sees the full day, not the
            // 50-row "On this day" filter that an earlier deep link set.
            next.delete('source')
            next.delete('q')
            setSearchParams(next)
          }}
          onClearTarget={() => {
            const next = new URLSearchParams(searchParams)
            next.delete('date')
            next.delete('source')
            setSearchParams(next)
          }}
          infiniteScroll={{
            loadingMore: infiniteLoadingMore,
            canLoadMore: infiniteCanLoadMore,
            onLoadMore: infiniteLoadMore,
            loadedPageCount: infiniteLoadedPageCount,
            totalPages: visibleTimeResults?.pageCount ?? 0,
            totalRows: visibleTimeResults?.total ?? 0,
            capReached: infiniteCapReached,
            error: infiniteError,
          }}
          language={language}
          copy={buildPaperExplorerCopy(explorerT)}
          filterStripSlot={paperFilterStrip}
          resolveDayInsights={browseDayInsightsCache.resolve}
          testId="explorer-paper-view"
        />
      ) : view === 'session' ? (
        <div className="explorer-grid">
          <div className="record-list">
            <SessionGroupPanel
              dateRange={groupedDateRange}
              explorerT={explorerT}
              intelligenceT={intelligenceT}
              language={language}
              onSelectVisit={(visit) =>
                setSelectedGroupedVisitState({
                  key: groupedSelectionKey,
                  visit,
                })
              }
              profileId={profileId}
            />
          </div>
          <ExplorerDetailPanel
            commonT={commonT}
            explorerT={explorerT}
            handleVisit={handleVisit}
            intelligenceT={intelligenceT}
            language={language}
            selectedVisit={selectedGroupedVisit}
          />
        </div>
      ) : view === 'trail' ? (
        <div className="explorer-grid">
          <div className="record-list">
            <TrailGroupPanel
              dateRange={groupedDateRange}
              explorerT={explorerT}
              intelligenceT={intelligenceT}
              language={language}
              onSelectVisit={(visit) =>
                setSelectedGroupedVisitState({
                  key: groupedSelectionKey,
                  visit,
                })
              }
              profileId={profileId}
            />
          </div>
          <ExplorerDetailPanel
            commonT={commonT}
            explorerT={explorerT}
            handleVisit={handleVisit}
            intelligenceT={intelligenceT}
            language={language}
            selectedVisit={selectedGroupedVisit}
          />
        </div>
      ) : null}

      {paperDetailOpen ? (
        <PaperDetailPanelMount
          selectedEntry={selectedEntry}
          annotations={annotations}
          explorerT={explorerT}
          enrichment={visitEnrichment}
          stars={{
            isStarred: (url) => stars.isStarred('url', url),
            onToggleStar: (url) => toggleStar('url', url),
          }}
          onClose={() => setPaperDetailOpen(false)}
          onOpen={(url) => void handleVisit(url)}
          onOpenDomain={(domain) => {
            // "All of {domain}" → drop into a domain-only Browse view.
            // The user is asking to see EVERYTHING from this domain, so
            // strip every contextual filter that the previous surface
            // accumulated (search surface, search query, semantic mode,
            // regex, date window, start/end pin, profile / browser
            // scope, paginator cursors) and write only the domain. This
            // matches what `clearAllFilters` would have written, plus
            // the new `domain=` chip.
            const next = new URLSearchParams()
            next.set('domain', domain)
            setSearchParams(next)
          }}
        />
      ) : null}
    </section>
  )
}
