/**
 * The default view's Pinned card (`agent-history.md` §5.2): the person's
 * own sections, nested and collapsible, and pinned projects, in their
 * order. Sections are made with "+ Section" (at the top, named at once)
 * or "Add section inside", renamed in place (F2), collapsed with a click,
 * Enter or Space, moved with the menu or by dragging, and deleted (asking
 * first when something is inside). It starts empty.
 */
import { ChevronRight, Ellipsis, Plus } from 'lucide-react'
import { useCallback, useRef, useState, type ReactNode } from 'react'
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { ArrangementNode } from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { nudge, pinnedRows, projectsInside, type TreeNode } from './arrangement'
import { DropArea } from './pinned-drag'
import { MoveToMenu } from './project-menu'
import { ProjectRow } from './project-row'
import { gridOf } from './project-words'
import { lastActive } from './project-words'
import { useProjectsShared } from './projects-context'
import { useDragState, useDropRow, useGrip } from './use-pinned-drag'
import { useProjectEdit } from './use-project-edit'

function DropLine({ id }: { id: string }) {
  const { hint } = useDragState()
  if (!hint || hint.over !== id || !hint.ok || hint.zone === 'into') return null
  return (
    <span
      aria-hidden
      className={cn(
        'pointer-events-none absolute right-2 left-2 h-0.5 rounded-full bg-brand',
        hint.zone === 'before' ? '-top-px' : '-bottom-px',
      )}
    />
  )
}

/** One element both dragged and dropped on; stable, so a drag is not dropped mid-way. */
function useBoth(
  dragRef: (element: Element | null) => void,
  dropRef: (element: Element | null) => void,
) {
  return useCallback(
    (element: Element | null) => {
      dragRef(element)
      dropRef(element)
    },
    [dragRef, dropRef],
  )
}

function PinnedProject({ entry, depth }: { entry: TreeNode; depth: number }) {
  const shared = useProjectsShared()
  const { ref: dragRef, grip, isDragging } = useGrip(entry.node.id, 'project')
  const { ref: dropRef } = useDropRow(entry.node.id, false)
  const ref = useBoth(dragRef, dropRef)
  const card = shared.cards.get(entry.node.id)
  if (!card) return null
  return (
    <div ref={ref} className={cn('relative', isDragging && 'opacity-40')}>
      <ProjectRow project={card} depth={depth} grip={grip} />
      <DropLine id={entry.node.id} />
    </div>
  )
}

function SectionName({
  node,
  renaming,
  onDone,
}: {
  node: ArrangementNode
  renaming: boolean
  onDone: () => void
}) {
  const { t } = useI18n()
  const edit = useProjectEdit()
  const [draft, setDraft] = useState(node.name ?? '')
  // Enter saves and the field goes away; the blur that may follow does not save again.
  const done = useRef(false)
  if (!renaming)
    return <span className="truncate font-medium">{node.name}</span>
  const finish = (save: boolean) => {
    if (done.current) return
    done.current = true
    onDone()
    const name = draft.trim()
    if (save && name && name !== node.name)
      void edit({ kind: 'renameSection', id: node.id, name })
  }
  return (
    <input
      // Named at once: a new section, or F2.
      autoFocus
      aria-label={t('agentProjects.section.nameLabel')}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onFocus={(event) => event.currentTarget.select()}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.nativeEvent.isComposing) return
        if (event.key === 'Enter') finish(true)
        else if (event.key === 'Escape') finish(false)
      }}
      onBlur={() => finish(true)}
      className="h-7 min-w-0 flex-1 rounded-md border bg-background px-2 text-[13px] font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
    />
  )
}

