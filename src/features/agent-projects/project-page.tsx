/**
 * One project (`#/agent/projects/:id`, `agent-history.md` §5.10): its
 * header (back to Projects, its name and place, tools, how many
 * conversations and when last active, Project settings, its menu) over
 * the Records workbench with the project fixed: every conversation of
 * every folder in it, the ones added from other projects included.
 */
import { ChevronLeft, FolderX, Settings2 } from 'lucide-react'
import { Link, useParams } from 'react-router-dom'
import { dragRegion } from '@/components/app/drag-region'
import { Button } from '@/components/ui/button'
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty'
import { Skeleton } from '@/components/ui/skeleton'
import { Workbench } from '@/features/agent-chats/workbench'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useProjectDetail, useProjectList } from '@/lib/queries/agent-projects'
import { agentProjectsPath } from '@/lib/spaces'
import { ProjectIcon } from './project-icon'
import { ProjectMenu } from './project-menu'
import { ToolIcons } from './project-row'
import { lastActive, projectLocator, projectName } from './project-words'
import { useProjectsShared } from './projects-context'
import { ProjectsProvider } from './projects-provider'

function NotFound() {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <FolderX />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentProjects.page.notFound')}
        </EmptyTitle>
        <EmptyDescription>
          {t('agentProjects.page.notFoundBody')}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button asChild size="sm" variant="outline">
          <Link to={agentProjectsPath()}>{t('agentProjects.page.back')}</Link>
        </Button>
      </EmptyContent>
    </Empty>
  )
}

function Header({ project }: { project: ProjectCard }) {
  const { t } = useI18n()
  const shared = useProjectsShared()
  const name = projectName(t, project)
  const locator = projectLocator(t, project, shared.home)
  return (
    <header
      {...dragRegion()}
      className="flex min-w-0 items-center gap-2.5 border-b px-4 pt-4 pb-3"
    >
      <Link
        to={agentProjectsPath()}
        aria-label={t('agentProjects.page.back')}
        className="flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
      >
        <ChevronLeft className="size-4" aria-hidden />
      </Link>
      <ProjectIcon project={project} className="size-5" />
      <div className="flex min-w-0 items-baseline gap-2">
        <h1 className="shrink-0 truncate text-[17px] font-semibold tracking-[-0.01em]">
          {name}
        </h1>
        {locator && (
          <span className="min-w-0 truncate text-[12.5px] text-muted-foreground">
            {locator}
          </span>
        )}
      </div>
      <ToolIcons tools={project.harnesses} />
      <span className="hidden shrink-0 text-[12.5px] text-muted-foreground tabular min-[1180px]:inline">
        {t('agentProjects.page.facts', {
          count: project.conversationCount,
          shown: shared.f.number(project.conversationCount),
          when: lastActive(t, project.lastActivity, shared.zone),
        })}
      </span>
      <div className="ml-auto flex shrink-0 items-center gap-1">
        <Button
          size="sm"
          variant="ghost"
          onClick={() => shared.openSettings(project)}
        >
          <Settings2 aria-hidden />
          {t('agentProjects.page.settings')}
        </Button>
        <ProjectMenu
          project={project}
          arrangement={shared.arrangement}
          onPage
          onRename={() => shared.openRename(project)}
          onSettings={() => shared.openSettings(project)}
        />
      </div>
    </header>
  )
}

export default function ProjectPage() {
  const { t } = useI18n()
  const { id = '' } = useParams()
  const detail = useProjectDetail(id)
  const all = useProjectList({})
  const data = detail.data
  const body = () => {
    if (detail.isError && !data)
      return (
        <Empty className="animate-rise flex-1">
          <EmptyHeader>
            <EmptyTitle className="text-base">
              {t('agentProjects.states.failed')}
            </EmptyTitle>
            <EmptyDescription className="break-words">
              {describeError(detail.error)}
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void detail.refetch()}
            >
              {t('common.retry')}
            </Button>
          </EmptyContent>
        </Empty>
      )
    if (!data)
      return (
        <div
          role="status"
          aria-label={t('agentProjects.states.loading')}
          className="flex flex-col gap-2 border-b px-4 pt-5 pb-3"
        >
          <Skeleton className="h-5 w-1/3" />
          <Skeleton className="h-3.5 w-1/4" />
        </div>
      )
    if (data.status !== 'ok') return <NotFound />
    return (
      <>
        <Header project={data.project} />
        <div className="flex min-h-0 flex-1">
          <Workbench fixedProject={id} />
        </div>
      </>
    )
  }
  return (
    <ProjectsProvider all={all.data}>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">{body()}</div>
    </ProjectsProvider>
  )
}
