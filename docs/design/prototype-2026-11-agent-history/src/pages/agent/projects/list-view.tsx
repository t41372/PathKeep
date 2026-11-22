/**
 * 清單: every project in one sortable table, with the two number columns
 * the other views leave out (分開加總, Token). Same row grid, icons, date
 * format and ⋯ menu as 預設.
 */
import { Search, X } from 'lucide-react'
import { memo, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { big, duration, num } from '~/data/format'
import type { Row } from './derive'
import { Card, ColumnHead, LIST_COLS, NameCell, Num, Tools, usePageList, When, type SortKey } from './grid'
import { ProjectRowMenu } from './project-menu'
import { matches, useProjectRows, type Locator } from './use-rows'

const sorters: Record<SortKey, (a: Row, b: Row, name: (id: string) => string) => number> = {
  recent: (a, b) => a.lastActive - b.lastActive,
  chats: (a, b) => a.chatCount - b.chatCount,
  summed: (a, b) => a.agentMs - b.agentMs,
  tokens: (a, b) => a.tokens - b.tokens,
  name: (a, b, name) => name(b.project.id).localeCompare(name(a.project.id), 'zh-Hant'),
}

const ListRow = memo(function ListRow({ row, name, where }: { row: Row; name: string; where: Locator }) {
  const navigate = useNavigate()
  const [menu, setMenu] = useState(false)
  const id = row.project.id
  return (
    <div
      role="row"
      tabIndex={0}
      onClick={() => void navigate(`/agent/projects/${id}`)}
      onKeyDown={(e) => e.target === e.currentTarget && e.key === 'Enter' && void navigate(`/agent/projects/${id}`)}
      onContextMenu={(e) => {
        e.preventDefault()
        setMenu(true)
      }}
      className={cn(LIST_COLS, 'group relative h-10 cursor-default rounded-lg pr-1 pl-5 outline-none hover:bg-foreground/[0.035] focus-visible:ring-2 focus-visible:ring-ring')}
    >
      <NameCell row={row} name={name} where={where} />
      <Tools ids={row.harnesses} />
      <Num>{num(row.chatCount)}</Num>
      <Num muted>{duration(row.agentMs, true)}</Num>
      <span className="text-right text-[13px] text-muted-foreground tabular" title={row.tokensKnown < row.chatCount ? `${row.tokensKnown} / ${row.chatCount} 段有用量紀錄` : undefined}>
        {row.tokensKnown ? big(row.tokens) : '—'}
        {row.tokensKnown > 0 && row.tokensKnown < row.chatCount && <span className="text-muted-foreground/60">*</span>}
      </span>
      <When t={row.lastActive} />
      <ProjectRowMenu id={id} label={name} open={menu} onOpenChange={setMenu} />
    </div>
  )
})

export function ListView() {
  const { list, name, where } = useProjectRows()
  const [q, setQ] = useState('')
  const [sort, setSort] = useState<SortKey>('recent')
  const [desc, setDesc] = useState(true)
  const needle = q.trim().toLowerCase()
  const rows = useMemo(() => {
    const dir = desc ? -1 : 1
    return list.filter((r) => matches(r, name(r.project.id), needle)).sort((a, b) => dir * sorters[sort](a, b, name) || b.lastActive - a.lastActive)
  }, [list, name, needle, sort, desc])
  const { ref, v, margin } = usePageList(rows.length)
  return (
    <Card
      title="全部專案"
      count={list.length}
      actions={
        <div className="relative w-[240px]">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="找名稱、資料夾或遠端" aria-label="搜尋專案" className="h-8 pr-7 pl-8 text-[13px]" />
          {q && (
            <button type="button" aria-label="清除搜尋" onClick={() => setQ('')} className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground">
              <X className="size-3.5" />
            </button>
          )}
        </div>
      }
    >
      <ColumnHead
        list
        sort={sort}
        desc={desc}
        onSort={(key) => {
          if (key === sort) setDesc((d) => !d)
          else {
            setSort(key)
            setDesc(key !== 'name')
          }
        }}
      />
      {rows.length === 0 ? (
        <p className="px-5 py-8 text-center text-[13px] text-muted-foreground">沒有符合「{q}」的專案</p>
      ) : (
        <div ref={ref} role="rowgroup" className="relative mb-2" style={{ height: v.getTotalSize() + 4 }}>
          {v.getVirtualItems().map((it) => {
            const row = rows[it.index]
            return (
              <div key={row.project.id} className="absolute inset-x-0 top-0" style={{ transform: `translateY(${it.start - margin + 4}px)` }}>
                <ListRow row={row} name={name(row.project.id)} where={where(row)} />
              </div>
            )
          })}
        </div>
      )}
    </Card>
  )
}
