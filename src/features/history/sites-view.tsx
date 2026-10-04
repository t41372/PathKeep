/**
 * Sites: the busiest sites for the current date and browser filter, each with
 * the link preview of its latest page. Clicking one filters the timeline.
 */
import { ChevronRight } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { Skeleton } from '@/components/ui/skeleton'
import { useFormat, useI18n } from '@/lib/i18n'
import type { DayRange } from './history-params'
import { type HistoryFilters } from './history-types'
import { StateMessage } from './state-message'
import { PreviewThumb } from './link-preview'
import { useLatestVisit, useTopSites, type SiteCount } from './queries'
import { VirtualRows } from './virtual-rows'

const ROW_HEIGHT = 63

function SiteRow({
  site,
  max,
  filters,
  onPick,
}: {
  site: SiteCount
  max: number
  filters: HistoryFilters
  onPick: (domain: string) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  // Rows mount only while visible, so this fires for what is on screen.
  const latest = useLatestVisit(site.domain, { ...filters, domain: null })
  const lookup = useMemo(
    () =>
      latest.data
        ? {
            profileId: latest.data.profileId,
            url: latest.data.url,
            visitTime: latest.data.visitTime,
          }
        : undefined,
    [latest.data],
  )
  const title = latest.data?.title?.trim() || latest.data?.url

  return (
    <button
      type="button"
      role="option"
      aria-selected={false}
      aria-label={t('history.sites.show', { domain: site.domain })}
      onClick={() => onPick(site.domain)}
      className="flex h-full w-full items-center gap-3.5 border-b px-2.5 text-left outline-none transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      <PreviewThumb
        url={latest.data?.url ?? null}
        domain={site.domain}
        lookup={lookup}
      />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-medium">{site.domain}</span>
        <span className="h-4 truncate text-xs text-muted-foreground">
          {title ? (
            t('history.sites.latest', { title })
          ) : (
            <Skeleton className="mt-0.5 h-3 w-48" />
          )}
        </span>
      </span>
      <span className="h-1.5 w-[120px] shrink-0 overflow-hidden rounded-full bg-muted">
        <span
          className="block h-full rounded-full bg-brand"
          style={{ width: `${Math.max(2, (site.visits / max) * 100)}%` }}
        />
      </span>
      <span className="w-16 shrink-0 text-right font-mono text-xs text-muted-foreground tabular">
        {format.number(site.visits)}
      </span>
      <ChevronRight
        className="size-4 shrink-0 text-muted-foreground"
        aria-hidden
      />
    </button>
  )
}

export function SitesView({
  range,
  profileIds,
  filters,
  resetKey,
  onPick,
}: {
  range: DayRange | null
  profileIds: string[] | null
  filters: HistoryFilters
  resetKey: string
  onPick: (domain: string) => void
}) {
  const { t } = useI18n()
  const query = useTopSites(range, profileIds)
  const sites = query.data?.sites
  const max = Math.max(1, sites?.[0]?.visits ?? 1)

  const rowKey = useCallback((site: SiteCount) => site.domain, [])
  const rowHeight = useCallback(() => ROW_HEIGHT, [])
  const renderRow = useCallback(
    (site: SiteCount) => (
      <SiteRow site={site} max={max} filters={filters} onPick={onPick} />
    ),
    [max, filters, onPick],
  )

  if (query.isPending) return <StateMessage loading />
  if (query.error) {
    return (
      <StateMessage
        error={query.error}
        title={t('history.state.errorTitle')}
        onRetry={() => void query.refetch()}
      />
    )
  }
  if (!sites || sites.length === 0) {
    return <StateMessage icon="sites" title={t('history.state.noSitesTitle')} />
  }
  return (
    <VirtualRows
      rows={sites}
      rowKey={rowKey}
      rowHeight={rowHeight}
      renderRow={renderRow}
      label={t('history.views.sites')}
      resetKey={resetKey}
    />
  )
}
