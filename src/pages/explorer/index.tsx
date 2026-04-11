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

import { useEffect, useMemo } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import {
  SkeletonExplorer,
  SkeletonExplorerResults,
} from '../../components/primitives/skeleton'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import { aiStatusMeta, selectedAiProvider } from '../../lib/intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { browserLabel, dateShortcutWindows } from './helpers'
import { useExplorerData } from './hooks/use-explorer-data'
import { useExplorerUrlState } from './hooks/use-explorer-url-state'
import { ExplorerResultsPanel } from './panels/results-panel'
import { ExplorerRuntimePanel } from './panels/runtime-panel'
import { ExplorerSemanticPanel } from './panels/semantic-panel'

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
    handleFirstHistoryPage,
    handleHistoryPageJump,
    handleLastHistoryPage,
    handleNextHistoryPage,
    handleNextSemanticPage,
    handlePreviousHistoryPage,
    handlePreviousSemanticPage,
    historyPageInput,
    historyQuerySignature,
    historyScrollPositionsRef,
    mode,
    pendingHistoryScrollKeyRef,
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
    setQueryInput,
    setRecentSearches,
    setSearchParams,
    start,
    updateParam,
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
  const embeddingProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'embedding')
    : null
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
    copiedExportPath,
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
    queueStatus,
    selectedId,
    semanticState,
    setQueueAction,
    setSelectedId,
  } = useExplorerData({
    archiveReady,
    currentQuery,
    embeddingProviderId: embeddingProvider?.id ?? null,
    end,
    historyBlockedByInvalidRegex,
    labels,
    mode,
    persistRecentSearch,
    refreshAppData,
    refreshKey,
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
      : null
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
    queryState.requestKey !== requestKey
  const historyPage = results?.page ?? explicitPage ?? 1
  const historyPageCount = results?.pageCount ?? 1
  const selectedEntry =
    results?.items.find((item) => item.id === selectedId) ??
    results?.items[0] ??
    null

  useEffect(() => {
    setHistoryPageInput(String(historyPage))
  }, [historyPage, setHistoryPageInput])

  useEffect(() => {
    const pendingKey = pendingHistoryScrollKeyRef.current
    if (!pendingKey) return
    if (pendingKey !== `${historyQuerySignature}|${historyPage}`) return
    const scrollContainer = document.querySelector('.workspace-scroll')
    if (!(scrollContainer instanceof HTMLElement)) {
      pendingHistoryScrollKeyRef.current = null
      return
    }
    const nextScrollTop = historyScrollPositionsRef.current[pendingKey]
    if (typeof nextScrollTop === 'number') {
      scrollContainer.scrollTop = nextScrollTop
    }
    pendingHistoryScrollKeyRef.current = null
  }, [
    historyPage,
    historyQuerySignature,
    historyScrollPositionsRef,
    pendingHistoryScrollKeyRef,
  ])

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
      <div className="timeline-bar">
        <div className="timeline-controls">
          {dateShortcutWindows.map((entry) => (
            <button
              key={entry.key}
              className={`tl-btn ${
                activeDateShortcut() === entry.key ? 'active' : ''
              }`}
              type="button"
              onClick={() => applyDateShortcut(entry.days)}
            >
              {explorerT(entry.labelKey)}
            </button>
          ))}
        </div>
        <div className="timeline-track">
          <span className="timeline-label">
            {results
              ? explorerT('historyPageSummary', {
                  page: historyPage,
                  loaded: results.items.length,
                  total: results.total,
                })
              : explorerT('waitingForQuery')}
          </span>
          <span className="timeline-label">
            {start || end
              ? `${start ?? '…'} → ${end ?? '…'}`
              : explorerT('allRecordedTime')}
          </span>
          {(start || end) && (
            <button className="tl-today" type="button" onClick={clearDateRange}>
              {explorerT('clearRange')}
            </button>
          )}
        </div>
      </div>

      {activeFilters.length > 0 && (
        <div className="filter-bar">
          <div className="filter-tags">
            {activeFilters.map((filter) => (
              <div key={`${filter.id}:${filter.value}`} className="filter-tag">
                <span>
                  {filter.label}: {filter.value}
                </span>
                <button
                  aria-label={explorerT('removeFilter', {
                    label: filter.label,
                    value: filter.value,
                  })}
                  className="filter-remove"
                  type="button"
                  onClick={() => updateParam(filter.id, null)}
                >
                  <span aria-hidden>×</span>
                  <span className="sr-only">
                    {explorerT('removeFilterShort')}
                  </span>
                </button>
              </div>
            ))}
          </div>
          <div className="filter-actions">
            <button
              className="filter-btn"
              type="button"
              onClick={clearAllFilters}
            >
              {explorerT('clearAllFilters')}
            </button>
          </div>
        </div>
      )}

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{explorerT('queryFiltersTitle')}</span>
          <span className="panel-action">
            {results
              ? explorerT('visibleRecords', { count: results.total })
              : explorerT('waitingForQuery')}
          </span>
        </div>
        <div className="panel-body">
          <div
            className="segmented-row"
            style={{ marginBottom: 'var(--space-4)' }}
          >
            {(['keyword', 'semantic', 'hybrid'] as const).map((option) => (
              <button
                key={option}
                className={`chip-button ${
                  mode === option ? 'chip-button--active' : ''
                }`}
                type="button"
                onClick={() =>
                  updateParam('mode', option === 'keyword' ? null : option)
                }
              >
                {option === 'keyword'
                  ? explorerT('modeKeyword')
                  : option === 'semantic'
                    ? explorerT('modeSemantic')
                    : explorerT('modeHybrid')}
              </button>
            ))}
          </div>
          <div className="explorer-filters">
            <div
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">
                {explorerT('filterKeyword')}
                {regexMode ? <span className="regex-badge">[.*]</span> : null}
              </span>
              <div className="regex-input-row">
                <input
                  aria-label={explorerT('filterKeywordAria')}
                  className={regexMode && !regexValid ? 'input-invalid' : ''}
                  type="search"
                  value={queryInput}
                  onChange={(event) => setQueryInput(event.target.value)}
                />
                <button
                  aria-label={explorerT('toggleRegex')}
                  aria-pressed={regexMode}
                  className={`regex-toggle ${
                    regexMode ? 'regex-toggle--active' : ''
                  }`}
                  title={explorerT('toggleRegex')}
                  type="button"
                  onClick={() => updateParam('regex', regexMode ? null : '1')}
                >
                  .*
                </button>
              </div>
              {regexMode && queryInput.trim() ? (
                <span
                  className={regexValid ? 'regex-valid' : 'regex-error'}
                  role={regexValid ? undefined : 'alert'}
                >
                  {regexValid
                    ? explorerT('regexValid')
                    : explorerT('regexInvalid')}
                </span>
              ) : null}
            </div>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterDomain')}</span>
              <input
                aria-label={explorerT('filterDomain')}
                type="search"
                value={searchParams.get('domain') ?? ''}
                onChange={(event) =>
                  updateParam('domain', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterProfile')}</span>
              <select
                aria-label={explorerT('filterProfileAria')}
                value={profileId ?? ''}
                onChange={(event) =>
                  updateParam('profileId', event.target.value || null)
                }
              >
                <option value="">{explorerT('allProfiles')}</option>
                {snapshot.config.selectedProfileIds.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </select>
              {!explicitProfileId && activeProfileId ? (
                <span className="mono-support">
                  {explorerT('scopeInherited', {
                    profile: profileIdLabel(activeProfileId),
                  })}
                </span>
              ) : null}
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterBrowser')}</span>
              <select
                aria-label={explorerT('filterBrowser')}
                value={searchParams.get('browserKind') ?? ''}
                onChange={(event) =>
                  updateParam('browserKind', event.target.value || null)
                }
              >
                <option value="">{explorerT('allBrowsers')}</option>
                {browserKinds.map((kind) => (
                  <option key={kind} value={kind}>
                    {browserLabel(kind)}
                  </option>
                ))}
              </select>
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterStart')}</span>
              <input
                aria-label={explorerT('filterStart')}
                type="date"
                value={start ?? ''}
                onChange={(event) =>
                  updateParam('start', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterEnd')}</span>
              <input
                aria-label={explorerT('filterEnd')}
                type="date"
                value={end ?? ''}
                onChange={(event) =>
                  updateParam('end', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterSort')}</span>
              <select
                aria-label={explorerT('filterSort')}
                value={searchParams.get('sort') ?? 'newest'}
                onChange={(event) => updateParam('sort', event.target.value)}
              >
                <option value="newest">{explorerT('sortNewest')}</option>
                <option value="oldest">{explorerT('sortOldest')}</option>
              </select>
            </label>
          </div>
        </div>
        <div
          className="panel-body"
          style={{
            borderTop: '1px solid var(--border)',
            paddingTop: 'var(--space-2)',
          }}
        >
          <div className="recent-search-bar">
            {recentSearches.length > 0 ? (
              recentSearches.map((entry) => (
                <button
                  key={JSON.stringify(entry.params)}
                  className="chip-button"
                  type="button"
                  onClick={() =>
                    setSearchParams(
                      new URLSearchParams(
                        Object.entries(entry.params).flatMap(([key, value]) =>
                          value ? [[key, value]] : [],
                        ),
                      ),
                    )
                  }
                >
                  {buildRecentSearchLabel(entry.params) || entry.label}
                </button>
              ))
            ) : (
              <span className="mono-support">
                {explorerT('recentFiltersEmpty')}
              </span>
            )}
          </div>
        </div>
      </div>

      {aiMeta && mode !== 'keyword' && (
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
              backend.loadAiQueueStatus(),
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

      {mode !== 'keyword' && (
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

      {loading ? (
        <SkeletonExplorerResults label={t('common.loadingExplorerResults')} />
      ) : historyBlockedByInvalidRegex ? (
        <StatusCallout
          tone="blocked"
          eyebrow={explorerT('regexEyebrow')}
          title={explorerT('regexInvalid')}
          body={explorerT('regexInvalidDetail')}
        />
      ) : error ? (
        <ErrorState title={explorerT('queryFailedTitle')} description={error} />
      ) : results && results.total === 0 ? (
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
      ) : results ? (
        <ExplorerResultsPanel
          actionError={actionError}
          commonT={commonT}
          copiedExportPath={copiedExportPath}
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
          language={language}
          onHistoryPageInputChange={setHistoryPageInput}
          onSelectHistory={setSelectedId}
          results={results}
          selectedEntry={selectedEntry}
        />
      ) : null}
    </section>
  )
}
