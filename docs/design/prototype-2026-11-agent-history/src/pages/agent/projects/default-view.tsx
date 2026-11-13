/**
 * 預設: two cards on one page.
 * - 釘選: what the user put on top — their own sections (nest, collapse,
 *   reorder) and pinned projects. + 區塊 and the section menu sit in this
 *   card's own header, where they act.
 * - 其他專案: everything not pinned, newest activity first, with its own
 *   search and filters, so you can find a project and drag it up.
 *
 * Dragging uses pointer events (no HTML5 drag-and-drop, no new dependency).
 * Drop targets are found under the pointer by their `data-drop` mark, so a
 * row dragged out of a filtered, virtualized list lands anywhere in 釘選;
 * the page scrolls when the pointer nears its top or bottom edge, a
 * collapsed section opens after a moment, Esc cancels. Every row's ⋯ menu
 * (or right-click) does the same without dragging.
 */
import { Check, ChevronDown, Ellipsis, FolderSearch, Pencil, Plus, Search, SortDesc, Trash2, X, MoveDown, MoveUp, Rows3 } from 'lucide-react'
import { memo, useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { cn } from '@/lib/cn'
import { HarnessIcon } from '~/components/bits'
import { harnesses, harnessOrder } from '~/data/archive'
import { DAY, NOW } from '~/data/fixture'
import { num } from '~/data/format'
import { folderWords } from '~/data/labels'
import type { HarnessId } from '~/data/types'
import {
  contains,
  createSection,
  deleteSection,
  findParent,
  move,
  nudge,
  placedIds,
  renameSection,
  resetArrangement,
  restore,
  setAllCollapsed,
  setCollapsed,
  snapshot,
  useArrangement,
  type Node,
  type Place,
  type SectionNode,
} from './arrangement'
import type { Row } from './derive'
import { Card, COLS, ColumnHead, Grip, GroupCell, INDENT, NameCell, Num, Tools, usePageList, When } from './grid'
import { MoveToSubmenu } from './move-menu'
import { ProjectRowMenu } from './project-menu'
import { openSuggestions } from './store'
import { matches, useProjectRows, type Locator } from './use-rows'

const LATEST = 15

interface Drag {
  id: string
  kind: Node['kind']
  label: string
}
/** `place: null` = back to 其他專案. */
interface Target {
  place: Place | null
  into: string | null
  other: boolean
  line: { top: number; left: number; width: number } | null
}

// ------------------------------------------------------------------ drag

function useDrag(root: Node[]) {
  const [drag, setDrag] = useState<Drag | null>(null)
  const [target, setTarget] = useState<Target | null>(null)
  const dragRef = useRef<Drag | null>(null)
  const targetRef = useRef<Target | null>(null)
  const press = useRef<{ x: number; y: number; drag: Drag } | null>(null)
  const pointer = useRef({ x: 0, y: 0 })
  const suppressClick = useRef(false)
  const wrapRef = useRef<HTMLDivElement>(null)
  const ghostRef = useRef<HTMLDivElement>(null)
  const rootRef = useRef(root)
  rootRef.current = root

  const compute = useCallback((x: number, y: number): Target | null => {
    const d = dragRef.current
    if (!d) return null
    const el = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>('[data-drop]')
    if (!el) return null
    const r = el.getBoundingClientRect()
    const at = (y - r.top) / r.height
    const ds = el.dataset
    const depth = Number(ds.depth ?? 0)
    const line = (bottom: boolean, lineDepth: number) => {
      const left = r.left + 20 + lineDepth * INDENT
      return { top: bottom ? r.bottom : r.top, left, width: r.right - 8 - left }
    }
    const tree = rootRef.current
    const ok = (place: Place | null) => place === null || d.kind === 'project' || !contains(tree, d.id, place.parent)
    const make = (place: Place | null, l: Target['line'], into: string | null = null, other = false): Target | null => (ok(place) ? { place, line: l, into, other } : null)
    const parent = ds.parent ? ds.parent : null
    const index = Number(ds.index ?? 0)
    switch (ds.drop) {
      case 'section': {
        const id = ds.id!
        if (d.kind === 'section' && contains(tree, d.id, id)) return null
        if (at < 0.28) return make({ parent, index }, line(false, depth))
        if (at > 0.72) return ds.open === '1' ? make({ parent: id, index: 0 }, line(true, depth + 1)) : make({ parent, index: index + 1 }, line(true, depth))
        return make({ parent: id, index: Number(ds.children ?? 0) }, null, id)
      }
      case 'placed':
        return at < 0.5 ? make({ parent, index }, line(false, depth)) : make({ parent, index: index + 1 }, line(true, depth))
      case 'empty':
        return make({ parent: ds.section!, index: 0 }, null, ds.section!)
      case 'pinned-root':
        return make({ parent: null, index: tree.length }, null, '@pinned')
      case 'other':
        return d.kind === 'project' ? { place: null, line: null, into: null, other: true } : null
      default:
        return null
    }
  }, [])

  const same = (a: Target | null, b: Target | null) =>
    (a === null) === (b === null) && a?.into === b?.into && a?.other === b?.other && a?.line?.top === b?.line?.top && a?.line?.left === b?.line?.left

  const refresh = useCallback(() => {
    const t = compute(pointer.current.x, pointer.current.y)
    if (!same(t, targetRef.current)) {
      targetRef.current = t
      setTarget(t)
    }
  }, [compute])

  const finish = useCallback((commit: boolean) => {
    const d = dragRef.current
    const t = targetRef.current
    dragRef.current = null
    targetRef.current = null
    setDrag(null)
    setTarget(null)
    document.body.style.removeProperty('user-select')
    document.body.style.removeProperty('cursor')
    if (!commit || !d || !t) return
    if (t.place === null && !findParent(snapshot(), d.id)) return // already in 其他專案
    const before = snapshot()
    if (!move(d.id, d.kind, t.place)) return
    const where = t.place === null ? '「其他專案」' : t.place.parent === null ? '「釘選」' : `「${titleOf(snapshot(), t.place.parent)}」`
    toast(`已把「${d.label}」移到${where}`, { action: { label: '復原', onClick: () => restore(before) } })
  }, [])

  useEffect(() => {
    let raf = 0
    let edgeSince = 0
    const loop = () => {
      const el = wrapRef.current?.closest<HTMLElement>('.overflow-y-auto')
      if (el && dragRef.current) {
        const rect = el.getBoundingClientRect()
        const y = pointer.current.y
        const edge = 64
        const depth = y < rect.top + edge ? rect.top + edge - y : y > rect.bottom - edge ? y - rect.bottom + edge : 0
        if (!depth) edgeSince = 0
        else if (!edgeSince) edgeSince = performance.now()
        // Faster the deeper into the edge, and up to 3× when held there.
        const ramp = depth ? 1 + Math.min(2, (performance.now() - edgeSince) / 600) : 0
        const speed = depth ? Math.sign(y - (rect.top + rect.bottom) / 2) * Math.min(90, Math.ceil((depth / 2) * ramp)) : 0
        if (speed) {
          el.scrollTop += speed
          refresh()
        }
      }
      raf = requestAnimationFrame(loop)
    }
    const onMove = (e: PointerEvent) => {
      pointer.current = { x: e.clientX, y: e.clientY }
      const p = press.current
      if (!p) return
      if (!dragRef.current) {
        if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < 5) return
        dragRef.current = p.drag
        setDrag(p.drag)
        suppressClick.current = true
        document.body.style.userSelect = 'none'
        document.body.style.cursor = 'grabbing'
        raf = requestAnimationFrame(loop)
      }
      if (ghostRef.current) ghostRef.current.style.transform = `translate(${e.clientX + 14}px, ${e.clientY + 10}px)`
      refresh()
    }
    const onUp = () => {
      const dragging = !!dragRef.current
      press.current = null
      cancelAnimationFrame(raf)
      if (dragging) finish(true)
      // The click that ends a drag must not open the project.
      setTimeout(() => (suppressClick.current = false), 0)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && dragRef.current) {
        e.preventDefault()
        press.current = null
        cancelAnimationFrame(raf)
        finish(false)
      }
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', onUp)
    window.addEventListener('keydown', onKey)
    return () => {
      cancelAnimationFrame(raf)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('pointercancel', onUp)
      window.removeEventListener('keydown', onKey)
    }
  }, [finish, refresh])

  // Hovering a collapsed section while dragging opens it.
  useEffect(() => {
    const into = target?.into
    if (!into || into === '@pinned') return
    const t = setTimeout(() => setCollapsed(into, false), 650)
    return () => clearTimeout(t)
  }, [target?.into])

  const onPointerDown = useCallback((e: ReactPointerEvent, d: Drag) => {
    if (e.button !== 0) return
    if ((e.target as HTMLElement).closest('[data-no-drag],input,button')) return
    press.current = { x: e.clientX, y: e.clientY, drag: d }
    pointer.current = { x: e.clientX, y: e.clientY }
  }, [])

  return { drag, target, onPointerDown, suppressClick, wrapRef, ghostRef, pointer }
}

