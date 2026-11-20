/**
 * A project's icon (`agent-history.md` §5.1): a git repository, a plain
 * folder, or the loose folders; grey, with a line on hover, when every
 * folder is gone from the disk.
 */
import { FolderClosed, FolderGit2, FolderOpenDot } from 'lucide-react'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'

export function ProjectIcon({
  project,
  className,
}: {
  project: Pick<ProjectCard, 'kind' | 'gone'>
  className?: string
}) {
  const { t } = useI18n()
  const Icon =
    project.kind === 'repo'
      ? FolderGit2
      : project.kind === 'scratch'
        ? FolderOpenDot
        : FolderClosed
  const icon = (
    <Icon
      className={cn(
        'size-4 shrink-0',
        project.gone ? 'text-muted-foreground/50' : 'text-muted-foreground',
        className,
      )}
      aria-hidden
    />
  )
  if (!project.gone) return icon
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          role="img"
          tabIndex={-1}
          aria-label={t('agentProjects.row.goneIcon')}
          className="inline-flex shrink-0"
        >
          {icon}
        </span>
      </TooltipTrigger>
      <TooltipContent className="max-w-[260px]">
        {t('agentProjects.row.goneIcon')}
      </TooltipContent>
    </Tooltip>
  )
}
