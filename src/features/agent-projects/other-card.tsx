/**
 * The default view's Other projects card (`agent-history.md` §5.3): every
 * project not pinned, with search, tools, status and sort; a line when
 * some folders may belong to another project; the 15 most recent until
 * "Show all". New projects appear here. The whole card takes a dragged
 * pin to unpin it.
 */
import { FolderSearch } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import type {
  ProjectCard,
  ProjectList,
} from '@/lib/backend-client/agent-projects'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import { PageRows } from './page-rows'
import { DropArea } from './pinned-drag'
import {
  ProjectSearch,
  SortMenu,
  StatusToggle,
  ToolsMenu,
  type OtherSort,
  type ProjectStatus,
} from './project-filters'
import { ProjectRow } from './project-row'
import { projectName } from './project-words'
import { useProjectsShared } from './projects-context'
import { useDebounced } from './use-debounced'
import { useGrip } from './use-pinned-drag'

const FIRST = 15

function OtherRow({ project }: { project: ProjectCard }) {
  const { ref, grip, isDragging } = useGrip(project.id, 'project')
  return (
    <div ref={ref} className={isDragging ? 'opacity-40' : undefined}>
      <ProjectRow project={project} grip={grip} />
    </div>
  )
}

export function OtherCard({ all }: { all: ProjectList | undefined }) {
  const { t, locale } = useI18n()
  const shared = useProjectsShared()
  const [, setSearch] = useSearchParams()
  const [text, setText] = useState('')
  const [tools, setTools] = useState<AgentToolId[]>([])
  const [status, setStatus] = useState<ProjectStatus>('all')
  const [sort, setSort] = useState<OtherSort>('activity')
  const [showAll, setShowAll] = useState(false)
  const settledText = useDebounced(text.trim())
  const narrowed = settledText !== '' || tools.length > 0 || status !== 'all'
  const list = useProjectList({
    otherOnly: true,
    text: settledText,
    harnesses: tools,
    status,
    timeZone: shared.zone,
  })
  const items = list.data?.items
  const sorted = useMemo(() => {
    const rows = [...(items ?? [])]
    if (sort === 'conversations')
      rows.sort(
        (a, b) =>
          b.conversationCount - a.conversationCount ||
          (b.lastActivity ?? 0) - (a.lastActivity ?? 0),
      )
    else if (sort === 'name') {
      const collator = new Intl.Collator(locale)
      rows.sort((a, b) =>
        collator.compare(projectName(t, a), projectName(t, b)),
      )
    }
    return rows
  }, [items, sort, locale, t])
  const shown = narrowed || showAll ? sorted : sorted.slice(0, FIRST)
  const pending = all?.pendingSuggestions ?? 0

  return (
    <DropArea id="other" over="other" zone="other" className="rounded-xl">
      <section
        aria-labelledby="other-title"
        className="rounded-xl border bg-card shadow-card"
      >
        <div className="flex items-center gap-2 px-4 pt-2.5">
          <h2 id="other-title" className="text-[14px] font-semibold">
            {t('agentProjects.other.title')}
          </h2>
          <span className="text-[12.5px] text-muted-foreground tabular">
            {list.data ? list.data.matching : ''}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-2 px-4 py-2.5">
          <ProjectSearch
            value={text}
            label={t('agentProjects.other.searchLabel')}
            placeholder={t('agentProjects.other.searchPlaceholder')}
            onChange={setText}
          />
          <ToolsMenu value={tools} onChange={setTools} />
          <StatusToggle value={status} onChange={setStatus} />
          <SortMenu value={sort} onChange={setSort} />
        </div>
        {pending > 0 && (
          <div className="mx-4 mb-2 flex items-center gap-2 rounded-md bg-brand-soft px-3 py-2 text-[12.5px]">
            <FolderSearch className="size-4 text-brand" aria-hidden />
            <span className="flex-1">
              {t('agentProjects.other.suggestions', { count: pending })}
            </span>
            <Button
              size="xs"
              variant="outline"
              onClick={() =>
                setSearch(
                  (previous) => {
                    const next = new URLSearchParams(previous)
                    next.set('review', '1')
                    return next
                  },
                  { replace: true },
                )
              }
            >
              {t('agentProjects.other.review')}
            </Button>
          </div>
        )}
        <div className="px-1.5 pb-1.5">
          {list.isError && !list.data ? (
            <p
              role="alert"
              className="px-3 py-4 text-[12.5px] text-muted-foreground"
            >
              {t('agentProjects.states.failed')}{' '}
              <button
                type="button"
                className="underline underline-offset-2"
                onClick={() => void list.refetch()}
              >
                {t('common.retry')}
              </button>
            </p>
          ) : !list.data ? (
            <p
              role="status"
              className="px-3 py-4 text-[12.5px] text-muted-foreground"
            >
              {t('agentProjects.states.loading')}
            </p>
          ) : shown.length === 0 ? (
            <div className="px-3 py-6 text-center text-[12.5px] text-muted-foreground">
              <p>{t('agentProjects.other.none')}</p>
              {narrowed && <p>{t('agentProjects.other.noneHint')}</p>}
            </div>
          ) : (
            <PageRows items={shown} keyOf={(project) => project.id}>
              {(project) => <OtherRow project={project} />}
            </PageRows>
          )}
        </div>
        {list.data && sorted.length > 0 && (
          <div className="flex items-center gap-3 border-t px-4 py-2 text-[12px] text-muted-foreground">
            {narrowed ? (
              <span>
                {t('agentProjects.other.matching', { count: sorted.length })}
              </span>
            ) : sorted.length > FIRST ? (
              <button
                type="button"
                onClick={() => setShowAll((value) => !value)}
                className="text-foreground underline-offset-2 hover:underline"
              >
                {showAll
                  ? t('agentProjects.other.showFirst', { count: FIRST })
                  : t('agentProjects.other.showAll', { count: sorted.length })}
              </button>
            ) : null}
            <span className="ml-auto">{t('agentProjects.other.newHere')}</span>
          </div>
        )}
      </section>
    </DropArea>
  )
}
