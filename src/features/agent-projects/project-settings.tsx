/**
 * Project settings (`agent-history.md` §5.7), a side sheet: the name, the
 * folders whose conversations count here (each with its role, why it is
 * here, and Move out where that is allowed), Add folder, and suggestions
 * of folders that may belong here. Only PathKeep's names and grouping
 * change; the tools' own folders and files are never touched.
 */
import {
  FolderClosed,
  FolderGit2,
  FolderInput,
  FolderOpenDot,
  GitBranch,
  Plus,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import type {
  ProjectCard,
  ProjectFolder,
} from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'
import {
  useProjectDetail,
  useProjectSuggestions,
} from '@/lib/queries/agent-projects'
import { NameForm } from './project-dialogs'
import { ProjectIcon } from './project-icon'
import { projectName, tildePath } from './project-words'
import { keepOpenForNotices } from './projects-context'
import { SuggestionList } from './suggestions'
import { useProjectEdit } from './use-project-edit'

const roleKey: Record<ProjectFolder['role'], MessageKey> = {
  main: 'agentProjects.settings.role.main',
  worktree: 'agentProjects.settings.role.worktree',
  subfolder: 'agentProjects.settings.role.subfolder',
  temp: 'agentProjects.settings.role.temp',
  userAdded: 'agentProjects.settings.role.userAdded',
}

const roleIcon = {
  main: FolderGit2,
  worktree: GitBranch,
  subfolder: FolderClosed,
  temp: FolderOpenDot,
  userAdded: FolderInput,
}

function reasonText(t: ReturnType<typeof useI18n>['t'], folder: ProjectFolder) {
  switch (folder.reason) {
    case 'repository':
      return t('agentProjects.settings.reason.repository')
    case 'folder':
      return t('agentProjects.settings.reason.folder')
    case 'commonGitDirectory':
      return t('agentProjects.settings.reason.worktree', {
        branch: folder.branch ?? '—',
      })
    case 'insideRepository':
      return t('agentProjects.settings.reason.inside')
    case 'home':
      return t('agentProjects.settings.reason.home')
    case 'temporary':
      return t('agentProjects.settings.reason.temporary')
    case 'userAssigned':
      return folder.addedFromProject
        ? t('agentProjects.settings.reason.added', {
            name:
              folder.addedFromProject.displayName ||
              folder.addedFromProject.defaultName,
          })
        : t('agentProjects.settings.reason.assigned')
    case 'userDetached':
      return t('agentProjects.settings.reason.detached')
  }
}

/** Inside the main folder: `./sub`; elsewhere: the path, `~` for home. */
function shownPath(path: string, main: string | null, home: string | null) {
  if (main && path !== main)
    for (const separator of ['/', '\\'])
      if (path.startsWith(main + separator))
        return `.${separator}${path.slice(main.length + 1)}`
  return tildePath(path, home)
}

function FolderRow({
  folder,
  project,
  home,
}: {
  folder: ProjectFolder
  project: ProjectCard
  home: string | null
}) {
  const { t } = useI18n()
  const edit = useProjectEdit()
  const Icon = roleIcon[folder.role]
  const path = shownPath(folder.path, project.mainFolder, home)
  const fullPath = tildePath(folder.path, home)
  const canMoveOut = folder.role !== 'main' && project.kind !== 'scratch'
  return (
    <li
      data-folder-path={folder.path}
      className="group flex items-start gap-2.5 rounded-md px-2 py-2 hover:bg-muted/50"
    >
      <Icon
        className="mt-0.5 size-4 shrink-0 text-muted-foreground"
        aria-hidden
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex min-w-0 flex-wrap items-center gap-x-2">
          <span
            title={folder.path}
            className={cn(
              'truncate font-mono text-[12.5px]',
              folder.exists === false && 'text-muted-foreground line-through',
            )}
          >
            {path}
          </span>
          <span className="text-[12px] text-muted-foreground">
            {t(roleKey[folder.role])}
          </span>
          {folder.exists === false && (
            <span className="text-[12px] text-muted-foreground">
              {t('agentProjects.settings.deleted')}
            </span>
          )}
        </div>
        <span className="text-[12px] text-muted-foreground">
          {reasonText(t, folder)}
        </span>
      </div>
      <span className="shrink-0 pt-0.5 text-[12px] text-muted-foreground tabular">
        {folder.conversationCount > 0
          ? t('agentProjects.row.conversations', {
              count: folder.conversationCount,
            })
          : t('agentProjects.settings.noConversations')}
      </span>
      {canMoveOut && (
        <Button
          size="xs"
          variant="ghost"
          aria-label={t('agentProjects.settings.moveOutLabel', {
            path: fullPath,
          })}
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          onClick={() =>
            void edit(
              { kind: 'detach', id: project.id, path: folder.path },
              {
                title:
                  folder.role === 'userAdded'
                    ? t('agentProjects.toast.movedOutAdded', { path: fullPath })
                    : t('agentProjects.toast.movedOut', { path: fullPath }),
              },
            )
          }
        >
          {t('agentProjects.settings.moveOut')}
        </Button>
      )}
    </li>
  )
}

export function ProjectSettings({
  project,
  home,
  onAddFolder,
  onClose,
}: {
  project: ProjectCard | null
  home: string | null
  onAddFolder: (project: ProjectCard) => void
  onClose: () => void
}) {
  const { t } = useI18n()
  const detail = useProjectDetail(project?.id ?? null)
  const suggestions = useProjectSuggestions(
    project?.id ?? null,
    project !== null,
  )
  const data = detail.data?.status === 'ok' ? detail.data : null
  // The newest card: a rename or a move out changes it while open.
  const card = data?.project ?? project
  const folders = data?.folders ?? []
  const gone = folders.filter((folder) => folder.exists === false).length
  const forThis = (suggestions.data?.items ?? []).filter(
    (item) => item.decision === 'pending' || item.decision === 'later',
  )
  return (
    <Sheet open={project !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent
        className="w-[580px] gap-0 sm:max-w-[580px]"
        onInteractOutside={keepOpenForNotices}
      >
        {card && (
          <>
            <SheetHeader className="border-b">
              <SheetTitle>{t('agentProjects.settings.title')}</SheetTitle>
              <SheetDescription className="flex min-w-0 items-center gap-1.5">
                <ProjectIcon project={card} />
                <span className="shrink-0 font-medium text-foreground">
                  {projectName(t, card)}
                </span>
                {card.mainFolder && (
                  <span className="truncate font-mono text-[12px]">
                    · {tildePath(card.mainFolder, home)}
                  </span>
                )}
              </SheetDescription>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto p-5">
              <NameForm key={`${card.id}:${card.displayName}`} project={card} />
              <section
                aria-labelledby="folders-heading"
                className="flex flex-col gap-2"
              >
                <div className="flex items-center gap-2">
                  <h3
                    id="folders-heading"
                    className="text-[13.5px] font-semibold"
                  >
                    {t('agentProjects.settings.folders', {
                      count: folders.length,
                    })}
                    {gone > 0 &&
                      t('agentProjects.settings.foldersGone', { count: gone })}
                  </h3>
                  {card.kind !== 'scratch' && (
                    <Button
                      size="xs"
                      variant="outline"
                      className="ml-auto"
                      onClick={() => onAddFolder(card)}
                    >
                      <Plus aria-hidden />
                      {t('agentProjects.settings.addFolder')}
                    </Button>
                  )}
                </div>
                <p className="text-[12.5px] text-muted-foreground">
                  {t('agentProjects.settings.foldersHelp')}
                </p>
                {!data ? (
                  <p
                    role="status"
                    className="text-[12.5px] text-muted-foreground"
                  >
                    {detail.isError
                      ? t('agentProjects.states.failed')
                      : t('agentProjects.states.loading')}
                  </p>
                ) : (
                  <ul className="flex flex-col">
                    {folders.map((folder) => (
                      <FolderRow
                        key={folder.path}
                        folder={folder}
                        project={card}
                        home={home}
                      />
                    ))}
                  </ul>
                )}
              </section>
              {forThis.length > 0 && (
                <section className="flex flex-col gap-2">
                  <h3 className="text-[13.5px] font-semibold">
                    {t('agentProjects.suggestions.title')}
                  </h3>
                  <SuggestionList items={forThis} home={home} />
                </section>
              )}
              <p className="mt-auto text-[12px] text-muted-foreground">
                {t('agentProjects.settings.footer')}
              </p>
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}
