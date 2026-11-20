/**
 * The Projects list view (`agent-history.md` §5.4): every project in one
 * card, searchable, sorted by any column (a new column starts from the
 * largest, Project from A to Z; ties put the newest first). No dragging.
 */
import { ArrowDown, ArrowUp } from 'lucide-react'
import { useMemo, useState } from 'react'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import { PageRows } from './page-rows'
import { ProjectSearch } from './project-filters'
import { ProjectRow } from './project-row'
import { gridOf } from './project-words'
import { projectName } from './project-words'
import { useDebounced } from './use-debounced'

type Column = 'name' | 'conversations' | 'summed' | 'tokens' | 'activity'
const COLUMNS: { id: Column | null; label: MessageKey; end?: boolean }[] = [
  { id: 'name', label: 'agentProjects.columns.project' },
  { id: null, label: 'agentProjects.columns.tools' },
  {
    id: 'conversations',
    label: 'agentProjects.columns.conversations',
    end: true,
  },
  { id: 'summed', label: 'agentProjects.columns.summed', end: true },
  { id: 'tokens', label: 'agentProjects.columns.tokens', end: true },
  { id: 'activity', label: 'agentProjects.columns.activity', end: true },
]

function value(project: ProjectCard, column: Column) {
  if (column === 'conversations') return project.conversationCount
  if (column === 'summed') return project.summedAgentMs ?? -1
  if (column === 'tokens') return project.tokensTotal ?? -1
  return project.lastActivity ?? -1
}

export function ListView() {
  const { t, locale } = useI18n()
  const [text, setText] = useState('')
  const [sort, setSort] = useState<{ column: Column; descending: boolean }>({
    column: 'activity',
    descending: true,
  })
  const settled = useDebounced(text.trim())
  const list = useProjectList({ text: settled })
  const items = list.data?.items
  const rows = useMemo(() => {
    const sorted = [...(items ?? [])]
    const collator = new Intl.Collator(locale)
    sorted.sort((a, b) => {
      const order =
        sort.column === 'name'
          ? collator.compare(projectName(t, a), projectName(t, b))
          : value(a, sort.column) - value(b, sort.column)
      return (
        (sort.descending ? -order : order) ||
        (b.lastActivity ?? 0) - (a.lastActivity ?? 0)
      )
    })
    return sorted
  }, [items, sort, locale, t])
  const pick = (column: Column) =>
    setSort((now) =>
      now.column === column
        ? { column, descending: !now.descending }
        : { column, descending: column !== 'name' },
    )
  return (
    <section
      aria-labelledby="all-title"
      className="rounded-xl border bg-card shadow-card"
    >
      <div className="flex flex-wrap items-center gap-2 px-4 py-2.5">
        <h2 id="all-title" className="text-[14px] font-semibold">
          {t('agentProjects.list.title')}
        </h2>
        <span className="text-[12.5px] text-muted-foreground tabular">
          {list.data?.matching ?? ''}
        </span>
        <div className="ml-auto w-[280px]">
          <ProjectSearch
            value={text}
            label={t('agentProjects.list.searchLabel')}
            placeholder={t('agentProjects.list.searchPlaceholder')}
            onChange={setText}
          />
        </div>
      </div>
      <div
        role="row"
        className={cn(
          gridOf(true),
          'border-y px-3.5 py-1.5 text-[12px] text-muted-foreground',
        )}
      >
        {COLUMNS.map((column) =>
          column.id === null ? (
            <span key={column.label} role="columnheader">
              {t(column.label)}
            </span>
          ) : (
            <span
              key={column.label}
              role="columnheader"
              aria-sort={
                sort.column === column.id
                  ? sort.descending
                    ? 'descending'
                    : 'ascending'
                  : 'none'
              }
              className={cn(column.end && 'text-right')}
            >
              <button
                type="button"
                title={
                  column.id === 'summed'
                    ? t('agentProjects.columns.summedHow')
                    : undefined
                }
                onClick={() => pick(column.id!)}
                className={cn(
                  'inline-flex items-center gap-1 hover:text-foreground',
                  sort.column === column.id && 'text-foreground',
                )}
              >
                {t(column.label)}
                {sort.column === column.id &&
                  (sort.descending ? (
                    <ArrowDown className="size-3" aria-hidden />
                  ) : (
                    <ArrowUp className="size-3" aria-hidden />
                  ))}
              </button>
            </span>
          ),
        )}
        <span />
      </div>
      <div className="p-1.5">
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
        ) : rows.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12.5px] text-muted-foreground">
            {t('agentProjects.list.none', { q: settled })}
          </p>
        ) : (
          <PageRows items={rows} keyOf={(project) => project.id}>
            {(project) => <ProjectRow project={project} list />}
          </PageRows>
        )}
      </div>
    </section>
  )
}
