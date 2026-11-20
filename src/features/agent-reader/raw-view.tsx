/**
 * The raw records (`agent-history.md` §4.8): every record PathKeep keeps
 * byte for byte, one row each, with the row of the conversation it feeds
 * ("See in conversation") or "Not shown"; a row opens to the record's
 * JSON, with Copy. Read a window at a time, so a conversation of tens of
 * thousands of records opens at once; a record's bytes are read only when
 * it is opened.
 */
import { useQuery } from '@tanstack/react-query'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ChevronRight, Copy } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import {
  agentChatsClient,
  type ChatHeader,
  type ChatRawRow,
} from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useI18n, type Formatters } from '@/lib/i18n'
import { copyText } from './reader-words'
import { WINDOW, useWindows } from './use-windows'

const ROW = 32
const PAGE = 65_536
const GRID =
  'grid grid-cols-[56px_minmax(0,1fr)_72px_72px_120px] items-center gap-3'

/** A record's bytes, every page, as text; pretty JSON when it is JSON. */
async function readRecord(id: string, publication: string, row: ChatRawRow) {
  const chunks: number[] = []
  let offset: number | null = 0
  while (offset !== null) {
    const page = await agentChatsClient.record({
      id,
      publication,
      sourceId: row.sourceId,
      ordinal: row.ordinal,
      offset,
      limit: PAGE,
    })
    for (const byte of page.bytes) chunks.push(byte)
    offset = page.next
  }
  const text = new TextDecoder().decode(Uint8Array.from(chunks))
  try {
    return { text: JSON.stringify(JSON.parse(text), null, 2), json: true }
  } catch {
    return { text, json: false }
  }
}

function RecordText({
  id,
  publication,
  row,
}: {
  id: string
  publication: string
  row: ChatRawRow
}) {
  const { t } = useI18n()
  const record = useQuery({
    queryKey: [
      'archive',
      'agent-chats',
      'chat',
      id,
      'record',
      publication,
      row.sourceId,
      row.ordinal,
    ],
    queryFn: () => readRecord(id, publication, row),
    staleTime: Infinity,
  })
  if (record.isError)
    return (
      <p className="px-5 py-2 text-[12.5px]">
        {t('agentInfo.raw.readFailed')}{' '}
        <span className="text-muted-foreground">
          {describeError(record.error)}
        </span>
      </p>
    )
  if (!record.data)
    return (
      <div
        role="status"
        aria-label={t('agentInfo.raw.reading')}
        className="px-5 py-2"
      >
        <Skeleton className="h-16 w-full" />
      </div>
    )
  return (
    <div className="flex flex-col gap-1.5 px-5 pt-1 pb-3">
      {!record.data.json && (
        <p className="text-[12px] text-muted-foreground">
          {t('agentInfo.raw.unreadable')}
        </p>
      )}
      <pre className="max-h-[360px] overflow-auto rounded-md bg-muted p-2.5 font-mono text-[11.5px] leading-[1.5] whitespace-pre-wrap [overflow-wrap:anywhere]">
        {record.data.text}
      </pre>
      <Button
        size="sm"
        variant="outline"
        className="self-start"
        onClick={() => copyText(t, record.data.text, t('agentInfo.raw.copied'))}
      >
        <Copy aria-hidden />
        {t('agentInfo.raw.copy')}
      </Button>
    </div>
  )
}

