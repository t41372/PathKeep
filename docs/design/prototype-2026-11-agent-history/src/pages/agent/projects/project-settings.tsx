/**
 * Project settings: the name and the folders, opened from 「專案設定…」 in any
 * project menu and from the button in a project's header. It is not called
 * 「資料夾」, which reads like "open this project's folder" (that is 「在 Finder
 * 顯示」).
 *
 * A PathKeep project can hold several folders (main folder, worktrees,
 * subfolders, an old location, or any folder you add). Adding a folder that
 * showed up as its own project moves its conversations here; moving it out
 * makes it its own project again. Only PathKeep's name and grouping change;
 * the tools' folders are never touched.
 */
import { FolderGit2, FolderInput, FolderOutput, FolderTree, GitBranch, Hourglass, MoveRight, Plus } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList } from '@/components/ui/command'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { cn } from '@/lib/cn'
import { ProjectIcon } from '~/components/bits'
import { archive, projectsByRecent } from '~/data/archive'
import { num } from '~/data/format'
import { folderWords } from '~/data/labels'
import type { Location, Project } from '~/data/types'
import { closeProjectDialog, openProjectDialog, useProjectDialog } from './dialogs'
import { SuggestionCard } from './suggestions-sheet'
import { addFoldersTo, displayName, joinedInto, openSuggestions, rename, useProjectEdits } from './store'
import { dirOf } from './use-rows'

const roleLabel: Record<Location['role'], string> = {
  main: '主資料夾',
  worktree: 'worktree',
  subfolder: '子資料夾',
  former: '搬家前的位置',
  temp: '暫存',
}

const roleIcon = { main: FolderGit2, worktree: GitBranch, subfolder: FolderTree, former: MoveRight, temp: Hourglass } as const

function reason(project: Project, loc: Location): string {
  if (project.kind === 'scratch') return loc.path === '~' ? '家目錄：在這裡跑的對話不屬於任何專案' : '系統暫存資料夾，用完就會被清掉'
  switch (loc.role) {
    case 'main':
      return project.kind === 'repo' ? 'git 根目錄' : '不是 git repo，只看這個資料夾'
    case 'worktree':
      return loc.path.includes('/.codex/')
        ? `Codex 建的 worktree，它記下的 git 遠端相同${loc.branch ? `；分支 ${loc.branch}` : ''}`
        : `Claude Code 在這個 repo 裡建的 worktree${loc.branch ? `；分支 ${loc.branch}` : ''}`
    case 'subfolder':
      return '在 git 根目錄裡面，所以自動算進來'
    case 'former':
      return '資料夾搬過家；備份時讀到的 git 遠端和現在的位置相同'
    default:
      return ''
  }
}

/** Folders inside the main one read better relative to it. */
function shown(project: Project, loc: Location) {
  const root = project.locations[0].path
  return loc.role !== 'main' && loc.path.startsWith(root + '/') ? '.' + loc.path.slice(root.length) : loc.path
}

function FolderRow({ project, loc, addedFrom }: { project: Project; loc: Location; addedFrom?: Project }) {
  const Icon = addedFrom ? FolderInput : roleIcon[loc.role]
  const removable = loc.role !== 'main' || !!addedFrom
  const moveOut = () => {
    if (addedFrom) {
      addFoldersTo(addedFrom.id, null)
      toast(`已把 ${loc.path} 移出，它又是獨立的專案了`, { action: { label: '復原', onClick: () => addFoldersTo(addedFrom.id, project.id) } })
      return
    }
    toast(`${loc.path} 會變成獨立的專案`, { description: `${num(loc.chatCount)} 段對話跟著過去，之後能再加回來。（原型只示意）` })
  }
  return (
    <div className="group flex items-start gap-3 rounded-lg px-2 py-2 hover:bg-foreground/[0.03]">
      <Icon className={cn('mt-0.5 size-4 shrink-0', loc.exists ? 'text-muted-foreground' : 'text-muted-foreground/50')} />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex min-w-0 items-center gap-2">
          <span title={loc.path} className={cn('truncate font-mono text-[12.5px]', !loc.exists && 'text-muted-foreground')}>
            {addedFrom ? loc.path : shown(project, loc)}
          </span>
          <span className="shrink-0 rounded-md bg-muted px-1.5 py-px text-[11px] text-muted-foreground">{addedFrom ? '你加進來的' : roleLabel[loc.role]}</span>
          {!loc.exists && <span className="shrink-0 text-[11px] text-muted-foreground">已刪除</span>}
        </div>
        <span className="text-xs text-muted-foreground">{addedFrom ? `原本單獨算一個專案（${addedFrom.name}）` : reason(project, loc)}</span>
      </div>
      <span className="shrink-0 pt-0.5 text-xs text-muted-foreground tabular">{loc.chatCount ? `${num(loc.chatCount)} 段` : '沒有對話'}</span>
      {removable ? (
        <Button size="xs" variant="ghost" onClick={moveOut} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" aria-label={`把 ${loc.path} ${folderWords.remove}`}>
          <FolderOutput />
          移出
        </Button>
      ) : (
        <span className="w-[52px] shrink-0" />
      )}
    </div>
  )
}

