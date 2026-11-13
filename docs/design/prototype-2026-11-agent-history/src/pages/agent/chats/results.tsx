/**
 * Full-text results, streamed: one card per conversation with up to three
 * matching passages, counts that only grow while the scan runs, and an exact
 * total at the end. Nothing is cut off; the list scrolls to the last match.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import { Check, Regex } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Progress } from '@/components/ui/progress'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { HarnessBadge, ProjectChip } from '~/components/bits'
import { ArchivedFlag, OnlyHereFlag } from '~/components/chat-row'
import { archive, projectOf } from '~/data/archive'
import { num, when } from '~/data/format'
import { roleLabels, runSearch, type ChatHit, type SearchState } from '~/data/search'
import type { Role } from '~/data/transcript'
import { useChatEdits } from './chat-edits'
import { applyFilters, type Filters } from './filters'

const roles: Role[] = ['user', 'agent', 'tool', 'thinking']
const shortRole: Record<Role, string> = { user: '你', agent: 'Agent', tool: '工具', thinking: '思考' }

function ResultCard({ hit, selected, onOpen }: { hit: ChatHit; selected: boolean; onOpen: (event?: number) => void }) {
  const { chat } = hit
  const archived = useChatEdits().archived.has(chat.id)
  return (
    <div className="px-2 py-1">
      <div className={cn('flex flex-col gap-1.5 rounded-lg px-3 py-2.5', selected ? 'bg-card shadow-card' : 'hover:bg-foreground/[0.035]')}>
        <button type="button" className="flex min-w-0 items-center gap-2 text-left" onClick={() => onOpen(hit.snippets[0]?.event)}>
          <HarnessBadge id={chat.harness} />
          <span className="truncate text-[13.5px] font-medium">{chat.title}</span>
          <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">{num(hit.hits)} 處</span>
        </button>
        <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
          <ProjectChip project={projectOf(chat)} link={false} className="-mx-1.5" />
          <span className="tabular">{when(chat.ended)}</span>
          {chat.source === 'gone' && <OnlyHereFlag compact />}
          {archived && <ArchivedFlag withText />}
        </div>
        {hit.snippets.map((s, i) => (
          <button
            key={i}
            type="button"
            onClick={() => onOpen(s.event)}
            className="flex min-w-0 items-baseline gap-2 rounded-md px-1.5 py-1 text-left text-[12.5px] leading-relaxed hover:bg-muted"
          >
            <span className="w-[36px] shrink-0 text-[11px] text-muted-foreground">{shortRole[s.role]}</span>
            <span className="line-clamp-2 min-w-0 text-foreground/85">
              {s.before}
              <mark className="hit">{s.match}</mark>
              {s.after}
            </span>
          </button>
        ))}
        {hit.hits > hit.snippets.length && <span className="pl-[50px] text-[11.5px] text-muted-foreground">這段對話裡還有 {num(hit.hits - hit.snippets.length)} 處，打開後按 Enter 逐一看</span>}
      </div>
    </div>
  )
}

export function SearchResults({
  filters,
  update,
  onOpen,
}: {
  filters: Filters
  update: (patch: Partial<Record<keyof Filters, string | number | boolean | string[] | undefined>>) => void
  onOpen: (chatId: number, event?: number) => void
}) {
  const [state, setState] = useState<SearchState | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  // Time and the 更多 flags narrow the search too, the same way they narrow the list.
  // Search looks through put-away conversations too; deleted ones are gone.
  const edits = useChatEdits()
  const pool = useMemo(
    () => applyFilters({ ...filters, sort: 'recent' }, edits, true),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [filters.project, filters.harnesses.join(), filters.range, filters.flags.join(), edits.archived, edits.deleted],
  )
  useEffect(() => {
    setState(null)
    return runSearch({ q: filters.q, regex: filters.regex, roles: filters.roles, harnesses: [], pool }, setState)
  }, [filters.q, filters.regex, filters.roles, pool])

  const results = state?.results ?? []
  const virtualizer = useVirtualizer({
    count: results.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 132,
    overscan: 6,
  })
  const toggleRole = (r: Role) => {
    const next = filters.roles.includes(r) ? filters.roles.filter((x) => x !== r) : [...filters.roles, r]
    if (next.length) update({ roles: roles.filter((x) => next.includes(x)) })
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-col gap-2 border-b px-4 pb-2.5">
        <div className="flex items-center gap-1.5">
          <span className="mr-1 text-[12px] text-muted-foreground">搜哪些內容</span>
          {roles.map((r) => (
            <button
              key={r}
              type="button"
              aria-pressed={filters.roles.includes(r)}
              onClick={() => toggleRole(r)}
              className={cn(
                'h-6 rounded-md border px-2 text-[12px] transition-colors',
                filters.roles.includes(r) ? 'border-brand/35 bg-brand-soft text-foreground' : 'border-dashed text-muted-foreground line-through decoration-muted-foreground/50 hover:bg-muted',
              )}
            >
              {filters.roles.includes(r) && <Check className="mr-0.5 inline size-3" />}
              {roleLabels[r]}
            </button>
          ))}
          <span className="flex-1" />
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-pressed={filters.regex}
                aria-label="正規表示式"
                onClick={() => update({ regex: !filters.regex })}
                className={cn('flex h-6 items-center gap-1 rounded-md border px-1.5 text-[12px]', filters.regex ? 'border-brand/35 bg-brand-soft' : 'text-muted-foreground hover:bg-muted')}
              >
                <Regex className="size-3.5" />
                正規
              </button>
            </TooltipTrigger>
            <TooltipContent>用正規表示式搜尋</TooltipContent>
          </Tooltip>
        </div>
        <div className="flex items-center gap-3 text-[12px] text-muted-foreground tabular" aria-live="polite">
          {state?.error ? (
            <span className="text-destructive">{state.error}</span>
          ) : state?.done ? (
            <span>
              {num(state.results.length)} 段對話、共 {num(state.hits)} 處符合 · 搜完{pool.length < archive.chats.length ? '篩選後的' : ''} {num(state.total)} 段
            </span>
          ) : (
            <>
              <span className="shrink-0">
                已找到 {num(state?.results.length ?? 0)} 段、{num(state?.hits ?? 0)} 處 · 搜尋中 {num(state?.scanned ?? 0)} / {num(state?.total ?? 0)}
              </span>
              <Progress value={state ? (state.scanned / Math.max(1, state.total)) * 100 : 0} className="h-1 flex-1" />
            </>
          )}
        </div>
      </div>
      {state?.done && results.length === 0 && !state.error ? (
        <div className="flex flex-1 items-center justify-center p-8 text-center text-[13px] text-muted-foreground">
          沒有對話提到「{filters.q}」。
          <br />
          搜尋涵蓋全部 {num(state.total)} 段對話的每一則訊息與工具輸出。
        </div>
      ) : (
        <div ref={scrollRef} className="thin-scroll min-h-0 flex-1 overflow-y-auto pt-1">
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((v) => {
              const hit = results[v.index]
              return (
                <div key={hit.chat.id} data-index={v.index} ref={virtualizer.measureElement} style={{ position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${v.start}px)` }}>
                  <ResultCard hit={hit} selected={hit.chat.id === filters.chat} onOpen={(event) => onOpen(hit.chat.id, event)} />
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
