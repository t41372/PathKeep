/**
 * One project's row, the same in every view of the Projects page
 * (`agent-history.md` §5.1): icon, name and where it is, tools,
 * conversations, last activity and ⋯; the list view adds the summed agent
 * time and tokens. Clicking the row or Enter on its name opens the
 * project; right-click opens its ⋯ menu.
 */
import { memo, useState, type ReactNode } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { hoursText } from '@/features/agent-chats/chat-words'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { agentProjectPath } from '@/lib/spaces'
import { ProjectIcon } from './project-icon'
import { ProjectMenu } from './project-menu'
import {
  gridOf,
  lastActive,
  projectLocator,
  projectName,
} from './project-words'
import { useProjectsShared } from './projects-context'
import { useDragState } from './use-pinned-drag'

export function ToolIcons({ tools }: { tools: ProjectCard['harnesses'] }) {
  const { t } = useI18n()
  return (
    <span className="flex items-center gap-1">
      {tools.map((tool) => (
        <span key={tool} title={t(toolName(tool))} className="flex">
          <ToolIcon id={tool} />
          <span className="sr-only">{t(toolName(tool))}</span>
        </span>
      ))}
    </span>
  )
}

export const ProjectRow = memo(function ProjectRow({
  project,
  list = false,
  depth = 0,
  grip,
  className,
}: {
  project: ProjectCard
  /** The list view's extra columns. */
  list?: boolean
  /** Indent inside a section. */
  depth?: number
  /** A drag handle, in the views that can drag. */
  grip?: ReactNode
  className?: string
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const shared = useProjectsShared()
  const [menu, setMenu] = useState(false)
  const { justDropped } = useDragState()
  const name = projectName(t, project)
  const locator = projectLocator(t, project, shared.home)
  const tokens =
    project.tokensTotal === null
      ? '—'
      : shared.f.compact(project.tokensTotal) +
        (project.tokensKnownConversations < project.conversationCount
          ? '*'
          : '')
  return (
    <div
      data-project-id={project.id}
      onClick={(event) => {
        if (justDropped()) return
        if ((event.target as HTMLElement).closest('button, a')) return
        void navigate(agentProjectPath(project.id))
      }}
      onContextMenu={(event) => {
        event.preventDefault()
        setMenu(true)
      }}
      className={cn(
        gridOf(list),
        'group h-10 cursor-pointer rounded-md px-2 text-[13px] transition-colors duration-150 hover:bg-muted/60',
        className,
      )}
    >
      <div
        className="flex min-w-0 items-center gap-2"
        style={{ paddingLeft: depth * 16 }}
      >
        {grip}
        <ProjectIcon project={project} />
        <Link
          to={agentProjectPath(project.id)}
          className={cn(
            'shrink-0 truncate font-medium outline-none hover:underline focus-visible:underline',
            project.gone && 'text-muted-foreground',
          )}
        >
          {name}
        </Link>
        {locator && (
          <span className="min-w-0 truncate text-[12px] text-muted-foreground">
            {locator}
          </span>
        )}
        {project.latestConversationTitle && (
          <span
            title={t('agentProjects.row.latest', {
              title: project.latestConversationTitle,
            })}
            className="ml-auto hidden min-w-0 max-w-[40%] truncate pl-4 text-[12px] text-muted-foreground min-[1500px]:block"
          >
            {project.latestConversationTitle}
          </span>
        )}
      </div>
      <ToolIcons tools={project.harnesses} />
      <span className="text-right text-muted-foreground tabular">
        {shared.f.number(project.conversationCount)}
      </span>
      {list && (
        <>
          <span className="text-right text-muted-foreground tabular">
            {project.summedAgentMs === null
              ? '—'
              : hoursText(t, shared.f, project.summedAgentMs)}
          </span>
          <span
            className="text-right text-muted-foreground tabular"
            title={
              project.tokensTotal !== null &&
              project.tokensKnownConversations < project.conversationCount
                ? t('agentProjects.row.tokensCoverage', {
                    known: project.tokensKnownConversations,
                    count: project.conversationCount,
                  })
                : undefined
            }
          >
            {tokens}
          </span>
        </>
      )}
      <span className="text-right text-[12px] text-muted-foreground">
        {lastActive(t, project.lastActivity, shared.zone)}
      </span>
      <ProjectMenu
        project={project}
        arrangement={shared.arrangement}
        open={menu}
        onOpenChange={setMenu}
        onRename={() => shared.openRename(project)}
        onSettings={() => shared.openSettings(project)}
        className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
      />
    </div>
  )
})
