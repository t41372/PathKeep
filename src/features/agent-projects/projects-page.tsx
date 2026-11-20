/**
 * Projects of the Agent space (`#/agent/projects`, `agent-history.md` §5):
 * get to a project's conversations, and arrange the projects that matter.
 * Three views: the default one (Pinned and Other projects), a sortable
 * list, and folders; `?view=list|tree` keeps the choice and `?review=1`
 * opens the folder suggestions.
 *
 * Responsible for: the page frame, the view switch, its states (loading,
 * failed, no projects yet, the first backup still running). Not
 * responsible for: the views or the dialogs (the sibling files).
 */
import { FolderKanban, TriangleAlert } from 'lucide-react'
import { useRef } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
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
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useProjectList } from '@/lib/queries/agent-projects'
import { spaceBackupPath } from '@/lib/spaces'
import { FolderView } from './folder-view'
import { ListView } from './list-view'
import { OtherCard } from './other-card'
import { PinnedCard } from './pinned-card'
import { PinnedDrag } from './pinned-drag'
import { PageScrollContext } from './projects-context'
import { ProjectsProvider } from './projects-provider'
import { SuggestionsSheet } from './suggestions'

type View = 'default' | 'list' | 'tree'

function CardSkeleton() {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-label={t('agentProjects.states.loading')}
      className="flex flex-col gap-2 rounded-xl border bg-card p-4 shadow-card"
    >
      <Skeleton className="h-4 w-32" />
      {Array.from({ length: 5 }, (_, index) => (
        <Skeleton key={index} className="h-7 w-full" />
      ))}
    </div>
  )
}

export default function ProjectsPage() {
  const { t } = useI18n()
  const [search, setSearch] = useSearchParams()
  const scroll = useRef<HTMLDivElement>(null)
  const { row } = useAgentKind()
  const all = useProjectList({})
  const view: View =
    search.get('view') === 'list'
      ? 'list'
      : search.get('view') === 'tree'
        ? 'tree'
        : 'default'
  const review = search.get('review') === '1'
  const setParam = (key: string, value: string | null) =>
    setSearch(
      (previous) => {
        const next = new URLSearchParams(previous)
        if (value === null) next.delete(key)
        else next.set(key, value)
        return next
      },
      { replace: true },
    )

  const body = () => {
    if (all.isError && !all.data)
      return (
        <Empty className="animate-rise">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <TriangleAlert />
            </EmptyMedia>
            <EmptyTitle className="text-base">
              {t('agentProjects.states.failed')}
            </EmptyTitle>
            <EmptyDescription className="break-words">
              {describeError(all.error)}
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void all.refetch()}
            >
              {t('common.retry')}
            </Button>
          </EmptyContent>
        </Empty>
      )
    if (!all.data)
      return (
        <div className="flex flex-col gap-4">
          <CardSkeleton />
          <CardSkeleton />
        </div>
      )
    if (all.data.total === 0)
      return (
        <Empty className="animate-rise">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <FolderKanban />
            </EmptyMedia>
            <EmptyTitle className="text-base">
              {t('agentProjects.states.noneTitle')}
            </EmptyTitle>
            <EmptyDescription>
              {t('agentProjects.states.noneBody')}
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button asChild variant="outline" size="sm">
              <Link to={spaceBackupPath('agent')}>
                {t('agentProjects.states.toBackup')}
              </Link>
            </Button>
          </EmptyContent>
        </Empty>
      )
    if (view === 'list') return <ListView />
    if (view === 'tree') return <FolderView />
    return (
      <PinnedDrag>
        <div className="flex flex-col gap-4">
          <PinnedCard />
          <OtherCard all={all.data} />
        </div>
      </PinnedDrag>
    )
  }

  return (
    <ProjectsProvider all={all.data}>
      <PageScrollContext.Provider value={scroll}>
        <div ref={scroll} className="min-h-0 min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto flex max-w-[1180px] flex-col gap-4 px-7 pb-10">
            <header
              {...dragRegion()}
              className="flex flex-wrap items-end gap-3 pt-6 pb-1"
            >
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <h1 className="text-[22px] font-semibold tracking-[-0.01em]">
                  {t('shell.nav.projects')}
                </h1>
                <p className="min-h-5 text-[13px] text-muted-foreground">
                  {all.data &&
                    t('agentProjects.subtitle', { count: all.data.total })}
                </p>
              </div>
              <ToggleGroup
                type="single"
                variant="outline"
                value={view}
                aria-label={t('agentProjects.views.label')}
                onValueChange={(next) =>
                  next && setParam('view', next === 'default' ? null : next)
                }
              >
                <ToggleGroupItem
                  value="default"
                  className="h-8 px-3 text-[12.5px]"
                >
                  {t('agentProjects.views.default')}
                </ToggleGroupItem>
                <ToggleGroupItem
                  value="list"
                  className="h-8 px-3 text-[12.5px]"
                >
                  {t('agentProjects.views.list')}
                </ToggleGroupItem>
                <ToggleGroupItem
                  value="tree"
                  className="h-8 px-3 text-[12.5px]"
                >
                  {t('agentProjects.views.tree')}
                </ToggleGroupItem>
              </ToggleGroup>
            </header>
            {row?.firstBackup === 'running' && (
              <p className="rounded-md bg-muted px-3 py-2 text-[12.5px] text-muted-foreground">
                {t('agentProjects.states.firstBackup')}
              </p>
            )}
            {body()}
          </div>
        </div>
      </PageScrollContext.Provider>
      <SuggestionsSheet
        open={review}
        home={all.data?.homeFolder ?? null}
        onClose={() => setParam('review', null)}
      />
    </ProjectsProvider>
  )
}
