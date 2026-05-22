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

import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
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
import { historyFaviconLookupKey, historyOgImageLookupKey } from './helpers'
import { useProfileScope } from '../../lib/profile-scope-context'
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

  const {
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
  const { faviconCache } = useExplorerFavicons({
    cacheToken: refreshKey,
    loading,
    results: visibleTimeResults,
  })
  const { ogImageCache } = useExplorerOgImages({
    cacheToken: refreshKey,
    loading,
    results: visibleTimeResults,
  })
  const archiveDensity = useExplorerArchiveDensity({
    archiveReady,
    profileId,
    refreshKey,
  })
  // Infinite scroll: accumulate pages 2..N when there's no date filter and
  // we're in the default paper Browse surface. A date-narrowed view, search
  // surface, or grouped (session/trail) view bypasses accumulation so the
  // existing pagination footer handles those edge cases honestly. The
  // `surface=search` check resolves below at line ~340, so we replicate the
  // same predicate here without forward-referencing.
  const dateFiltered = Boolean(searchParams.get('date'))
  const surfaceIsSearch = searchParams.get('surface') === 'search'
  const infiniteDisabled =
    !archiveReady ||
    view !== 'time' ||
    surfaceIsSearch ||
    dateFiltered ||
    historyBlockedByInvalidRegex
  const {
    extraItems: infiniteExtraItems,
    loadedPageCount: infiniteLoadedPageCount,
    loadingMore: infiniteLoadingMore,
    canLoadMore: infiniteCanLoadMore,
    loadMore: infiniteLoadMore,
  } = useExplorerInfinitePages({
    query: currentQuery,
    headResults: visibleTimeResults,
    disabled: infiniteDisabled,
    cacheToken: refreshKey,
  })
  const renderedTimeResults = useMemo(() => {
    if (!visibleTimeResults) {
      return null
    }
    // When infinite scroll is active, prepend the head page items + every
    // accumulated extra page so the contact sheet renders one continuous
    // timeline. Date-filtered / search-surface views keep the head-only
    // shape so their pagination footer still drives the visible window.
    const combinedItems = infiniteDisabled
      ? visibleTimeResults.items
      : [...visibleTimeResults.items, ...infiniteExtraItems]
    return {
      ...visibleTimeResults,
      items: combinedItems.map((item) => ({
        ...item,
        favicon:
          item.favicon ??
          faviconCache.get(
            historyFaviconLookupKey(item.profileId, item.url, item.visitTime),
          ) ??
          null,
        ogImage:
          item.ogImage ??
          ogImageCache.get(historyOgImageLookupKey(item.url)) ??
          null,
      })),
    }
  }, [
    faviconCache,
    infiniteDisabled,
    infiniteExtraItems,
    ogImageCache,
    visibleTimeResults,
  ])
  const selectedEntry =
    (!loading ? visibleTimeResults : null)?.items.find(
      (item) => item.id === selectedId,
    ) ??
    (!loading ? visibleTimeResults : null)?.items[0] ??
    null
  const selectedGroupedVisit =
    selectedGroupedVisitState?.key === groupedSelectionKey
      ? selectedGroupedVisitState.visit
      : null
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

  const paperSearchSurface = searchParams.get('surface') === 'search'

  // The full-page EmptyState below should only fire when the zero-result
  // came from an active filter — a palette / hash search query, a domain
  // facet, a browserKind filter, etc. For pure date browsing (Calendar,
  // year rail, day-nav arrows) we want the contact-sheet shell to stay
  // visible so users can keep navigating; the inner "Nothing here yet"
  // copy already handles the "this day has no visits" case without
  // hijacking the whole route.
  const hasActiveQueryOrFilter =
    paperSearchSurface ||
    queryInput.trim() !== '' ||
    (searchParams.get('q') ?? '').trim() !== '' ||
    Boolean(searchParams.get('domain')) ||
    Boolean(searchParams.get('browserKind'))

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

      {historyBlockedByInvalidRegex ? (
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
        paperSearchSurface ? (
          <PaperSearchPanel
            query={queryInput}
            mode={mode}
            regexMode={regexMode}
            entries={renderedTimeResults?.items ?? []}
            totalResults={renderedTimeResults?.total ?? 0}
            language={language}
            explorerT={explorerT}
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
              // Drop the search-mode params so the user actually lands in the
              // Browse day-context view instead of "search results within
              // that day". Keeping `q` / `mode` / `regex` here would leave
              // the user staring at the same filtered list and call the
              // jump a no-op.
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
        ) : (
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
                  }
            }
            language={language}
            copy={buildPaperExplorerCopy(explorerT)}
            testId="explorer-paper-view"
          />
        )
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
        />
      ) : null}
    </section>
  )
}
