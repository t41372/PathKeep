/**
 * History: timeline, sites and starred views with search and filters, and a
 * detail panel for the selected page.
 *
 * Responsible for: wiring URL state, queries and selection into the toolbar,
 * the lists and the panel.
 * Not responsible for: row rendering, data fetching details or the panel's
 * contents (see the sibling files).
 */
import { TriangleAlert } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { DetailPanel } from './detail-panel'
import { HistoryToolbar } from './history-toolbar'
import { openInBrowser } from './open-link'
import {
  dayRange,
  parseDateFilter,
  resolveSearch,
  timeBounds,
  useHistoryParams,
  type HistoryView,
  type SearchMode,
} from './history-params'
import {
  targetOf,
  type DetailTarget,
  type HistoryFilters,
  type VisitItem,
} from './history-types'
import { useBrowserCatalog, useStarCounts, useVisitList } from './queries'
import { SitesView } from './sites-view'
import { StarredView } from './starred-view'
import { StateMessage } from './state-message'
import { buildTimeline, visitRows, type TimelineRow } from './timeline-model'
import { useListKeys } from './use-list-keys'
import { useStars } from './use-stars'
import { VisitRows } from './visit-rows'

const SEARCH_DEBOUNCE_MS = 250

/**
 * The query box. Typing is held locally and debounced into the URL; with
 * nothing pending the box just shows the URL's value, so links from Home or
 * the palette appear in it.
 */
