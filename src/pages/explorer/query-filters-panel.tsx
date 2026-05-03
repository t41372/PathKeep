/**
 * @file query-filters-panel.tsx
 * @description Render-only query, filter, and recent-search shell for the Explorer route.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Render the primary keyword search affordance with the regex toggle and
 *   mode chips inline so search reads as the route's main job-to-be-done.
 * - Render recent searches directly under the search input so re-running a
 *   prior recall stays one click away.
 * - Render the view-by toggle and the secondary filter inputs as a distinct,
 *   visually quieter cluster so they do not compete with search.
 * - Render the active-filter chip bar above the search hero so what is
 *   currently filtering the result set stays visible at all times.
 *
 * ## Not responsible for
 * - Owning URL state or debouncing query changes.
 * - Fetching Explorer results or computing optional-AI availability.
 * - Rendering results, grouped views, or AI runtime state.
 *
 * ## Dependencies
 * - Depends on Explorer and Intelligence translator copy, selected profile ids,
 *   browser labels, recent-search label helpers, and the optional-AI
 *   availability descriptor from the route owner.
 *
 * ## Performance notes
 * - Render-only owner so Explorer can re-use the same filter shell while
 *   result panels stage behind it.
 */

import { browserLabel } from './helpers'
import { profileIdLabel } from '../../lib/profile-scope-context'
import {
  evaluateOptionalAiAvailability,
  optionalAiUnavailableI18nKey,
  type OptionalAiAvailability,
} from '../../lib/optional-ai-availability'
import { optionalAiFeaturesAvailable } from '../../lib/release-capabilities'
import type {
  ExplorerMode,
  ExplorerViewMode,
  RecentSearchEntry,
  Translator,
} from './types'

function explorerProfileLabel(profileId: string) {
  const browserKind = profileId.split(':')[0]
  const profileLabel = profileIdLabel(profileId)
  return `${browserLabel(browserKind)} · ${profileLabel}`
}

interface ExplorerQueryFiltersPanelProps {
  activeFilters: Array<{
    id: string
    label: string
    value: string
  }>
  activeScopeLabel: string | null
  browserKinds: string[]
  buildRecentSearchLabel: (params: RecentSearchEntry['params']) => string
  clearAllFilters: () => void
  explicitProfileId: string | null
  explorerT: Translator
  intelligenceT: Translator
  mode: ExplorerMode
  optionalAiAvailability?: OptionalAiAvailability
  profileId: string | null
  queryInput: string
  recentSearches: RecentSearchEntry[]
  regexMode: boolean
  regexValid: boolean
  searchParams: URLSearchParams
  selectedProfileIds: string[]
  setQueryInput: (value: string) => void
  setSearchParams: (params: URLSearchParams) => void
  setView: (view: ExplorerViewMode) => void
  updateParam: (id: string, value: string | null) => void
  view: ExplorerViewMode
  visibleRecordCount: number | null
}

/**
 * Keeps Explorer query and filter chrome in one render-only owner.
 *
 * The route still owns debouncing, URL synchronization, and persistence; this
 * component only renders the controls so the route shell stays readable.
 */