export function RawView({
  header,
  f,
  onSee,
  onStale,
}: {
  header: ChatHeader
  f: Formatters
  /** Back to the conversation, at this rank. */
  onSee: (visibleRank: number) => void
  onStale: () => void
}) {
  const { t } = useI18n()
  const id = header.summary.id
  const scrollRef = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState({ first: 0, last: 0 })
  const [open, setOpen] = useState<string | null>(null)
  const { total, rankState } = useWindows<ChatRawRow>({
    id,
    publication: header.publication,
    view: 'raw',
    leaf: null,
    knownTotal: header.physicalRecords,
    range,
    onStale,
  })
  // eslint-disable-next-line react-hooks/incompatible-library -- the virtualizer is only read below
  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW,
    overscan: 16,
  })
  const items = virtualizer.getVirtualItems()
  const first = items[0]?.index ?? 0
  const last = items.at(-1)?.index ?? 0
  useEffect(() => {
    const next = {
      first: Math.floor(first / WINDOW) * WINDOW,
      last: Math.floor(last / WINDOW) * WINDOW,
    }
    setRange((previous) =>
      previous.first === next.first && previous.last === next.last
        ? previous
        : next,
    )
  }, [first, last])
  const hidden = Math.max(0, header.physicalRecords - header.visibleRecords)
  return (
    <section
      aria-label={t('agentInfo.raw.list')}
      className="flex min-h-0 min-w-0 flex-1 flex-col"
    >
      <p className="border-b px-5 py-2.5 text-[12.5px] leading-relaxed text-muted-foreground">
        {t('agentInfo.raw.intro', {
          count: f.number(total),
          hidden: f.number(hidden),
        })}
      </p>
      <div
        className={cn(
          GRID,
          'border-b px-5 py-1.5 text-[11.5px] font-medium text-muted-foreground',
        )}
        aria-hidden
      >
        <span>{t('agentInfo.raw.number')}</span>
        <span>{t('agentInfo.raw.type')}</span>
        <span>{t('agentInfo.raw.time')}</span>
        <span className="text-right">{t('agentInfo.raw.size')}</span>
        <span />
      </div>
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto">
        <div
          className="relative w-full"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {items.map((item) => {
            const state = rankState(item.index)
            const row = state.state === 'row' ? state.row : null
            const key = row ? `${row.sourceId}:${row.ordinal}` : null
            const expanded = key !== null && open === key
            return (
              <div
                key={item.key}
                data-index={item.index}
                ref={virtualizer.measureElement}
                className="absolute top-0 left-0 w-full"
                style={{ transform: `translateY(${item.start}px)` }}
              >
                {row ? (
                  <>
                    <div
                      className={cn(
                        GRID,
                        'px-5 font-mono text-[11.5px] transition-colors duration-150 hover:bg-muted/60',
                      )}
                      style={{ minHeight: ROW }}
                    >
                      <button
                        type="button"
                        aria-expanded={expanded}
                        aria-label={t('agentInfo.raw.expand', {
                          ordinal: row.ordinal + 1,
                        })}
                        onClick={() => setOpen(expanded ? null : key)}
                        className="flex items-center gap-1 text-left text-muted-foreground tabular hover:text-foreground"
                      >
                        <ChevronRight
                          className={cn(
                            'size-3 transition-transform duration-200',
                            expanded && 'rotate-90',
                          )}
                          aria-hidden
                        />
                        {row.ordinal + 1}
                      </button>
                      <span className="truncate" title={row.kind}>
                        {row.kind}
                      </span>
                      <span className="text-muted-foreground tabular">
                        {row.at === null ? '' : f.time(row.at)}
                      </span>
                      <span className="text-right text-muted-foreground tabular">
                        {f.bytes(row.bytes)}
                      </span>
                      {row.visible && row.visibleRank !== null ? (
                        <button
                          type="button"
                          onClick={() => onSee(row.visibleRank!)}
                          className="justify-self-end font-sans text-[12px] underline-offset-2 hover:underline"
                        >
                          {t('agentInfo.raw.see')}
                        </button>
                      ) : (
                        <span className="justify-self-end font-sans text-[11.5px] text-muted-foreground">
                          {row.mirrorOfOrdinal !== null
                            ? t('agentInfo.raw.mirror', {
                                ordinal: row.mirrorOfOrdinal + 1,
                              })
                            : t('agentInfo.raw.notShown')}
                        </span>
                      )}
                    </div>
                    {expanded && (
                      <RecordText
                        id={id}
                        publication={header.publication}
                        row={row}
                      />
                    )}
                  </>
                ) : state.state === 'failed' ? (
                  item.index % WINDOW === 0 || item.index === first ? (
                    <div className="flex items-center gap-2 px-5 py-2 text-[12.5px]">
                      {t('agentReader.rows.windowFailed')}
                      <Button size="sm" variant="outline" onClick={state.retry}>
                        {t('agentReader.rows.retry')}
                      </Button>
                    </div>
                  ) : null
                ) : (
                  <div
                    className="flex items-center px-5"
                    style={{ height: ROW }}
                  >
                    <Skeleton className="h-3 w-2/3" />
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>
    </section>
  )
}
