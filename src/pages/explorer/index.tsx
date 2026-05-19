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

import { useEffect, useMemo, useState } from 'react'
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
import { historyFaviconLookupKey } from './helpers'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { useExplorerData } from './hooks/use-explorer-data'
import { useExplorerFavicons } from './hooks/use-explorer-favicons'
import { useExplorerUrlState } from './hooks/use-explorer-url-state'
import { ExplorerDetailPanel } from './panels/detail-panel'
import { ExplorerResultsPanel } from './panels/results-panel'
import { ExplorerRuntimePanel } from './panels/runtime-panel'
import { ExplorerSemanticPanel } from './panels/semantic-panel'
import { SessionGroupPanel } from './panels/session-group'
import { TrailGroupPanel } from './panels/trail-group'
import { ExplorerQueryFiltersPanel } from './query-filters-panel'
import { ExplorerTimelineBar } from './timeline-bar'
import {
  buildPaperDetailPanelCopy,
  buildPaperExplorerCopy,
  buildPaperSearchViewCopy,
} from './paper-explorer-copy'
import { PaperExplorerView } from './paper-view'
import { PaperDetailPanel, PaperSearchView } from '@/components/explorer-paper'
import {
  buildPaperSearchDayGroups,
  explorerStateFromPaperSearchMode,
  paperSearchModeFromExplorerState,
} from './paper-search-helpers'
import { getDomainAbbr, getDomainColor } from './paper/domain-color'
import { useLocalAnnotations } from './use-local-annotations'
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
    activeDateShortcut,
    activeFilters,
    applyDateShortcut,
    browserKinds,
    buildRecentSearchLabel,
    clearAllFilters,
    clearDateRange,
    currentQuery,
    end,
    explicitPage,
    explicitProfileId,
    groupedDateRange,
    handleFirstHistoryPage,
    handleHistoryPageJump,
    handleLastHistoryPage,
    handleNextHistoryPage,
    handleNextSemanticPage,
    handlePreviousHistoryPage,
    handlePreviousSemanticPage,
    historyPageInput,
    mode,
    pageSize,
    persistRecentSearch,
    profileId,
    queryInput,
    recentSearches,
    regexMode,
    regexValid,
    searchParams,
    semanticQuery,
    semanticTrail,
    setHistoryPageInput,
    setHistoryPageSize,
    setQueryInput,
    setRecentSearches,
    setSearchParams,
    setView,
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
  const annotations = useLocalAnnotations()
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
    actionError,
    cachedHistoryResults,
    copyFeedback,
    exportResult,
    handleCopyExportPath,
    handleExport,
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
  const renderedTimeResults = useMemo(() => {
    if (!visibleTimeResults) {
      return null
    }

    return {
      ...visibleTimeResults,
      items: visibleTimeResults.items.map((item) => ({
        ...item,
        favicon:
          item.favicon ??
          faviconCache.get(
            historyFaviconLookupKey(item.profileId, item.url, item.visitTime),
          ) ??
          null,
      })),
    }
  }, [faviconCache, visibleTimeResults])
  const historyPage = loading
    ? (explicitPage ?? visibleTimeResults?.page ?? 1)
    : (visibleTimeResults?.page ?? explicitPage ?? 1)
  const historyPageCount = visibleTimeResults?.pageCount ?? 1
  const activeScopeLabel = activeProfileId
    ? profileIdLabel(activeProfileId)
    : null
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

  useEffect(() => {
    setHistoryPageInput(String(historyPage))
  }, [historyPage, setHistoryPageInput])

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
      <ExplorerTimelineBar
        activeShortcutKey={activeDateShortcut()}
        explorerT={explorerT}
        onApplyDateShortcut={applyDateShortcut}
        onClearDateRange={clearDateRange}
        summary={
          visibleTimeResults
            ? {
                currentPage: historyPage,
                loaded: visibleTimeResults.items.length,
                pageCount: historyPageCount,
                total: visibleTimeResults.total,
              }
            : null
        }
        end={end}
        start={start}
      />

      <ExplorerQueryFiltersPanel
        activeFilters={activeFilters}
        activeScopeLabel={activeScopeLabel}
        browserKinds={browserKinds}
        buildRecentSearchLabel={buildRecentSearchLabel}
        clearAllFilters={clearAllFilters}
        explicitProfileId={explicitProfileId}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        mode={mode}
        optionalAiAvailability={optionalAiAvailability}
        profileId={profileId}
        queryInput={queryInput}
        recentSearches={recentSearches}
        regexMode={regexMode}
        regexValid={regexValid}
        searchParams={searchParams}
        selectedProfileIds={snapshot.config.selectedProfileIds}
        setQueryInput={setQueryInput}
        setSearchParams={setSearchParams}
        setView={setView}
        updateParam={updateParam}
        view={view}
        visibleRecordCount={visibleTimeResults?.total ?? null}
      />

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
      ) : !loading && results && results.total === 0 ? (
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
        searchParams.get('layout') === 'paper' &&
        searchParams.get('surface') === 'search' ? (
          <PaperSearchView
            query={queryInput}
            mode={paperSearchModeFromExplorerState(mode, regexMode)}
            activeFilters={[]}
            groups={buildPaperSearchDayGroups(
              renderedTimeResults?.items ?? [],
              { language },
            )}
            totalResults={renderedTimeResults?.total ?? 0}
            resolveDomainColor={getDomainColor}
            resolveDomainAbbr={getDomainAbbr}
            onQueryChange={(next) => {
              setQueryInput(next)
              updateParam('q', next)
            }}
            onModeChange={(nextMode) => {
              const next = explorerStateFromPaperSearchMode(nextMode)
              updateParam('mode', next.mode === 'keyword' ? null : next.mode)
              updateParam('regex', next.regexMode ? '1' : null)
            }}
            onRemoveFilter={() => {
              /* paper hero filters are deferred to the next pass */
            }}
            onSubmit={(query) => {
              setQueryInput(query)
              updateParam('q', query)
            }}
            onSelectEntry={(entry) => {
              setSelectedId(Number(entry.id))
              setPaperDetailOpen(true)
            }}
            onSeeInContext={(entry, dayDate) => {
              const next = new URLSearchParams(searchParams)
              next.set('date', dayDate)
              next.set('source', 'search')
              next.delete('surface')
              setSearchParams(next)
              setSelectedId(Number(entry.id))
            }}
            copy={buildPaperSearchViewCopy(explorerT)}
            testId="explorer-paper-search-view"
          />
        ) : searchParams.get('layout') === 'paper' ? (
          <PaperExplorerView
            entries={renderedTimeResults?.items ?? []}
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
              setSearchParams(next)
            }}
            onClearTarget={() => {
              const next = new URLSearchParams(searchParams)
              next.delete('date')
              next.delete('source')
              setSearchParams(next)
            }}
            language={language}
            copy={buildPaperExplorerCopy(explorerT)}
            testId="explorer-paper-view"
          />
        ) : (
          <ExplorerResultsPanel
            actionError={actionError}
            commonT={commonT}
            copyFeedback={copyFeedback}
            explorerT={explorerT}
            exportResult={exportResult}
            handleCopyExportPath={handleCopyExportPath}
            handleExport={handleExport}
            handleHistoryPageJump={(historyPageCount) =>
              handleHistoryPageJump(historyPage, historyPageCount)
            }
            handleFirstHistoryPage={() => handleFirstHistoryPage(historyPage)}
            handleLastHistoryPage={(historyPageCount) =>
              handleLastHistoryPage(historyPage, historyPageCount)
            }
            handleNextHistoryPage={() => handleNextHistoryPage(historyPage)}
            handleOpenExportPath={async (path) => {
              await backend.openPathInFileManager(path)
            }}
            handlePreviousHistoryPage={() =>
              handlePreviousHistoryPage(historyPage)
            }
            handleVisit={handleVisit}
            historyBlockedByInvalidRegex={historyBlockedByInvalidRegex}
            historyPage={historyPage}
            historyPageCount={historyPageCount}
            historyPageInput={historyPageInput}
            historyPageSize={pageSize}
            intelligenceT={intelligenceT}
            language={language}
            loading={loading}
            onHistoryPageInputChange={setHistoryPageInput}
            onHistoryPageSizeChange={setHistoryPageSize}
            onSelectHistory={setSelectedId}
            results={renderedTimeResults}
            selectedEntry={selectedEntry}
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

      {searchParams.get('layout') === 'paper' &&
      paperDetailOpen &&
      selectedEntry ? (
        <PaperDetailPanel
          entry={{
            id: selectedEntry.id,
            title: selectedEntry.title ?? selectedEntry.url,
            url: selectedEntry.url,
            domain: selectedEntry.domain,
            firstVisitAt: selectedEntry.visitedAt,
            lastVisitAt: selectedEntry.visitedAt,
            source: profileIdLabel(selectedEntry.profileId),
          }}
          notes={annotations.notesFor(selectedEntry.url)}
          tags={annotations.tagsFor(selectedEntry.url)}
          onClose={() => setPaperDetailOpen(false)}
          onOpen={(entry) => void handleVisit(String(entry.id))}
          onCopyUrl={(entry) => {
            void globalThis.navigator?.clipboard?.writeText(entry.url)
          }}
          onUpdateNotes={(next) =>
            annotations.updateNotes(selectedEntry.url, next)
          }
          onUpdateTags={(next) =>
            annotations.updateTags(selectedEntry.url, next)
          }
          copy={buildPaperDetailPanelCopy(explorerT)}
          testId="explorer-paper-detail-panel"
        />
      ) : null}
    </section>
  )
}