function SectionRow({
  entry,
  depth,
  renaming,
  onRename,
  onRenamed,
  onDelete,
}: {
  entry: TreeNode
  depth: number
  renaming: boolean
  onRename: (id: string) => void
  onRenamed: () => void
  onDelete: (node: ArrangementNode) => void
}) {
  const { t } = useI18n()
  const shared = useProjectsShared()
  const edit = useProjectEdit()
  const [menu, setMenu] = useState(false)
  const node = entry.node
  const { ref: dragRef, grip, isDragging } = useGrip(node.id, 'section')
  const { ref: dropRef } = useDropRow(node.id, true)
  const ref = useBoth(dragRef, dropRef)
  const { hint, justDropped } = useDragState()
  const into = hint?.over === node.id && hint.zone === 'into' && hint.ok
  const toggle = () =>
    void edit({ kind: 'collapse', id: node.id, collapsed: !node.collapsed })
  const up = nudge(shared.arrangement, node.id, -1)
  const down = nudge(shared.arrangement, node.id, 1)
  return (
    <div ref={ref} className={cn('relative', isDragging && 'opacity-40')}>
      <div
        role="button"
        tabIndex={0}
        data-section-id={node.id}
        aria-expanded={!node.collapsed}
        onClick={(event) => {
          if (justDropped()) return
          if ((event.target as HTMLElement).closest('button, input')) return
          toggle()
        }}
        onKeyDown={(event) => {
          if (event.target !== event.currentTarget) return
          if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault()
            toggle()
          } else if (event.key === 'F2') {
            event.preventDefault()
            onRename(node.id)
          }
        }}
        onContextMenu={(event) => {
          event.preventDefault()
          setMenu(true)
        }}
        className={cn(
          gridOf(false),
          'group h-10 cursor-pointer rounded-md px-2 text-[13px] outline-none transition-colors duration-150 hover:bg-muted/60 focus-visible:ring-2 focus-visible:ring-ring/50',
          into && 'bg-brand-soft',
        )}
      >
        <div
          className="flex min-w-0 items-center gap-2"
          style={{ paddingLeft: depth * 16 }}
        >
          {grip}
          <ChevronRight
            className={cn(
              'size-4 shrink-0 text-muted-foreground transition-transform duration-200',
              !node.collapsed && 'rotate-90',
            )}
            aria-hidden
          />
          <SectionName
            key={String(renaming)}
            node={node}
            renaming={renaming}
            onDone={onRenamed}
          />
          <span className="shrink-0 text-[12px] text-muted-foreground tabular">
            {node.projectCount}
          </span>
        </div>
        <span />
        <span className="text-right text-muted-foreground/70 tabular">
          {shared.f.number(node.conversationCount)}
        </span>
        <span className="text-right text-[12px] text-muted-foreground">
          {node.projectCount > 0
            ? lastActive(t, node.lastActivity, shared.zone)
            : ''}
        </span>
        <DropdownMenu open={menu} onOpenChange={setMenu}>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label={t('agentProjects.section.menu', {
                name: node.name ?? '',
              })}
              className="flex size-7 items-center justify-center rounded-md text-muted-foreground opacity-0 transition-opacity duration-150 group-hover:opacity-100 hover:bg-muted hover:text-foreground focus-visible:opacity-100 data-[state=open]:opacity-100"
            >
              <Ellipsis className="size-4" aria-hidden />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-[220px]">
            <DropdownMenuItem
              onSelect={() =>
                void edit({
                  kind: 'createSection',
                  parent: node.id,
                  name: t('agentProjects.section.newName'),
                }).then((result) => {
                  if (result?.createdId) onRename(result.createdId)
                })
              }
            >
              {t('agentProjects.section.addInside')}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onRename(node.id)}>
              {t('agentProjects.section.rename')}
            </DropdownMenuItem>
            <MoveToMenu
              id={node.id}
              label={node.name ?? ''}
              kind="section"
              arrangement={shared.arrangement}
            />
            <DropdownMenuItem
              disabled={!up}
              onSelect={() => up && void edit(up)}
            >
              {t('agentProjects.section.up')}
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled={!down}
              onSelect={() => down && void edit(down)}
            >
              {t('agentProjects.section.down')}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onSelect={() => onDelete(node)}
            >
              {t('agentProjects.section.delete')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <DropLine id={node.id} />
    </div>
  )
}

