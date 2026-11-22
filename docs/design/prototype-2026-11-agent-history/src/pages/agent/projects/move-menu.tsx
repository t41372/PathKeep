/**
 * The keyboard and menu path for arranging 釘選: 移到… lists the top of
 * 釘選, every section (indented) and 其他專案. Used by each row's ⋯ menu
 * and by the project header, so nothing needs drag.
 */
import { CornerDownRight, Inbox, Pin, Rows3 } from 'lucide-react'
import { toast } from 'sonner'
import {
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from '@/components/ui/dropdown-menu'
import { contains, findParent, findSection, move, restore, sectionTree, snapshot, useArrangement, type Node } from './arrangement'

export function moveWithUndo(id: string, kind: Node['kind'], parent: string | null | 'other', label: string, where: string) {
  const before = snapshot()
  const root = snapshot()
  const index = parent === null ? root.length : parent === 'other' ? 0 : (findSection(root, parent)?.children.length ?? 0)
  const ok = move(id, kind, parent === 'other' ? null : { parent, index })
  if (ok) toast(`已把「${label}」移到${where}`, { action: { label: '復原', onClick: () => restore(before) } })
}

export function MoveToSubmenu({ id, kind, label, trigger = '移到…' }: { id: string; kind: Node['kind']; label: string; trigger?: string }) {
  const root = useArrangement()
  const current = findParent(root, id)
  const sections = sectionTree(root)
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>
        <CornerDownRight />
        {trigger}
      </DropdownMenuSubTrigger>
      <DropdownMenuSubContent className="max-h-[360px] w-[240px] overflow-y-auto">
        <DropdownMenuItem disabled={!!current && current.parent === null} onSelect={() => moveWithUndo(id, kind, null, label, '「釘選」最上層')}>
          <Pin />
          釘選（最上層）
        </DropdownMenuItem>
        {sections.map((s) => {
          const self = kind === 'section' && contains(root, id, s.id)
          return (
            <DropdownMenuItem
              key={s.id}
              disabled={self || current?.parent === s.id}
              style={{ paddingLeft: 8 + (s.depth + 1) * 14 }}
              onSelect={() => moveWithUndo(id, kind, s.id, label, `「${s.title}」`)}
            >
              <Rows3 />
              <span className="truncate">{s.title}</span>
            </DropdownMenuItem>
          )
        })}
        {kind === 'project' && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem disabled={!current} onSelect={() => moveWithUndo(id, kind, 'other', label, '「其他專案」')}>
              <Inbox />
              其他專案（取消釘選）
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuSubContent>
    </DropdownMenuSub>
  )
}