export function ExplorerQueryFiltersPanel({
  activeFilters,
  activeScopeLabel,
  browserKinds,
  buildRecentSearchLabel,
  clearAllFilters,
  explicitProfileId,
  explorerT,
  intelligenceT,
  mode,
  optionalAiAvailability,
  profileId,
  queryInput,
  recentSearches,
  regexMode,
  regexValid,
  searchParams,
  selectedProfileIds,
  setQueryInput,
  setSearchParams,
  setView,
  updateParam,
  view,
  visibleRecordCount,
}: ExplorerQueryFiltersPanelProps) {
  // The route owner can pass a richer availability object that explains the
  // specific reason optional AI is closed. When it is absent we still honor
  // the release flag so the panel stays safe in isolation tests and previews.
  const aiAvailability =
    optionalAiAvailability ??
    evaluateOptionalAiAvailability({
      releaseEnabled: optionalAiFeaturesAvailable,
      embeddingProviderId: 'panel-default',
      aiStatusState: null,
    })
  const aiUnavailableTitle = aiAvailability.reason
    ? explorerT(optionalAiUnavailableI18nKey(aiAvailability.reason))
    : undefined

  return (
    <>
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

      <div className="panel explorer-search-hero">
        <div className="panel-header">
          <span className="panel-title">{explorerT('queryFiltersTitle')}</span>
          <span className="panel-action">
            {visibleRecordCount !== null
              ? explorerT('visibleRecords', { count: visibleRecordCount })
              : explorerT('waitingForQuery')}
          </span>
        </div>
        <div className="panel-body explorer-search-hero__body">
          <div className="explorer-search-hero__row">
            <div className="explorer-search-hero__input-wrap">
              <span className="mono-kicker explorer-search-hero__kicker">
                {explorerT('searchHeroEyebrow')}
                {regexMode ? <span className="regex-badge">[.*]</span> : null}
              </span>
              <div className="regex-input-row explorer-search-hero__input-row">
                <input
                  autoFocus
                  aria-label={explorerT('filterKeywordAria')}
                  className={`explorer-search-hero__input ${
                    regexMode && !regexValid ? 'input-invalid' : ''
                  }`}
                  placeholder={explorerT('searchHeroPlaceholder')}
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
            <div
              className="segmented-row explorer-search-hero__modes"
              role="group"
              aria-label={explorerT('searchHeroLabel')}
            >
              {(['keyword', 'semantic', 'hybrid'] as const).map((option) => {
                const disabled =
                  option !== 'keyword' && !aiAvailability.available

                return (
                  <button
                    key={option}
                    className={`chip-button ${
                      mode === option ? 'chip-button--active' : ''
                    } ${disabled ? 'chip-button--muted' : ''}`}
                    aria-disabled={disabled || undefined}
                    title={disabled ? aiUnavailableTitle : undefined}
                    type="button"
                    onClick={() => {
                      if (disabled) return
                      updateParam('mode', option === 'keyword' ? null : option)
                    }}
                  >
                    {option === 'keyword'
                      ? explorerT('modeKeyword')
                      : option === 'semantic'
                        ? explorerT('modeSemantic')
                        : explorerT('modeHybrid')}
                  </button>
                )
              })}
            </div>
          </div>

          <div className="recent-search-bar explorer-search-hero__recent">
            <span className="mono-kicker">
              {explorerT('recentSearchesEyebrow')}
            </span>
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

        <div className="panel-body explorer-secondary-controls">
          <div
            className="segmented-row explorer-view-by"
            role="toolbar"
            aria-label={intelligenceT('viewModeLabel')}
          >
            <span className="mono-kicker explorer-view-by__label">
              {intelligenceT('viewModeLabel')}
            </span>
            {(['time', 'session', 'trail'] as const).map((option) => (
              <button
                key={option}
                className={`chip-button ${
                  view === option ? 'chip-button--active' : ''
                }`}
                type="button"
                disabled={mode !== 'keyword' && option !== 'time'}
                aria-pressed={view === option}
                onClick={() => setView(option)}
              >
                {option === 'time'
                  ? intelligenceT('viewModeTime')
                  : option === 'session'
                    ? intelligenceT('viewModeSession')
                    : intelligenceT('viewModeTrail')}
              </button>
            ))}
          </div>

          <div className="explorer-filters explorer-filters--secondary">
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
                {selectedProfileIds.map((id) => (
                  <option key={id} value={id}>
                    {explorerProfileLabel(id)}
                  </option>
                ))}
              </select>
              {!explicitProfileId && activeScopeLabel ? (
                <span className="mono-support">
                  {explorerT('scopeInherited', { profile: activeScopeLabel })}
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
                inputMode="numeric"
                pattern="\d{4}-\d{2}-\d{2}"
                placeholder={explorerT('allRecordedTime')}
                type="text"
                value={searchParams.get('start') ?? ''}
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
                inputMode="numeric"
                pattern="\d{4}-\d{2}-\d{2}"
                placeholder={explorerT('allRecordedTime')}
                type="text"
                value={searchParams.get('end') ?? ''}
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
      </div>
    </>
  )
}
