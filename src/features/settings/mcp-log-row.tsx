/**
 * Settings → AI → What other apps read: every request an outside app made
 * through PathKeep's MCP server, newest first (`load_mcp_queries`), so the
 * person can see what left PathKeep and when.
 *
 * Shown while MCP is on, and afterwards as long as there is a request to
 * show. Requests can number in the thousands, so the list is windowed and
 * reads on a page at a time as it scrolls.
 *
 * Not responsible for the MCP switch or the connection details (`ai-section`).
 */
import { useInfiniteQuery } from '@tanstack/react-query'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useEffect, useMemo, useRef } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { McpQueryRecord } from '@/lib/types'

const PAGE = 50
const ROW_HEIGHT = 52
const knownTools = ['search-history', 'archive-status', 'usage-guide'] as const

export function McpLogRow() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const on = ai.enabled && ai.mcpEnabled
  const log = useInfiniteQuery({
    queryKey: ['ai', 'mcp-queries'],
    queryFn: ({ pageParam }) =>
      intelligenceClient.listMcpQueries(pageParam, PAGE),
    initialPageParam: null as number | null,
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    staleTime: 0,
  })
  const records = useMemo(
    () => log.data?.pages.flatMap((page) => page.items) ?? [],
    [log.data],
  )
  if (!on && !log.isPending && records.length === 0) return null

  return (
    <SettingRow
      title={t('settingsAi.mcpLog.title')}
      description={t('settingsAi.mcpLog.description')}
    >
      {log.isPending ? (
        <div className="flex flex-col gap-2 border-t pt-3" aria-hidden>
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="h-4 w-1/2" />
        </div>
      ) : records.length === 0 ? (
        <p className="border-t pt-3 text-[13px] text-muted-foreground">
          {log.isError
            ? t('settingsAi.mcpLog.loadFailed', {
                message: describeError(log.error, 'load_mcp_queries'),
              })
            : t('settingsAi.mcpLog.empty')}
        </p>
      ) : (
        <Requests
          records={records}
          hasMore={log.hasNextPage}
          loadingMore={log.isFetchingNextPage}
          loadError={log.isError ? log.error : null}
          onLoadMore={() => void log.fetchNextPage()}
        />
      )}
    </SettingRow>
  )
}

function Requests({
  records,
  hasMore,
  loadingMore,
  loadError,
  onLoadMore,
}: {
  records: McpQueryRecord[]
  hasMore: boolean
  loadingMore: boolean
  loadError: unknown
  onLoadMore: () => void
}) {
  const { t } = useI18n()
  const scrollRef = useRef<HTMLDivElement>(null)
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: records.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (index) => records[index].id,
    overscan: 8,
  })
  const items = virtualizer.getVirtualItems()
  const lastIndex = items.at(-1)?.index ?? -1
  useEffect(() => {
    if (
      hasMore &&
      !loadingMore &&
      !loadError &&
      lastIndex >= records.length - 8
    )
      onLoadMore()
  }, [hasMore, loadingMore, loadError, lastIndex, records.length, onLoadMore])

  return (
    <div ref={scrollRef} className="max-h-[364px] overflow-y-auto border-t">
      <ul
        aria-label={t('settingsAi.mcpLog.label')}
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {items.map((item) => (
          <li
            key={item.key}
            data-mcp-query={records[item.index].id}
            className="absolute top-0 left-0 flex w-full flex-col justify-center gap-0.5 border-b"
            style={{
              height: item.size,
              transform: `translateY(${item.start}px)`,
            }}
          >
            <Request record={records[item.index]} />
          </li>
        ))}
      </ul>
      <div className="py-2 text-xs text-muted-foreground" aria-live="polite">
        {loadError ? (
          <span className="flex items-center gap-2 text-destructive">
            {t('settingsAi.mcpLog.moreFailed', {
              message: describeError(loadError, 'load_mcp_queries'),
            })}
            <Button size="xs" variant="outline" onClick={onLoadMore}>
              {t('common.retry')}
            </Button>
          </span>
        ) : loadingMore ? (
          <span className="flex items-center gap-2">
            <Spinner />
            {t('settingsAi.mcpLog.loadingMore')}
          </span>
        ) : hasMore ? null : (
          t('settingsAi.mcpLog.allShown')
        )}
      </div>
    </div>
  )
}

function Request({ record }: { record: McpQueryRecord }) {
  const { t } = useI18n()
  const format = useFormat()
  const tool = (knownTools as readonly string[]).includes(record.tool)
    ? t(`settingsAi.mcpLog.tool.${record.tool as (typeof knownTools)[number]}`)
    : t('settingsAi.mcpLog.tool.other', { tool: record.tool })
  const details = [
    format.dayAndTime(record.at),
    record.results != null &&
      t('settingsAi.mcpLog.results', { count: record.results }),
    record.domain && t('settingsAi.mcpLog.onSite', { site: record.domain }),
  ].filter(Boolean)
  return (
    <>
      <span className="truncate text-[13px]">
        <span className="font-medium">{tool}</span>
        {record.query && (
          <span className="text-muted-foreground">
            {' · '}
            {t('settingsAi.mcpLog.asked', { query: record.query })}
          </span>
        )}
      </span>
      <span className="truncate text-xs text-muted-foreground tabular-nums">
        {details.join(' · ')}
      </span>
    </>
  )
}