/** The name field: alone in the 改名 dialog (focused, closes on save), or the first section of project settings. */
function RenameField({ project, inSettings }: { project: Project; inSettings?: boolean }) {
  const edits = useProjectEdits()
  const current = displayName(edits, project.id)
  const [value, setValue] = useState(current)
  const changed = value.trim() !== current
  return (
    <form
      className="flex flex-col gap-1.5"
      onSubmit={(e) => {
        e.preventDefault()
        rename(project.id, value.trim() === project.name ? '' : value)
        toast('名稱已更新')
        if (!inSettings) closeProjectDialog()
      }}
    >
      <label htmlFor={inSettings ? 'project-settings-name' : 'project-name'} className={inSettings ? 'text-[13px] font-semibold' : 'text-[12px] font-medium text-muted-foreground'}>
        名稱
      </label>
      <div className="flex gap-2">
        <Input id={inSettings ? 'project-settings-name' : 'project-name'} autoFocus={!inSettings} value={value} onChange={(e) => setValue(e.target.value)} className="h-8 text-[13px]" />
        <Button type="submit" size="sm" disabled={!changed}>
          儲存
        </Button>
      </div>
      <span className="text-[12px] text-muted-foreground">只改 PathKeep 裡顯示的名字，資料夾名稱不會變。留空就用回「{project.name}」。</span>
    </form>
  )
}

function FoldersSection({ project }: { project: Project }) {
  const edits = useProjectEdits()
  const joined = joinedInto(edits)
  const added = [...joined.entries()].filter(([, into]) => into === project.id).map(([from]) => archive.projectById.get(from)!)
  const mine = openSuggestions(edits).filter((s) => s.intoId === project.id)
  const total = project.locations.length + added.reduce((s, p) => s + p.locations.length, 0)
  const gone = project.locations.filter((l) => !l.exists).length
  return (
    <section aria-labelledby="project-settings-folders" className="flex flex-col gap-5">
      <div className="flex flex-col gap-1">
        <div className="flex items-center justify-between gap-2">
          <h3 id="project-settings-folders" className="text-[13px] font-semibold">
            資料夾
            <span className="ml-1.5 font-normal text-muted-foreground tabular">
              {num(total)} 個{gone ? `，${gone} 個已不在磁碟上` : ''}
            </span>
          </h3>
          {project.kind !== 'scratch' && (
            <Button size="sm" variant="outline" onClick={() => openProjectDialog('add-folder', project.id)}>
              <Plus />
              {folderWords.add}
            </Button>
          )}
        </div>
        <p className="text-[12px] text-muted-foreground">一個專案可以包含好幾個資料夾，這些資料夾裡的對話都算在這個專案。</p>
        <div className="-mx-2 mt-1 flex flex-col">
          {project.locations.map((loc) => (
            <FolderRow key={loc.path} project={project} loc={loc} />
          ))}
          {added.flatMap((p) => p.locations.map((loc) => <FolderRow key={`${p.id}-${loc.path}`} project={project} loc={loc} addedFrom={p} />))}
        </div>
      </div>
      {mine.length > 0 && (
        <div className="flex flex-col gap-2">
          <span className="text-[12px] font-medium text-muted-foreground">可能也屬於這個專案</span>
          {mine.map((s) => (
            <SuggestionCard key={s.id} s={s} decision={edits.decisions.get(s.id)} />
          ))}
        </div>
      )}
    </section>
  )
}