type DragApi = ReturnType<typeof useDrag>

function titleOf(root: Node[], id: string): string {
  for (const n of root) {
    if (n.kind !== 'section') continue
    if (n.id === id) return n.title
    const t = titleOf(n.children, id)
    if (t) return t
  }
  return ''
}

// ------------------------------------------------------------------ rows

/** A project row; ⋯ and right-click open the same menu. */
const ProjectLine = memo(function ProjectLine({
  row,
  name,
  where,
  depth,
  drop,
  dragging,
  onPointerDown,
  suppressClick,
}: {
  row: Row
  name: string
  where: Locator
  depth: number
  drop?: Record<string, string>
  dragging: boolean
  onPointerDown: DragApi['onPointerDown']
  suppressClick: DragApi['suppressClick']
}) {
  const navigate = useNavigate()
  const [menu, setMenu] = useState(false)
  const id = row.project.id
  const open = () => {
    if (!suppressClick.current) void navigate(`/agent/projects/${id}`)
  }
  return (
    <div
      role="row"
      tabIndex={0}
      {...drop}
      onPointerDown={(e) => onPointerDown(e, { id, kind: 'project', label: name })}
      onClick={open}
      onKeyDown={(e) => e.target === e.currentTarget && e.key === 'Enter' && open()}
      onContextMenu={(e) => {
        e.preventDefault()
        setMenu(true)
      }}
      className={cn(COLS, 'group relative h-10 cursor-default rounded-lg pr-1 pl-5 outline-none select-none hover:bg-foreground/[0.035] focus-visible:ring-2 focus-visible:ring-ring', dragging && 'opacity-40')}
    >
      <Grip />
      <NameCell row={row} name={name} where={where} depth={depth} />
      <Tools ids={row.harnesses} />
      <Num>{num(row.chatCount)}</Num>
      <When t={row.lastActive} />
      <ProjectRowMenu id={id} label={name} open={menu} onOpenChange={setMenu} />
    </div>
  )
})

