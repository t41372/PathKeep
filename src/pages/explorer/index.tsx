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

import { useCallback, useMemo, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
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
import { useExplorerArchiveDensity } from './hooks/use-explorer-archive-density'
import { useExplorerData } from './hooks/use-explorer-data'
import { useExplorerFavicons } from './hooks/use-explorer-favicons'
import { useExplorerInfinitePages } from './hooks/use-explorer-infinite-pages'
import { useExplorerOgImages } from './hooks/use-explorer-og-images'
import { useExplorerUrlState } from './hooks/use-explorer-url-state'
import { ExplorerDetailPanel } from './panels/detail-panel'
import { ExplorerRuntimePanel } from './panels/runtime-panel'
import { ExplorerSemanticPanel } from './panels/semantic-panel'
import { SessionGroupPanel } from './panels/session-group'
import { TrailGroupPanel } from './panels/trail-group'
import { buildPaperExplorerCopy } from './paper-explorer-copy'
import {
  PaperFilterStrip,
  type PaperFilterStripFormState,
} from '../../components/explorer-paper'
import { PaperExplorerView } from './paper-view'
import { PaperSearchPanel } from './paper-search-panel'
import { PaperDetailPanelMount } from './paper-detail-panel-mount'
import { useLocalAnnotations } from './use-local-annotations'
import { useDesktopAnnotations } from './use-desktop-annotations'
import { hasDesktopCommandTransport } from '../../lib/runtime'
import type { ExplorerVisitSelection } from './types'

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

  const {
    activeFilters,
    browserKinds,
    clearAllFilters,
    currentQuery,
    end,
    explicitPage,
    groupedDateRange,
    handleNextHistoryPage,
    handlePreviousHistoryPage,
    handleNextSemanticPage,
    handlePreviousSemanticPage,
    mode,
    pageSize,
    persistRecentSearch,
    profileId,
    queryInput,
    regexMode,
    regexValid,
    searchParams,
    semanticQuery,
    semanticTrail,
    setHistoryPageSize,
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
  const { ogImageCache } = useExplorerOgImages({
    cacheToken: refreshKey,
    loading,
    results: combinedTimeResults,
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
  const paperFilterStrip = useMemo(
    () => (
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
    ),
    [
      browserOptions,
      clearAllFilters,
      filterStripChips,
      filterStripCopy,
      filterStripFormState,
      handleFilterStripApply,
      handleFilterStripRemove,
      profileOptions,
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
  const selectedEntry =
    selectedEntryPool?.items.find((item) => item.id === selectedId) ?? null
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
  const optionalAiReason = optionalAiAvailability.reason
  const optionalAiFixableReason =
    optionalAiReason === 'ai-disabled' ||
    optionalAiReason === 'no-embedding-provider' ||
    optionalAiReason === 'embedding-provider-error'
  const optionalAiUnavailableTitle =
    optionalAiReason === 'ai-disabled'
      ? explorerT('optionalAiDisabledTitle')
      : optionalAiReason === 'no-embedding-provider'
        ? explorerT('optionalAiNoProviderTitle')
        : optionalAiReason === 'embedding-provider-error'
          ? explorerT('optionalAiProviderErrorTitle')
          : explorerT('optionalAiDeferredTitle')
  const optionalAiUnavailableBody =
    optionalAiReason === 'ai-disabled'
      ? explorerT('optionalAiDisabledBody')
      : optionalAiReason === 'no-embedding-provider'
        ? explorerT('optionalAiNoProviderBody')
        : optionalAiReason === 'embedding-provider-error'
          ? explorerT('optionalAiProviderErrorBody')
          : explorerT('optionalAiDeferredBody')

  const paperSearchSurface =
    routeIsSearchPath || searchParams.get('surface') === 'search'

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

  return (
    <section className="page-shell explorer-page" data-testid="explorer-page">
      {optionalAiFixableReason ? (
        <StatusCallout
          tone={
            optionalAiReason === 'embedding-provider-error' ? 'blocked' : 'info'
          }
          eyebrow={explorerT('semanticStatusEyebrow')}
          title={optionalAiUnavailableTitle}
          body={optionalAiUnavailableBody}
          actions={
            <Link className="btn-secondary" to="/settings">
              {explorerT('optionalAiOpenSettings')}
            </Link>
          }
        />
      ) : null}

      {mode !== 'keyword' &&
      !optionalAiAvailability.available &&
      !optionalAiFixableReason ? (
        <StatusCallout
          tone="info"
          eyebrow={explorerT('semanticStatusEyebrow')}
          title={optionalAiUnavailableTitle}
          body={optionalAiUnavailableBody}
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

      {mode !== 'keyword' && optionalAiAvailability.available && (
        <ExplorerSemanticPanel
          explorerT={explorerT}
          intelligenceT={intelligenceT}
          language={language}
          mode={mode}
          onNextPage={handleNextSemanticPage}
          onPreviousPage={handlePreviousSemanticPage}
          onSelectHistory={setSelectedId}
          semanticError={semanticError}
          semanticLoading={semanticLoading}
          semanticQuery={semanticQuery}
          semanticResults={semanticResults}
          semanticTrailLength={semanticTrail.length}
        />
      )}

      {paperSearchSurface ? (
        // Search surface: the composer must always remain mounted so a
        // user who typed a misspelt query can keep editing. Render any
        // regex-block / backend-error / empty-result state as a
        // StatusCallout above the composer (via `belowHeroSlot`),
        // never as a full-screen takeover.
        <PaperSearchPanel
          query={queryInput}
          mode={mode}
          regexMode={regexMode}
          entries={renderedTimeResults?.items ?? []}
          totalResults={renderedTimeResults?.total ?? 0}
          language={language}
          explorerT={explorerT}
          aboveResultsCallout={
            historyBlockedByInvalidRegex
              ? {
                  tone: 'blocked',
                  eyebrow: explorerT('regexEyebrow'),
                  title: explorerT('regexInvalid'),
                  body: explorerT('regexInvalidDetail'),
                }
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
      ) : view === 'time' && (loading || visibleTimeResults || results) ? (
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
          pagination={
            infiniteDisabled
              ? {
                  page: explicitPage,
                  pageSize,
                  total: visibleTimeResults?.total ?? 0,
                  pageCount: visibleTimeResults?.pageCount ?? 0,
                  hasPrevious: Boolean(visibleTimeResults?.hasPrevious),
                  hasNext: Boolean(visibleTimeResults?.hasNext),
                  onPrevious: () =>
                    handlePreviousHistoryPage(
                      visibleTimeResults?.page ?? explicitPage ?? 1,
                    ),
                  onNext: () =>
                    handleNextHistoryPage(
                      visibleTimeResults?.page ?? explicitPage ?? 1,
                    ),
                  onChangePageSize: setHistoryPageSize,
                }
              : undefined
          }
          infiniteScroll={
            infiniteDisabled
              ? undefined
              : {
                  loadingMore: infiniteLoadingMore,
                  canLoadMore: infiniteCanLoadMore,
                  onLoadMore: infiniteLoadMore,
                  loadedPageCount: infiniteLoadedPageCount,
                  totalPages: visibleTimeResults?.pageCount ?? 0,
                  totalRows: visibleTimeResults?.total ?? 0,
                  capReached: infiniteCapReached,
                  error: infiniteError,
                }
          }
          language={language}
          copy={buildPaperExplorerCopy(explorerT)}
          filterStripSlot={paperFilterStrip}
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
