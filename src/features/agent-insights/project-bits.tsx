/**
 * A project as Insights writes it (`agent-history.md` §5.1): its icon (a
 * git repository, a plain folder, or the loose folders; faded when every
 * folder is gone from the disk). Its name is `projectName` in
 * `words.ts`.
 */
import { FolderClosed, FolderGit2, FolderOpenDot } from 'lucide-react'
import type { InsightsProjectChip } from '@/lib/backend-client/agent-insights'
import { cn } from '@/lib/cn'

type Kind = Pick<InsightsProjectChip, 'kind' | 'gone'>

export function ProjectIcon({
  project,
  className,
}: {
  project: Kind
  className?: string
}) {
  const Icon =
    project.kind === 'repo'
      ? FolderGit2
      : project.kind === 'scratch'
        ? FolderOpenDot
        : FolderClosed
  return (
    <Icon
      aria-hidden
      className={cn(
        'size-3.5 shrink-0',
        project.gone ? 'text-muted-foreground/50' : 'text-muted-foreground',
        className,
      )}
    />
  )
}
