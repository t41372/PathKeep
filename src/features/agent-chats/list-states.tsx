/**
 * What the list shows when it has no rows to draw (`agent-history.md`
 * §3.3): skeleton rows while the first page loads, a failure with Retry,
 * no match for the filters, and no conversation saved at all (the folders
 * PathKeep looked in and a way to Backup); and the lines over the rows:
 * the first backup still running, the trash.
 */
import { Filter, MessagesSquare, TriangleAlert } from 'lucide-react'
import { Link } from 'react-router-dom'
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
import { useI18n } from '@/lib/i18n'
import { useAgentOverview } from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'

export function ListSkeleton() {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('agentChats.states.loading')}
      className="flex-1 overflow-hidden px-2 pt-2"
    >
      {Array.from({ length: 10 }, (_, index) => (
        <div
          key={index}
          className="flex h-[60px] flex-col justify-center gap-2 px-3"
          style={{ opacity: 1 - index * 0.08 }}
        >
          <Skeleton className="h-3.5 w-3/5" />
          <Skeleton className="h-3 w-2/5" />
        </div>
      ))}
    </div>
  )
}

export function ListFailed({
  error,
  onRetry,
}: {
  error: Error
  onRetry: () => void
}) {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <TriangleAlert />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentChats.states.failedTitle')}
        </EmptyTitle>
        <EmptyDescription className="break-words">
          {error.message}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button variant="outline" size="sm" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      </EmptyContent>
    </Empty>
  )
}

export function NoMatch() {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Filter />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentChats.states.emptyTitle')}
        </EmptyTitle>
        <EmptyDescription>{t('agentChats.states.emptyBody')}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}

/** Nothing saved yet: where PathKeep looked, and the way to Backup. */
export function NoConversations() {
  const { t } = useI18n()
  const overview = useAgentOverview('backup')
  const lookedIn = overview.data?.lookedIn ?? []
  return (
    <Empty className="animate-rise flex-1">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MessagesSquare />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentChats.states.noneTitle')}
        </EmptyTitle>
        {overview.data && (
          <EmptyDescription>
            {lookedIn.length > 0
              ? t('agentChats.states.noneBody')
              : t('agentChats.states.noneNoFolders')}
          </EmptyDescription>
        )}
      </EmptyHeader>
      <EmptyContent>
        {lookedIn.length > 0 && (
          <ul className="flex w-full flex-col gap-1 text-left">
            {lookedIn.map((folder) => (
              <li
                key={folder}
                className="rounded-md bg-muted px-2 py-1 font-mono text-[11.5px] break-all"
              >
                {folder}
              </li>
            ))}
          </ul>
        )}
        <Button asChild size="sm">
          <Link to={spaceBackupPath('agent')}>
            {t('agentChats.states.goBackup')}
          </Link>
        </Button>
      </EmptyContent>
    </Empty>
  )
}

/** A line over the rows: the first backup still running, or what the trash is. */
export function ListNote({ children }: { children: string }) {
  return (
    <p className="animate-rise border-b px-5 py-2.5 text-[12.5px] leading-relaxed text-muted-foreground">
      {children}
    </p>
  )
}
