/**
 * One project = the conversation workbench showing only that project.
 * The header names the project and where it lives, and offers its insights
 * and its settings (name, folders) behind buttons; ⋯ is the same project menu
 * the projects page uses, so managing works the same in both places.
 */
import { ChartColumn, ChevronLeft, Ellipsis, Settings2 } from 'lucide-react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { ProjectIcon } from '~/components/bits'
import { archive } from '~/data/archive'
import { num } from '~/data/format'
import { folderWords } from '~/data/labels'
import { ChatsWorkbench } from './chats'
import { openProjectDialog } from './projects/dialogs'
import { ProjectDialogs } from './projects/project-settings'
import { LocatorText, Tools } from './projects/grid'
import { ProjectMenuItems } from './projects/project-menu'
import { relTime, useProjectRows } from './projects/use-rows'

function ProjectHeader({ projectId }: { projectId: string }) {
  const project = archive.projectById.get(projectId)!
  const navigate = useNavigate()
  const { byId, name: nameOf, where } = useProjectRows()
  const row = byId.get(project.id)
  const name = nameOf(project.id)
  return (
    <header className="flex items-center gap-3 border-b px-4 py-2.5">
      <Link to="/agent/projects" aria-label="回到專案" className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
        <ChevronLeft className="size-4" />
      </Link>
      <ProjectIcon project={project} className="size-4" />
      <div className="flex min-w-0 flex-col">
        <h1 className="truncate text-[15px] leading-tight font-semibold">{name}</h1>
        {row ? <LocatorText where={where(row)} /> : <span className="truncate font-mono text-[11.5px] text-muted-foreground">{project.locations[0].path}</span>}
      </div>
      {row && (
        <span className="ml-1">
          <Tools ids={row.harnesses} />
        </span>
      )}
      <span className="ml-1 hidden text-[12px] text-muted-foreground tabular min-[1180px]:inline">
        {num(row?.chatCount ?? project.chatIds.length)} 段 · {relTime(row?.lastActive ?? project.lastActive)}
      </span>
      <span className="flex-1" />
      <Button variant="outline" size="sm" onClick={() => void navigate(`/agent/projects/${project.id}/insights`)}>
        <ChartColumn />
        洞察
      </Button>
      <Button variant="outline" size="sm" onClick={() => openProjectDialog('settings', project.id)}>
        <Settings2 />
        {folderWords.settings}
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button type="button" aria-label="這個專案的選項" className="flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
            <Ellipsis className="size-4" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-[210px]">
          <ProjectMenuItems id={project.id} label={name} inProject />
        </DropdownMenuContent>
      </DropdownMenu>
      <ProjectDialogs />
    </header>
  )
}

export default function ProjectPage() {
  const { projectId } = useParams()
  const project = projectId ? archive.projectById.get(projectId) : undefined
  if (!project) return <div className="flex flex-1 items-center justify-center text-muted-foreground">找不到這個專案</div>
  return <ChatsWorkbench key={project.id} projectId={project.id} header={<ProjectHeader projectId={project.id} />} />
}
