/**
 * The one menu for a project, used by every row on the projects page (⋯ and
 * right-click) and by the project's own header: open, insights, rename,
 * pin / move to a section, project settings (name and folders), Finder.
 * Managing a project never requires opening its conversations first.
 */
import { ChartColumn, Ellipsis, FolderSearch, MessagesSquare, Pencil, Pin, PinOff, Settings2 } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/cn'
import { archive } from '~/data/archive'
import { folderWords } from '~/data/labels'
import { findParent, move, restore, snapshot, useArrangement } from './arrangement'
import { openProjectDialog } from './dialogs'
import { MoveToSubmenu } from './move-menu'

export function pin(id: string, label: string) {
  const before = snapshot()
  if (move(id, 'project', { parent: null, index: snapshot().length })) toast(`已釘選「${label}」`, { action: { label: '復原', onClick: () => restore(before) } })
}

export function unpin(id: string, label: string) {
  const before = snapshot()
  if (move(id, 'project', null)) toast(`已取消釘選「${label}」，它回到「其他專案」`, { action: { label: '復原', onClick: () => restore(before) } })
}

export function ProjectMenuItems({ id, label, inProject }: { id: string; label: string; inProject?: boolean }) {
  const navigate = useNavigate()
  const root = useArrangement()
  const placed = !!findParent(root, id)
  const project = archive.projectById.get(id)
  return (
    <>
      {!inProject && (
        <DropdownMenuItem onSelect={() => void navigate(`/agent/projects/${id}`)}>
          <MessagesSquare />
          打開對話
        </DropdownMenuItem>
      )}
      <DropdownMenuItem onSelect={() => void navigate(`/agent/projects/${id}/insights`)}>
        <ChartColumn />
        洞察
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => openProjectDialog('rename', id)}>
        <Pencil />
        改名…
      </DropdownMenuItem>
      {placed ? (
        <DropdownMenuItem onSelect={() => unpin(id, label)}>
          <PinOff />
          取消釘選
        </DropdownMenuItem>
      ) : (
        <DropdownMenuItem onSelect={() => pin(id, label)}>
          <Pin />
          釘選
        </DropdownMenuItem>
      )}
      <MoveToSubmenu id={id} kind="project" label={label} trigger="移到區塊…" />
      <DropdownMenuItem onSelect={() => openProjectDialog('settings', id)}>
        <Settings2 />
        {folderWords.settings}…
      </DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem disabled={!project || project.gone} onSelect={() => toast('已在 Finder 顯示')}>
        <FolderSearch />
        在 Finder 顯示
      </DropdownMenuItem>
    </>
  )
}

/** The ⋯ button on a row; the row opens the same menu on right-click through `open`. */
export function ProjectRowMenu({ id, label, open, onOpenChange }: { id: string; label: string; open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          data-no-drag
          aria-label={`「${label}」的選項`}
          onClick={(e) => e.stopPropagation()}
          className={cn(
            'flex size-7 items-center justify-center rounded-md text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100',
            open && 'opacity-100',
          )}
        >
          <Ellipsis className="size-4" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[210px]" onClick={(e) => e.stopPropagation()}>
        <ProjectMenuItems id={id} label={label} />
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