function SectionMenu({ s, onRename, onDelete, onAddChild, open, onOpenChange }: { s: SectionNode; onRename: () => void; onDelete: () => void; onAddChild: () => void; open: boolean; onOpenChange: (v: boolean) => void }) {
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          data-no-drag
          aria-label={`區塊「${s.title}」的選項`}
          onClick={(e) => e.stopPropagation()}
          className={cn('flex size-7 items-center justify-center rounded-md text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100', open && 'opacity-100')}
        >
          <Ellipsis className="size-4" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[200px]" onClick={(e) => e.stopPropagation()}>
        <DropdownMenuItem onSelect={onAddChild}>
          <Plus />
          在裡面新增區塊
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={onRename}>
          <Pencil />
          重新命名
        </DropdownMenuItem>
        <MoveToSubmenu id={s.id} kind="section" label={s.title} />
        <DropdownMenuItem onSelect={() => nudge(s.id, -1)}>
          <MoveUp />
          上移
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => nudge(s.id, 1)}>
          <MoveDown />
          下移
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem variant="destructive" onSelect={onDelete}>
          <Trash2 />
          刪除區塊…
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function SectionLine({
  s,
  depth,
  parent,
  index,
  count,
  chats,
  last,
  editing,
  setEditing,
  api,
  onDelete,
}: {
  s: SectionNode
  depth: number
  parent: string | null
  index: number
  count: number
  chats: number
  last: number
  editing: boolean
  setEditing: (id: string | null) => void
  api: DragApi
  onDelete: () => void
}) {
  const [menu, setMenu] = useState(false)
  const open = !s.collapsed
  return (
    <div
      role="row"
      aria-expanded={open}
      tabIndex={0}
      data-drop="section"
      data-id={s.id}
      data-parent={parent ?? ''}
      data-index={index}
      data-depth={depth}
      data-open={open ? '1' : '0'}
      data-children={s.children.length}
      onPointerDown={(e) => api.onPointerDown(e, { id: s.id, kind: 'section', label: s.title })}
      onClick={() => {
        if (!api.suppressClick.current && !editing) setCollapsed(s.id, open)
      }}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          setCollapsed(s.id, open)
        }
        if (e.key === 'F2') setEditing(s.id)
      }}
      onContextMenu={(e) => {
        e.preventDefault()
        setMenu(true)
      }}
      className={cn(
        COLS,
        'group relative h-10 cursor-default rounded-lg pr-1 pl-5 outline-none select-none hover:bg-foreground/[0.035] focus-visible:ring-2 focus-visible:ring-ring',
        api.target?.into === s.id && 'bg-brand-soft ring-1 ring-brand/50',
        api.drag?.id === s.id && 'opacity-40',
      )}
    >
      <Grip />
      <GroupCell open={open} label={s.title} count={count} depth={depth}>
        {editing ? (
          <input
            autoFocus
            defaultValue={s.title}
            aria-label="區塊名稱"
            onFocus={(e) => e.currentTarget.select()}
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.nativeEvent.isComposing) return
              if (e.key === 'Enter') {
                renameSection(s.id, e.currentTarget.value)
                setEditing(null)
              }
              if (e.key === 'Escape') setEditing(null)
            }}
            onBlur={(e) => {
              renameSection(s.id, e.currentTarget.value)
              setEditing(null)
            }}
            className="h-7 min-w-0 flex-1 rounded-md border bg-background px-1.5 text-[13px] font-semibold outline-none focus:border-ring"
          />
        ) : undefined}
      </GroupCell>
      <span />
      <Num muted>{num(chats)}</Num>
      <When t={last} />
      <SectionMenu s={s} open={menu} onOpenChange={setMenu} onRename={() => setEditing(s.id)} onDelete={onDelete} onAddChild={() => setEditing(createSection(s.id))} />
    </div>
  )
}

