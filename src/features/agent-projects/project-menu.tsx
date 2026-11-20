/**
 * The project menu (`agent-history.md` §5.6), the same from a row's ⋯, a
 * right-click on the row, and a project page's ⋯: open its conversations
 * (not on its own page), its Insights, rename, pin or unpin, move to a
 * section, project settings, show the main folder in the file manager. Every change says
 * what it did, with Undo.
 */
import {
  ChartColumn,
  Ellipsis,
  FolderInput,
  FolderOpen,
  MessagesSquare,
  Pencil,
  Pin,
  PinOff,
  Settings2,
} from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { revealLabel } from '@/features/agent-reader/reader-words'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { supportClient } from '@/lib/backend-client/support'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { agentInsightsPath, agentProjectPath } from '@/lib/spaces'
import {
  placeOf,
  sectionList,
  selfAndInside,
  type Arrangement,
} from './arrangement'
import { projectName } from './project-words'
import { useProjectEdit } from './use-project-edit'

/**
 * "Move to ▸" for a pin or a section: the top of Pinned, each section
 * (indented; not where it is now, not into itself), and for a project,
 * Other projects.
 */
export function MoveToMenu({
  id,
  label,
  kind,
  arrangement,
}: {
  id: string
  label: string
  kind: 'project' | 'section'
  arrangement: Arrangement
}) {
  const { t } = useI18n()
  const edit = useProjectEdit()
  const place = placeOf(arrangement, id)
  const blocked = kind === 'section' ? selfAndInside(arrangement, id) : null
  const moved = (where: string) =>
    t('agentProjects.toast.moved', { name: label, where })
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>
        <FolderInput aria-hidden />
        {t('agentProjects.menu.moveTo')}
      </DropdownMenuSubTrigger>
      <DropdownMenuSubContent className="max-h-[360px] w-[240px] overflow-y-auto">
        <DropdownMenuItem
          disabled={place !== null && place.parent === null}
          onSelect={() =>
            void edit(
              { kind: 'move', id, parent: null },
              { title: moved(t('agentProjects.where.top')) },
            )
          }
        >
          {t('agentProjects.menu.pinnedTop')}
        </DropdownMenuItem>
        {sectionList(arrangement).map(({ node, depth }) => (
          <DropdownMenuItem
            key={node.id}
            disabled={
              place?.parent === node.id || (blocked?.has(node.id) ?? false)
            }
            style={{ paddingLeft: 8 + (depth + 1) * 12 }}
            onSelect={() =>
              void edit(
                { kind: 'move', id, parent: node.id },
                {
                  title: moved(
                    t('agentProjects.where.section', { name: node.name ?? '' }),
                  ),
                },
              )
            }
          >
            {node.name}
          </DropdownMenuItem>
        ))}
        {kind === 'project' && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              disabled={place === null}
              onSelect={() =>
                void edit(
                  { kind: 'unpin', id },
                  { title: moved(t('agentProjects.where.other')) },
                )
              }
            >
              {t('agentProjects.menu.otherUnpin')}
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuSubContent>
    </DropdownMenuSub>
  )
}

export function ProjectMenu({
  project,
  arrangement,
  onRename,
  onSettings,
  onPage = false,
  open,
  onOpenChange,
  className,
}: {
  project: ProjectCard
  arrangement: Arrangement
  onRename: () => void
  onSettings: () => void
  /** On the project's own page: no "Open conversations". */
  onPage?: boolean
  open?: boolean
  onOpenChange?: (open: boolean) => void
  className?: string
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const edit = useProjectEdit()
  const name = projectName(t, project)
  const pinned = arrangement.pinned.has(project.id)
  const canReveal = project.mainFolder !== null && project.mainExists !== false
  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={
            onPage
              ? t('agentProjects.menu.pageLabel')
              : t('agentProjects.menu.label', { name })
          }
          onClick={(event) => event.stopPropagation()}
          className={cn(
            'flex size-7 items-center justify-center rounded-md text-muted-foreground transition-[opacity,background-color] duration-150 hover:bg-muted hover:text-foreground data-[state=open]:opacity-100',
            className,
          )}
        >
          <Ellipsis className="size-4" aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="w-[240px]"
        onClick={(event) => event.stopPropagation()}
      >
        {!onPage && (
          <DropdownMenuItem
            onSelect={() => void navigate(agentProjectPath(project.id))}
          >
            <MessagesSquare aria-hidden />
            {t('agentProjects.menu.open')}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem
          onSelect={() => void navigate(agentInsightsPath(project.id))}
        >
          <ChartColumn aria-hidden />
          {t('agentProjects.menu.insights')}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onRename}>
          <Pencil aria-hidden />
          {t('agentProjects.menu.rename')}
        </DropdownMenuItem>
        {pinned ? (
          <DropdownMenuItem
            onSelect={() =>
              void edit(
                { kind: 'unpin', id: project.id },
                { title: t('agentProjects.toast.unpinned', { name }) },
              )
            }
          >
            <PinOff aria-hidden />
            {t('agentProjects.menu.unpin')}
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            onSelect={() =>
              void edit(
                { kind: 'move', id: project.id, parent: null },
                { title: t('agentProjects.toast.pinned', { name }) },
              )
            }
          >
            <Pin aria-hidden />
            {t('agentProjects.menu.pin')}
          </DropdownMenuItem>
        )}
        <MoveToMenu
          id={project.id}
          label={name}
          kind="project"
          arrangement={arrangement}
        />
        <DropdownMenuItem onSelect={onSettings}>
          <Settings2 aria-hidden />
          {t('agentProjects.menu.settings')}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={!canReveal}
          onSelect={() =>
            void supportClient
              .openPathInFileManager(project.mainFolder!)
              .catch((error: unknown) =>
                toast.error(t('agentReader.menu.revealFailed'), {
                  description: describeError(error),
                }),
              )
          }
        >
          <FolderOpen aria-hidden />
          {t(revealLabel())}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
