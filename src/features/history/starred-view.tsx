/** Starred: pages and whole sites the user has starred, with a note preview where there is one. */
import { Star } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import type { StarListItem } from '@/lib/backend-client/stars'
import type { DetailTarget } from './history-types'
import { SiteIcon } from './site-icon'
import { StateMessage } from './state-message'
import { useAnnotationList, useLatestVisit, useStarList } from './queries'
import { VirtualRows } from './virtual-rows'

const ROW_HEIGHT = 62

function StarRow({
  item,
  note,
  selected,
  onPick,
}: {
  item: StarListItem
  note: string | undefined
  selected: boolean
  onPick: (item: StarListItem) => void
}) {
  const { t } = useI18n()
  const latest = useLatestVisit(item.domain, null)
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
  const isSite = item.entityKind === 'domain'
  const title = isSite ? item.domain : item.title.trim() || item.entityKey
  const detail = isSite
    ? t('common.visits', { count: item.visitCount })
    : note?.trim() || item.entityKey

  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={() => onPick(item)}
      className={cn(
        'flex h-full w-full items-center gap-3 rounded-[10px] px-3 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/50',
        selected ? 'bg-brand-soft' : 'hover:bg-muted',
      )}
    >
      <Star
        className="size-[15px] shrink-0 fill-brand text-brand"
        aria-hidden
      />
      <SiteIcon
        domain={item.domain}
        lookup={lookup}
        className="size-6 text-xs"
      />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate font-medium">{title}</span>
        <span className="truncate text-xs text-muted-foreground">{detail}</span>
      </span>
      <span className="shrink-0 text-xs text-muted-foreground">
        {isSite ? t('history.starred.wholeSite') : item.domain}
      </span>
    </button>
  )
}

export function StarredView({
  selectedUrl,
  onSelect,
  onPickSite,
}: {
  selectedUrl: string | null
  onSelect: (target: DetailTarget) => void
  onPickSite: (domain: string) => void
}) {
  const { t } = useI18n()
  const stars = useStarList()
  const annotations = useAnnotationList()
  const notes = useMemo(
    () =>
      new Map(
        (annotations.data ?? []).map((entry) => [entry.url, entry.notes]),
      ),
    [annotations.data],
  )

  const onPick = useCallback(
    (item: StarListItem) => {
      if (item.entityKind === 'domain') onPickSite(item.domain)
      else
        onSelect({
          url: item.entityKey,
          title: item.title || null,
          domain: item.domain,
        })
    },
    [onSelect, onPickSite],
  )
  const rowKey = useCallback(
    (item: StarListItem) => `${item.entityKind}:${item.entityKey}`,
    [],
  )
  const rowHeight = useCallback(() => ROW_HEIGHT, [])
  const renderRow = useCallback(
    (item: StarListItem) => (
      <StarRow
        item={item}
        note={notes.get(item.entityKey)}
        selected={item.entityKind === 'url' && item.entityKey === selectedUrl}
        onPick={onPick}
      />
    ),
    [notes, selectedUrl, onPick],
  )

  if (stars.isPending) return <StateMessage loading />
  if (stars.error) {
    return (
      <StateMessage
        error={stars.error}
        title={t('history.state.errorTitle')}
        onRetry={() => void stars.refetch()}
      />
    )
  }
  if (!stars.data || stars.data.length === 0) {
    return (
      <StateMessage
        icon="star"
        title={t('history.state.noStarredTitle')}
        body={t('history.state.noStarredBody')}
      />
    )
  }
  return (
    <VirtualRows
      rows={stars.data}
      rowKey={rowKey}
      rowHeight={rowHeight}
      renderRow={renderRow}
      label={t('history.views.starred')}
    />
  )
}