function useSearchDraft(q: string, commit: (q: string) => void) {
  const [draft, setDraft] = useState<string | null>(null)
  if (draft !== null && draft === q) setDraft(null)

  useEffect(() => {
    if (draft === null) return
    const timer = setTimeout(() => commit(draft), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [draft, commit])

  return { value: draft ?? q, setDraft }
}

export default function HistoryPage() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const { params, update } = useHistoryParams()
  const catalog = useBrowserCatalog()
  const starCounts = useStarCounts()

  const commitQuery = useCallback((q: string) => update({ q }), [update])
  const { value: draft, setDraft } = useSearchDraft(params.q, commitQuery)

  const ai = snapshot.config.ai
  const semanticAvailable =
    ai.enabled && ai.semanticIndexEnabled && snapshot.aiStatus.ready
  const search = useMemo(
    () => resolveSearch(params.q, params.mode, semanticAvailable),
    [params.q, params.mode, semanticAvailable],
  )
  const searching = search.text !== ''
  const showList = searching || params.view === 'timeline'

  const range = useMemo(
    () => dayRange(parseDateFilter(params.date)),
    [params.date],
  )
  const filters = useMemo<HistoryFilters>(
    () => ({
      ...timeBounds(range),
      browserKind: params.browser,
      domain: params.domain,
    }),
    [range, params.browser, params.domain],
  )
  const hasFilters =
    params.date !== null || params.browser !== null || params.domain !== null
  const resetKey = `${search.text}|${search.mode}|${JSON.stringify(filters)}`

  const list = useVisitList(search, filters, showList)
  const { items, hasNextPage, isFetchingNextPage, fetchNextPage } = list

  const timeline = useMemo(
    () => (searching ? null : buildTimeline(items, hasNextPage)),
    [searching, items, hasNextPage],
  )
  const rows = useMemo<TimelineRow[]>(() => {
    const base = timeline ? timeline.rows : visitRows(items)
    if (!isFetchingNextPage) return base
    return [
      ...base,
      ...[0, 1, 2].map(
        (n): TimelineRow => ({ kind: 'loading', key: `loading:${n}` }),
      ),
    ]
  }, [timeline, items, isFetchingNextPage])

  const [selected, setSelected] = useState<DetailTarget | null>(null)
  const [retained, setRetained] = useState(selected)
  if (selected && selected !== retained) setRetained(selected)
  const panelTarget = selected ?? retained

  const [appliedVisit, setAppliedVisit] = useState<number | null>(null)
  const select = useCallback(
    (item: VisitItem) => {
      setSelected(targetOf(item))
      setAppliedVisit(item.id)
      update({ visit: item.id })
    },
    [update],
  )
  const selectTarget = useCallback(
    (target: DetailTarget) => {
      setSelected(target)
      update({ visit: null })
    },
    [update],
  )
  const close = useCallback(() => {
    setSelected(null)
    update({ visit: null })
  }, [update])

  // A `?visit=` link selects that visit once it shows up in the loaded pages.
  if (params.visit === null && appliedVisit !== null) setAppliedVisit(null)
  if (params.visit !== null && params.visit !== appliedVisit) {
    const found = items.find((item) => item.id === params.visit)
    if (found) {
      setAppliedVisit(params.visit)
      setSelected(targetOf(found))
    }
  }

  const starUrls = useMemo(
    () =>
      selected
        ? [...items.map((item) => item.url), selected.url]
        : items.map((item) => item.url),
    [items, selected],
  )
  const stars = useStars(starUrls)

  const open = useCallback(
    (url: string) => void openInBrowser(url, t('history.detail.openFailed')),
    [t],
  )
  const ids = useMemo(() => items.map((item) => item.id), [items])
  useListKeys({
    ids,
    selectedId: selected?.visitId ?? null,
    onSelectIndex: (index) => select(items[index]),
    onOpen: () => selected && open(selected.url),
    onClose: close,
  })

  const scrollIndex = useMemo(() => {
    const id = selected?.visitId
    return id === undefined
      ? null
      : rows.findIndex((row) => row.kind === 'visit' && row.item.id === id)
  }, [rows, selected?.visitId])

  const onEndReached = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) fetchNextPage()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  const setView = (view: HistoryView) => {
    setDraft(null)
    update({ view, q: '' })
  }
  const filterBySite = useCallback(
    (domain: string) => {
      setDraft(null)
      update({ view: 'timeline', domain, q: '' })
    },
    [setDraft, update],
  )
  const clearFilters = () => update({ date: null, browser: null, domain: null })

  const modeLabel = t(`history.modes.${search.mode}`)
  const starredCount = starCounts.data
    ? starCounts.data.urls + starCounts.data.domains
    : null
  const profileIds = params.browser
    ? (catalog.options.find((option) => option.kind === params.browser)
        ?.profileIds ?? null)
    : null

  const resultsLine = (() => {
    if (!searching) return null
    if (search.regexError) {
      return (
        <span className="flex items-center gap-1.5 text-destructive">
          <TriangleAlert className="size-3.5" aria-hidden />
          {t('history.modes.invalidRegex')}
        </span>
      )
    }
    if (list.total !== null)
      return t('history.results.count', { count: list.total, mode: modeLabel })
    if (list.isPending || items.length === 0) return null
    return t(
      hasNextPage ? 'history.results.countMore' : 'history.results.count',
      {
        count: items.length,
        mode: modeLabel,
      },
    )
  })()

  const listContent = () => {
    if (search.regexError) return null
    if (list.isPending) return <StateMessage loading />
    if (list.error) {
      return (
        <StateMessage
          error={list.error}
          title={t(
            searching
              ? 'history.state.searchErrorTitle'
              : 'history.state.errorTitle',
          )}
          onRetry={list.refetch}
        />
      )
    }
    if (items.length === 0) {
      if (searching) {
        return (
          <StateMessage
            icon="search"
            title={t('history.state.noResultsTitle')}
            body={t('history.state.noResultsBody', { query: search.text })}
          />
        )
      }
      if (hasFilters) {
        return (
          <StateMessage
            icon="filter"
            title={t('history.state.noMatchTitle')}
            body={t('history.state.noMatchBody')}
          >
            <Button variant="outline" size="sm" onClick={clearFilters}>
              {t('history.filters.clear')}
            </Button>
          </StateMessage>
        )
      }
      return (
        <StateMessage
          icon="history"
          title={t('history.state.noHistoryTitle')}
          body={t('history.state.noHistoryBody')}
        >
          <Button asChild size="sm">
            <Link to="/backup">{t('history.state.goBackup')}</Link>
          </Button>
        </StateMessage>
      )
    }
    return (
      <VisitRows
        rows={rows}
        variant={searching ? 'results' : 'timeline'}
        label={t(
          searching
            ? 'history.results.listLabel'
            : 'history.results.timelineLabel',
        )}
        selectedId={selected?.visitId ?? null}
        isStarred={stars.isStarred}
        browserName={catalog.nameOf}
        onSelect={select}
        onOpen={(item) => open(item.url)}
        onEndReached={onEndReached}
        scrollToIndex={scrollIndex}
        resetKey={resetKey}
        dimmed={list.isPlaceholder}
      />
    )
  }

  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <HistoryToolbar
        draft={draft}
        onDraft={setDraft}
        view={searching ? ('' as HistoryView) : params.view}
        onView={setView}
        starredCount={starredCount}
        date={params.date}
        onDate={(date) => update({ date })}
        browser={params.browser}
        browsers={catalog.options}
        onBrowser={(browser) => update({ browser })}
        domain={params.domain}
        onClearDomain={() => update({ domain: null })}
        hasFilters={hasFilters}
        onClearFilters={clearFilters}
        searching={draft.trim() !== ''}
        mode={search.mode}
        onMode={(mode: SearchMode) => update({ mode })}
        semanticAvailable={semanticAvailable}
        invalidRegex={search.regexError}
      />
      <div className="flex min-h-0 flex-1">
        <section className="flex min-w-0 flex-1 flex-col">
          {resultsLine && (
            <p
              className="px-7 pt-3.5 pb-1.5 text-[13px] text-muted-foreground"
              aria-live="polite"
            >
              {resultsLine}
            </p>
          )}
          {showList ? (
            listContent()
          ) : params.view === 'sites' ? (
            <SitesView
              range={range}
              profileIds={profileIds}
              filters={filters}
              resetKey={resetKey}
              onPick={filterBySite}
            />
          ) : (
            <StarredView
              selectedUrl={selected?.url ?? null}
              onSelect={selectTarget}
              onPickSite={filterBySite}
            />
          )}
        </section>
        <aside
          inert={!selected}
          className={cn(
            'shrink-0 overflow-hidden transition-[width] duration-200 ease-out motion-reduce:transition-none',
            selected ? 'w-80' : 'w-0',
          )}
        >
          <div className="h-full w-80 border-l">
            {panelTarget && (
              <DetailPanel
                target={panelTarget}
                starred={stars.isStarred(panelTarget.url)}
                sessionMates={
                  timeline && panelTarget.visitId !== undefined
                    ? timeline.sessionMates(panelTarget.visitId)
                    : []
                }
                onClose={close}
                onToggleStar={() =>
                  void stars.toggle(
                    panelTarget,
                    !stars.isStarred(panelTarget.url),
                  )
                }
                onSelectMate={select}
              />
            )}
          </div>
        </aside>
      </div>
    </div>
  )
}
