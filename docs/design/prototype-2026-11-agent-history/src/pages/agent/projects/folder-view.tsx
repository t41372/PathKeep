/**
 * 資料夾: projects by where they live on disk. Starts collapsed (one row per
 * top-level folder with its counts), folds single-child chains into one
 * label (`~/work/acme`, not `~` › `work` › `acme`), opens on click, and
 * never lists every project at once. Searching opens the folders that
 * contain a match.
 */
import { Search, X } from 'lucide-react'
import { memo, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { num } from '~/data/format'
import type { Row } from './derive'
import { Card, COLS, ColumnHead, GroupCell, NameCell, Num, Tools, usePageList, When } from './grid'
import { ProjectRowMenu } from './project-menu'
import { dirOf, matches, useProjectRows, type Locator } from './use-rows'

interface Dir {
  path: string
  label: string
  dirs: Dir[]
  rows: Row[]
  count: number
  chats: number
  last: number
}

const SCRATCH = '@scratch'

function buildTree(list: Row[], keep: (r: Row) => boolean): Dir {
  const root: Dir = { path: '', label: '', dirs: [], rows: [], count: 0, chats: 0, last: 0 }
  const index = new Map<string, Dir>([['', root]])
  const ensure = (path: string): Dir => {
    const hit = index.get(path)
    if (hit) return hit
    const parentPath = path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : ''
    const parent = ensure(parentPath)
    const d: Dir = { path, label: path.slice(parentPath.length ? parentPath.length + 1 : 0), dirs: [], rows: [], count: 0, chats: 0, last: 0 }
    parent.dirs.push(d)
    index.set(path, d)
    return d
  }
  for (const row of list) {
    if (!keep(row)) continue
    if (row.project.kind === 'scratch') {
      const d = ensure(SCRATCH)
      d.label = '家目錄與暫存資料夾'
      d.rows.push(row)
      continue
    }
    ensure(dirOf(row.project.locations[0].path)).rows.push(row)
  }
  // Fold chains: a folder with one subfolder and no projects of its own.
  const fold = (d: Dir) => {
    d.dirs = d.dirs.map((c) => {
      let x = c
      while (x.rows.length === 0 && x.dirs.length === 1) {
        const only = x.dirs[0]
        x = { ...only, label: `${x.label}/${only.label}` }
      }
      fold(x)
      return x
    })
  }
  fold(root)
  const total = (d: Dir) => {
    d.count = d.rows.length
    d.chats = d.rows.reduce((s, r) => s + r.chatCount, 0)
    d.last = d.rows.reduce((m, r) => Math.max(m, r.lastActive), 0)
    for (const c of d.dirs) {
      total(c)
      d.count += c.count
      d.chats += c.chats
      d.last = Math.max(d.last, c.last)
    }
    d.dirs.sort((a, b) => b.last - a.last)
    d.rows.sort((a, b) => b.lastActive - a.lastActive)
  }
  total(root)
  // `~` itself only holds `~/dotfiles`-style projects; show its children at the top.
  const home = root.dirs.find((d) => d.path === '~')
  if (home) {
    for (const c of home.dirs) c.label = `~/${c.label}`
    root.dirs = [...home.dirs, ...root.dirs.filter((d) => d !== home)].sort((a, b) => b.last - a.last)
    root.rows = home.rows
  }
  return root
}

type Item = { type: 'dir'; key: string; dir: Dir; depth: number; open: boolean } | { type: 'row'; key: string; row: Row; depth: number }

const TreeRow = memo(function TreeRow({ row, name, where, depth }: { row: Row; name: string; where: Locator; depth: number }) {
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
      className={cn(COLS, 'group relative h-10 cursor-default rounded-lg pr-1 pl-5 outline-none hover:bg-foreground/[0.035] focus-visible:ring-2 focus-visible:ring-ring')}
    >
      <NameCell row={row} name={name} where={where} depth={depth} />
      <Tools ids={row.harnesses} />
      <Num>{num(row.chatCount)}</Num>
      <When t={row.lastActive} />
      <ProjectRowMenu id={id} label={name} open={menu} onOpenChange={setMenu} />
    </div>
  )
})

export function FolderView() {
  const { list, name, where } = useProjectRows()
  const [q, setQ] = useState('')
  const [open, setOpen] = useState<Set<string>>(new Set())
  const needle = q.trim().toLowerCase()
  const tree = useMemo(() => buildTree(list, (r) => matches(r, name(r.project.id), needle)), [list, name, needle])
  const items = useMemo<Item[]>(() => {
    const out: Item[] = []
    const walk = (d: Dir, depth: number) => {
      for (const c of d.dirs) {
        const isOpen = !!needle || open.has(c.path)
        out.push({ type: 'dir', key: `d:${c.path}`, dir: c, depth, open: isOpen })
        if (isOpen) walk(c, depth + 1)
      }
      for (const r of d.rows) out.push({ type: 'row', key: `r:${r.project.id}`, row: r, depth })
    }
    walk(tree, 0)
    return out
  }, [tree, open, needle])
  const { ref, v, margin } = usePageList(items.length)
  const toggle = (path: string) =>
    setOpen((s) => {
      const next = new Set(s)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  return (
    <Card
      title="依資料夾"
      count={list.length}
      actions={
        <div className="flex items-center gap-2">
          {open.size > 0 && !needle && (
            <button type="button" onClick={() => setOpen(new Set())} className="text-[12.5px] text-muted-foreground hover:text-foreground">
              全部收合
            </button>
          )}
          <div className="relative w-[240px]">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="找名稱、資料夾或遠端" aria-label="搜尋專案" className="h-8 pr-7 pl-8 text-[13px]" />
            {q && (
              <button type="button" aria-label="清除搜尋" onClick={() => setQ('')} className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground">
                <X className="size-3.5" />
              </button>
            )}
          </div>
        </div>
      }
    >
      <ColumnHead />
      {items.length === 0 ? (
        <p className="px-5 py-8 text-center text-[13px] text-muted-foreground">沒有符合「{q}」的專案</p>
      ) : (
        <div ref={ref} role="rowgroup" className="relative mb-2" style={{ height: v.getTotalSize() + 4 }}>
          {v.getVirtualItems().map((it) => {
            const item = items[it.index]
            return (
              <div key={item.key} className="absolute inset-x-0 top-0" style={{ transform: `translateY(${it.start - margin + 4}px)` }}>
                {item.type === 'dir' ? (
                  <div
                    role="row"
                    aria-expanded={item.open}
                    tabIndex={0}
                    title={item.dir.path === SCRATCH ? undefined : item.dir.path}
                    onClick={() => !needle && toggle(item.dir.path)}
                    onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), !needle && toggle(item.dir.path))}
                    className={cn(COLS, 'h-10 cursor-default rounded-lg pr-1 pl-5 outline-none hover:bg-foreground/[0.035] focus-visible:ring-2 focus-visible:ring-ring')}
                  >
                    <GroupCell open={item.open} label={item.dir.label} count={item.dir.count} depth={item.depth} mono={item.dir.path !== SCRATCH} />
                    <span />
                    <Num muted>{num(item.dir.chats)}</Num>
                    <When t={item.dir.last} />
                    <span />
                  </div>
                ) : (
                  <TreeRow row={item.row} name={name(item.row.project.id)} where={where(item.row)} depth={item.depth} />
                )}
              </div>
            )
          })}
        </div>
      )}
    </Card>
  )
}
