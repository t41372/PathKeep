/**
 * What Insights show instead of numbers (`agent-history.md` §2.5): the
 * skeleton while the report is read, a range or filter with no
 * conversation (said in words, never drawn as zeros), a project the
 * address names that no longer exists, and a report that could not be
 * read, with Retry. "No conversation saved at all" is History's own
 * (`NoConversations`), with the folders PathKeep looked in.
 */
import { FolderX, MessagesSquare, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
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
import type { InsightsRange } from '@/lib/backend-client/agent-insights'
import { useI18n, type MessageKey } from '@/lib/i18n'

/** Five number tiles and four cards; `between` (the year) keeps its place. */
export function ReportSkeleton({ between }: { between?: ReactNode }) {
  const { t } = useI18n()
  return (
    <>
      <div
        role="status"
        aria-label={t('agentInsights.states.loading')}
        className="grid grid-cols-[repeat(auto-fit,minmax(190px,1fr))] gap-3"
      >
        {Array.from({ length: 5 }, (_, index) => (
          <Skeleton key={index} className="h-[104px] rounded-xl" />
        ))}
      </div>
      {between}
      <div
        aria-hidden
        className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,456px),1fr))] gap-4"
      >
        {Array.from({ length: 4 }, (_, index) => (
          <Skeleton key={index} className="h-[300px] rounded-xl" />
        ))}
      </div>
    </>
  )
}

/** Nothing in this range, or nothing that matches the filters in it. */
export function EmptyReport({
  range,
  filtered,
  onAllTime,
  onClearFilters,
}: {
  range: InsightsRange
  filtered: boolean
  onAllTime: () => void
  onClearFilters: () => void
}) {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise rounded-xl border bg-card py-10 shadow-card">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MessagesSquare />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {filtered
            ? t('agentInsights.states.emptyFilter')
            : t(`agentInsights.states.emptyIn.${range}` as MessageKey)}
        </EmptyTitle>
        <EmptyDescription>
          {t('agentInsights.states.emptyBody')}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent className="flex-row justify-center">
        {filtered && (
          <Button size="sm" variant="outline" onClick={onClearFilters}>
            {t('agentInsights.states.clearFilters')}
          </Button>
        )}
        {range !== 'all' && (
          <Button
            size="sm"
            variant={filtered ? 'ghost' : 'outline'}
            onClick={onAllTime}
          >
            {t('agentInsights.states.showAll')}
          </Button>
        )}
      </EmptyContent>
    </Empty>
  )
}

export function ProjectNotFound({ onClear }: { onClear: () => void }) {
  const { t } = useI18n()
  return (
    <Empty className="animate-rise rounded-xl border bg-card py-10 shadow-card">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <FolderX />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentInsights.project.notFoundTitle')}
        </EmptyTitle>
        <EmptyDescription>
          {t('agentInsights.project.notFoundBody')}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button size="sm" variant="outline" onClick={onClear}>
          {t('agentInsights.project.clear')}
        </Button>
      </EmptyContent>
    </Empty>
  )
}

export function ReportFailed({
  code,
  onRetry,
}: {
  code: string
  onRetry: () => void
}) {
  const { t } = useI18n()
  return (
    <Empty
      role="alert"
      className="animate-rise rounded-xl border bg-card py-10 shadow-card"
    >
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <TriangleAlert />
        </EmptyMedia>
        <EmptyTitle className="text-base">
          {t('agentInsights.states.failedTitle')}
        </EmptyTitle>
        <EmptyDescription className="break-words">
          {t('agentInsights.states.failedBody', { code })}
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button size="sm" variant="outline" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      </EmptyContent>
    </Empty>
  )
}