// ------------------------------------------------------------------ 釘選

function PinnedCard({ api, root }: { api: DragApi; root: Node[] }) {
  const { byId, name, where } = useProjectRows()
  const [editing, setEditing] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<SectionNode | null>(null)
  const stats = useCallback(
    (s: SectionNode): { count: number; chats: number; last: number } => {
      let count = 0
      let chats = 0
      let last = 0
      for (const c of s.children) {
        if (c.kind === 'project') {
          const r = byId.get(c.id)
          if (!r) continue
          count++
          chats += r.chatCount
          last = Math.max(last, r.lastActive)
        } else {
          const x = stats(c)
          count += x.count
          chats += x.chats
          last = Math.max(last, x.last)
        }
      }
      return { count, chats, last }
    },
    [byId],
  )
  const pinnedCount = useMemo(() => [...placedIds(root)].filter((id) => byId.has(id)).length, [root, byId])

  const askDelete = (s: SectionNode) => {
    const n = stats(s).count
    if (n === 0 && !s.children.some((c) => c.kind === 'section')) {
      const before = snapshot()
      deleteSection(s.id)
      toast(`已刪除「${s.title}」`, { action: { label: '復原', onClick: () => restore(before) } })
    } else setDeleting(s)
  }

  const lines: React.ReactNode[] = []
  const walk = (nodes: Node[], depth: number, parent: string | null) => {
    nodes.forEach((n, index) => {
      if (n.kind === 'project') {
        const row = byId.get(n.id)
        if (!row) return // its folders now belong to another project, or it has no conversations
        lines.push(
          <ProjectLine
            key={`p:${n.id}`}
            row={row}
            name={name(n.id)}
            where={where(row)}
            depth={depth}
            drop={{ 'data-drop': 'placed', 'data-id': n.id, 'data-parent': parent ?? '', 'data-index': String(index), 'data-depth': String(depth) }}
            dragging={api.drag?.id === n.id}
            onPointerDown={api.onPointerDown}
            suppressClick={api.suppressClick}
          />,
        )
        return
      }
      const x = stats(n)
      lines.push(
        <SectionLine key={`s:${n.id}`} s={n} depth={depth} parent={parent} index={index} count={x.count} chats={x.chats} last={x.last} editing={editing === n.id} setEditing={setEditing} api={api} onDelete={() => askDelete(n)} />,
      )
      if (n.collapsed) return
      if (n.children.length === 0)
        lines.push(
          <div key={`e:${n.id}`} data-drop="empty" data-section={n.id} className="py-1 pr-4" style={{ paddingLeft: 20 + (depth + 1) * INDENT }}>
            <div className={cn('flex h-8 items-center rounded-md border border-dashed px-3 text-[12px] text-muted-foreground', api.target?.into === n.id && 'border-brand bg-brand-soft text-foreground')}>
              空的區塊。把專案拖進來，或用專案的 ⋯ →「移到區塊」
            </div>
          </div>,
        )
      else walk(n.children, depth + 1, n.id)
    })
  }
  walk(root, 0, null)

  return (
    <Card
      title="釘選"
      count={pinnedCount}
      actions={
        <>
          <Button variant="ghost" size="sm" className="text-[12.5px]" onClick={() => setEditing(createSection(null))}>
            <Plus />
            區塊
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="size-8" aria-label="釘選的選項">
                <Ellipsis />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onSelect={() => setAllCollapsed(true)}>全部收合</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => setAllCollapsed(false)}>全部展開</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onSelect={() => {
                  const before = snapshot()
                  resetArrangement()
                  toast('已恢復預設的釘選', { action: { label: '復原', onClick: () => restore(before) } })
                }}
              >
                恢復預設的釘選
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </>
      }
    >
      <ColumnHead />
      <div role="rowgroup" aria-label="釘選" data-drop="pinned-root" className={cn('flex flex-col px-0 pt-1 pb-2', api.target?.into === '@pinned' && 'bg-brand-soft/60')}>
        {lines.length > 0 ? (
          lines
        ) : (
          <div className={cn('mx-5 my-2 flex h-16 items-center justify-center gap-2 rounded-lg border border-dashed text-[13px] text-muted-foreground', api.target?.into === '@pinned' && 'border-brand bg-brand-soft text-foreground')}>
            <Rows3 className="size-4" />
            把常用的專案拖到這裡，或在專案的 ⋯ 選「釘選」
          </div>
        )}
      </div>
      <AlertDialog open={!!deleting} onOpenChange={(o) => !o && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>刪除「{deleting?.title}」區塊？</AlertDialogTitle>
            <AlertDialogDescription>裡面的 {deleting ? num(stats(deleting).count) : 0} 個專案會回到「其他專案」，子區塊也一起拿掉。專案和對話都不會刪除。</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!deleting) return
                const before = snapshot()
                deleteSection(deleting.id)
                toast(`已刪除「${deleting.title}」`, { action: { label: '復原', onClick: () => restore(before) } })
                setDeleting(null)
              }}
            >
              刪除區塊
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  )
}