export function PinnedCard() {
  const { t } = useI18n()
  const shared = useProjectsShared()
  const edit = useProjectEdit()
  const [renaming, setRenaming] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<ArrangementNode | null>(null)
  const arrangement = shared.arrangement
  const rows = pinnedRows(arrangement)
  const count = arrangement.pinned.size

  const remove = (node: ArrangementNode) =>
    void edit(
      { kind: 'deleteSection', id: node.id },
      {
        title: t('agentProjects.toast.sectionDeleted', {
          name: node.name ?? '',
        }),
      },
    )
  const askDelete = (node: ArrangementNode) => {
    const entry = arrangement.byId.get(node.id)
    if (projectsInside(arrangement, node.id) === 0 && !entry?.children.length)
      remove(node)
    else setDeleting(node)
  }

  const body: ReactNode[] = []
  for (const { entry, depth } of rows) {
    if (entry.node.kind === 'section') {
      body.push(
        <SectionRow
          key={entry.node.id}
          entry={entry}
          depth={depth}
          renaming={renaming === entry.node.id}
          onRename={setRenaming}
          onRenamed={() => setRenaming(null)}
          onDelete={askDelete}
        />,
      )
      if (!entry.node.collapsed && entry.children.length === 0)
        body.push(
          <DropArea
            key={`${entry.node.id}-empty`}
            id={`into:${entry.node.id}`}
            over={`into:${entry.node.id}`}
            className="rounded-md"
          >
            <p
              className="py-2 pr-2 text-[12.5px] text-muted-foreground"
              style={{ paddingLeft: 40 + (depth + 1) * 16 }}
            >
              {t('agentProjects.pinned.emptySection')}
            </p>
          </DropArea>,
        )
    } else
      body.push(
        <PinnedProject key={entry.node.id} entry={entry} depth={depth} />,
      )
  }

  return (
    <section
      aria-labelledby="pinned-title"
      className="rounded-xl border bg-card shadow-card"
    >
      <div className="flex items-center gap-2 border-b px-4 py-2.5">
        <h2 id="pinned-title" className="text-[14px] font-semibold">
          {t('agentProjects.pinned.title')}
        </h2>
        <span className="text-[12.5px] text-muted-foreground tabular">
          {count}
        </span>
        <button
          type="button"
          onClick={() =>
            void edit({
              kind: 'createSection',
              parent: null,
              name: t('agentProjects.section.newName'),
            }).then((result) => {
              if (result?.createdId) setRenaming(result.createdId)
            })
          }
          className="ml-auto flex h-7 items-center gap-1 rounded-md px-2 text-[12.5px] text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <Plus className="size-3.5" aria-hidden />
          {t('agentProjects.pinned.addSection')}
        </button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label={t('agentProjects.pinned.menu')}
              className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
            >
              <Ellipsis className="size-4" aria-hidden />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onSelect={() => void edit({ kind: 'collapse', collapsed: true })}
            >
              {t('agentProjects.pinned.collapseAll')}
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() => void edit({ kind: 'collapse', collapsed: false })}
            >
              {t('agentProjects.pinned.expandAll')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <DropArea id="pinned-end" over="pinned-end" zone="end" className="p-1.5">
        {body.length > 0 ? (
          <div className="flex flex-col">{body}</div>
        ) : (
          <p className="px-3 py-6 text-center text-[12.5px] text-muted-foreground">
            {t('agentProjects.pinned.empty')}
          </p>
        )}
      </DropArea>
      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('agentProjects.section.deleteTitle', {
                name: deleting?.name ?? '',
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('agentProjects.section.deleteBody', {
                count: deleting ? projectsInside(arrangement, deleting.id) : 0,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (deleting) remove(deleting)
                setDeleting(null)
              }}
            >
              {t('agentProjects.section.deleteConfirm')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  )
}