function AddFolderDialog({ project, open, onOpenChange }: { project: Project; open: boolean; onOpenChange: (v: boolean) => void }) {
  const edits = useProjectEdits()
  const joined = joinedInto(edits)
  const into = displayName(edits, project.id)
  const candidates = projectsByRecent.filter((p) => p.id !== project.id && !joined.has(p.id) && p.kind !== 'scratch' && ![...joined.values()].includes(p.id))
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-3 p-0 sm:max-w-[520px]">
        <DialogHeader className="px-5 pt-5">
          <DialogTitle>{folderWords.addFrom(into)}</DialogTitle>
          <DialogDescription>選一個目前單獨算一個專案的資料夾。它的對話會算進「{into}」；之後能在專案設定裡移出。</DialogDescription>
        </DialogHeader>
        <Command className="border-t">
          <CommandInput placeholder="搜尋資料夾或專案名稱" />
          <CommandList className="max-h-[340px]">
            <CommandEmpty>沒有符合的資料夾</CommandEmpty>
            {candidates.map((p) => (
              <CommandItem
                key={p.id}
                value={`${displayName(edits, p.id)} ${p.locations[0].path}`}
                onSelect={() => {
                  addFoldersTo(p.id, project.id)
                  onOpenChange(false)
                  toast(`已把 ${p.locations[0].path} 加到「${into}」`, {
                    description: `${num(p.chatIds.length)} 段對話跟著過來`,
                    action: { label: '復原', onClick: () => addFoldersTo(p.id, null) },
                  })
                }}
              >
                <ProjectIcon project={p} />
                <span className="shrink-0">{displayName(edits, p.id)}</span>
                <span className="min-w-0 truncate font-mono text-[11.5px] text-muted-foreground">{dirOf(p.locations[0].path)}</span>
                <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">{num(p.chatIds.length)} 段</span>
              </CommandItem>
            ))}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  )
}

/** Renders whichever project dialog is open. Mount one per page. */
export function ProjectDialogs() {
  const open = useProjectDialog()
  const edits = useProjectEdits()
  const project = open ? archive.projectById.get(open.id) : undefined
  const name = project ? displayName(edits, project.id) : ''
  return (
    <>
      <Sheet open={open?.kind === 'settings' || open?.kind === 'add-folder'} onOpenChange={(o) => !o && closeProjectDialog()}>
        <SheetContent className="w-[580px] sm:max-w-[580px]" onOpenAutoFocus={(e) => e.preventDefault()}>
          {project && (
            <>
              <SheetHeader>
                <SheetTitle>專案設定</SheetTitle>
                <SheetDescription className="flex min-w-0 items-center gap-1.5">
                  <ProjectIcon project={project} />
                  <span className="shrink-0 text-foreground">{name}</span>
                  <span className="min-w-0 truncate font-mono text-[11.5px]">{project.locations[0].path}</span>
                </SheetDescription>
              </SheetHeader>
              <div className="flex flex-col gap-7 overflow-y-auto px-4 pb-6">
                <RenameField key={project.id} project={project} inSettings />
                <FoldersSection project={project} />
                <p className="text-[12px] text-muted-foreground">這裡只改 PathKeep 裡的名稱和分組，不動 Claude Code、Codex 等工具自己的資料夾和檔案。</p>
              </div>
            </>
          )}
        </SheetContent>
      </Sheet>
      {project && <AddFolderDialog project={project} open={open?.kind === 'add-folder'} onOpenChange={(o) => (o ? undefined : openProjectDialog('settings', project.id))} />}
      <Dialog open={open?.kind === 'rename'} onOpenChange={(o) => !o && closeProjectDialog()}>
        <DialogContent className="sm:max-w-[440px]">
          <DialogHeader>
            <DialogTitle>改名</DialogTitle>
            <DialogDescription className="font-mono text-[12px]">{project?.locations[0].path}</DialogDescription>
          </DialogHeader>
          {project && <RenameField key={project.id} project={project} />}
        </DialogContent>
      </Dialog>
    </>
  )
}