// ------------------------------------------------------------- 其他專案

type Status = 'all' | 'recent' | 'gone'
type OtherSort = 'recent' | 'chats' | 'name'
const sortLabel: Record<OtherSort, string> = { recent: '最近活動', chats: '對話數', name: '名稱' }

function OthersCard({ api, root }: { api: DragApi; root: Node[] }) {
  const { list, name, where, edits } = useProjectRows()
  const [, setParams] = useSearchParams()
  const [q, setQ] = useState('')
  const [tools, setTools] = useState<HarnessId[]>([])
  const [status, setStatus] = useState<Status>('all')
  const [sort, setSort] = useState<OtherSort>('recent')
  const [showAll, setShowAll] = useState(false)
  const needle = q.trim().toLowerCase()
  const placed = useMemo(() => placedIds(root), [root])
  const others = useMemo(() => list.filter((r) => !placed.has(r.project.id)), [list, placed])
  const filtered = useMemo(() => {
    const out = others.filter(
      (r) =>
        matches(r, name(r.project.id), needle) &&
        (tools.length === 0 || r.harnesses.some((h) => tools.includes(h))) &&
        (status === 'all' || (status === 'recent' ? r.lastActive > NOW - 30 * DAY : r.project.gone)),
    )
    const by: Record<OtherSort, (a: Row, b: Row) => number> = {
      recent: (a, b) => b.lastActive - a.lastActive,
      chats: (a, b) => b.chatCount - a.chatCount || b.lastActive - a.lastActive,
      name: (a, b) => name(a.project.id).localeCompare(name(b.project.id), 'zh-Hant'),
    }
    return out.sort(by[sort])
  }, [others, name, needle, tools, status, sort])
  const narrowed = !!needle || tools.length > 0 || status !== 'all'
  const shown = narrowed || showAll ? filtered : filtered.slice(0, LATEST)
  const { ref, v, margin } = usePageList(shown.length)
  const suggestions = openSuggestions(edits).length

  return (
    <Card
      title="其他專案"
      count={others.length}
      data-drop="other"
      className={cn(api.target?.other && 'ring-2 ring-brand/50')}
      actions={
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative w-[220px]">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="找名稱、資料夾或遠端" aria-label="在其他專案裡找" className="h-8 pr-7 pl-8 text-[13px]" />
            {q && (
              <button type="button" aria-label="清除搜尋" onClick={() => setQ('')} className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground">
                <X className="size-3.5" />
              </button>
            )}
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm" className={cn('h-8 text-[12.5px]', tools.length > 0 && 'border-brand/40 bg-brand-soft')}>
                {tools.length === 0 ? '所有工具' : tools.map((h) => harnesses[h].name).join('、')}
                <ChevronDown className="opacity-60" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuLabel>用過這些工具的專案</DropdownMenuLabel>
              {harnessOrder.map((h) => (
                <DropdownMenuCheckboxItem
                  key={h}
                  checked={tools.includes(h)}
                  onSelect={(e) => e.preventDefault()}
                  onCheckedChange={() => setTools((t) => (t.includes(h) ? t.filter((x) => x !== h) : [...t, h]))}
                >
                  <HarnessIcon id={h} />
                  {harnesses[h].name}
                </DropdownMenuCheckboxItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <ToggleGroup type="single" variant="outline" size="sm" value={status} onValueChange={(v) => v && setStatus(v as Status)} aria-label="專案狀態">
            <ToggleGroupItem value="all" className="px-2.5 text-[12.5px]">
              全部
            </ToggleGroupItem>
            <ToggleGroupItem value="recent" className="px-2.5 text-[12.5px]">
              30 天內有活動
            </ToggleGroupItem>
            <ToggleGroupItem value="gone" className="px-2.5 text-[12.5px]">
              資料夾已不在
            </ToggleGroupItem>
          </ToggleGroup>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" className="h-8 text-[12.5px]" aria-label={`排序：${sortLabel[sort]}`}>
                <SortDesc />
                {sortLabel[sort]}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuRadioGroup value={sort} onValueChange={(v) => setSort(v as OtherSort)}>
                {(Object.keys(sortLabel) as OtherSort[]).map((k) => (
                  <DropdownMenuRadioItem key={k} value={k}>
                    {sortLabel[k]}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      }
    >
      {suggestions > 0 && (
        <button
          type="button"
          onClick={() => setParams((p) => (p.set('review', '1'), p), { replace: true })}
          className="mx-5 mb-2 flex items-center gap-2 rounded-lg bg-muted/60 px-3 py-2 text-left text-[12.5px] text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <FolderSearch className="size-4 shrink-0" />
          {folderWords.suggestions(suggestions)}
          <span className="ml-auto text-brand">檢視</span>
        </button>
      )}
      <ColumnHead />
      {shown.length === 0 ? (
        <p className="px-5 py-8 text-center text-[13px] text-muted-foreground">沒有符合條件的專案。{narrowed && '試試清除搜尋或篩選。'}</p>
      ) : (
        <div ref={ref} role="rowgroup" aria-label="其他專案" className="relative pt-1" style={{ height: v.getTotalSize() + 4 }}>
          {v.getVirtualItems().map((it) => {
            const row = shown[it.index]
            return (
              <div key={row.project.id} className="absolute inset-x-0 top-0" style={{ transform: `translateY(${it.start - margin + 4}px)` }}>
                <ProjectLine row={row} name={name(row.project.id)} where={where(row)} depth={0} dragging={api.drag?.id === row.project.id} onPointerDown={api.onPointerDown} suppressClick={api.suppressClick} />
              </div>
            )
          })}
        </div>
      )}
      <div className="flex items-center gap-3 border-t px-5 py-2.5 text-[12.5px] text-muted-foreground">
        {narrowed ? (
          <span>
            符合的有 {num(filtered.length)} 個
          </span>
        ) : showAll ? (
          <button type="button" onClick={() => setShowAll(false)} className="hover:text-foreground">
            只看最近 {LATEST} 個
          </button>
        ) : (
          filtered.length > LATEST && (
            <button type="button" onClick={() => setShowAll(true)} className="text-brand hover:underline">
              顯示全部 {num(filtered.length)} 個
            </button>
          )
        )}
        <span className="ml-auto">新出現的專案會先放在這裡；拖到「釘選」就會留在上面。</span>
      </div>
    </Card>
  )
}

// ------------------------------------------------------------------ view

export function DefaultView() {
  const root = useArrangement()
  const api = useDrag(root)
  return (
    <div ref={api.wrapRef} className="flex flex-col gap-4">
      <PinnedCard api={api} root={root} />
      <OthersCard api={api} root={root} />
      {api.drag && api.target?.line && (
        <div aria-hidden className="pointer-events-none fixed z-50 h-0.5 rounded-full bg-brand" style={{ top: api.target.line.top - 1, left: api.target.line.left, width: api.target.line.width }}>
          <span className="absolute -top-[3px] -left-1 size-2 rounded-full border-2 border-brand bg-background" />
        </div>
      )}
      {api.drag && (
        <div
          ref={api.ghostRef}
          aria-hidden
          className="pointer-events-none fixed top-0 left-0 z-50 flex items-center gap-1.5 rounded-lg border bg-popover px-2.5 py-1.5 text-[13px] font-medium shadow-float"
          style={{ transform: `translate(${api.pointer.current.x + 14}px, ${api.pointer.current.y + 10}px)` }}
        >
          {api.drag.kind === 'section' && <Rows3 className="size-4 text-muted-foreground" />}
          {api.drag.label}
          {api.target === null ? (
            <span className="text-xs font-normal text-muted-foreground">· 不能放在這裡</span>
          ) : api.target.other ? (
            <span className="text-xs font-normal text-muted-foreground">· 取消釘選</span>
          ) : (
            <Check className="size-3.5 text-brand" />
          )}
        </div>
      )}
    </div>
  )
}
