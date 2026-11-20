/**
 * The Projects folder view (`agent-history.md` §5.5): projects grouped by
 * the folder their main folder is in, folders inside folders, the home
 * folder flattened, the loose folders a group of their own; folders and
 * projects by last activity. Everything starts collapsed and is not
 * remembered; a search opens every folder with a match and holds them
 * open.
 */
import { ChevronRight } from 'lucide-react'
import { useMemo, useState } from 'react'
import type {
  FolderGroup,
  ProjectCard,
} from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import { PageRows } from './page-rows'
import { ProjectSearch } from './project-filters'
import { ProjectRow } from './project-row'
import { gridOf } from './project-words'
import { lastActive, tildePath } from './project-words'
import { useProjectsShared } from './projects-context'
import { useDebounced } from './use-debounced'

type Row =
  | { kind: 'folder'; group: FolderGroup; depth: number; label: string }
  | { kind: 'project'; project: ProjectCard; depth: number }

function byActivity(a: number | null, b: number | null) {
  return (b ?? 0) - (a ?? 0)
}

export function FolderView() {
  const { t } = useI18n()
  const shared = useProjectsShared()
  const [text, setText] = useState('')
  const [open, setOpen] = useState<Set<string>>(new Set())
  const settled = useDebounced(text.trim())
  const searching = settled !== ''
  const list = useProjectList({ text: settled, folderTree: true })
  const data = list.data

  const rows = useMemo(() => {
    if (!data?.tree) return []
    const cards = new Map(data.items.map((card) => [card.id, card]))
    const children = new Map<string | null, FolderGroup[]>()
    for (const group of data.tree.groups) {
      const list = children.get(group.parent) ?? []
      list.push(group)
      children.set(group.parent, list)
    }
    const out: Row[] = []
    const walk = (
      parent: FolderGroup | null,
      projects: string[],
      depth: number,
    ) => {
      const entries: (
        | { at: number | null; group: FolderGroup }
        | { at: number | null; project: ProjectCard }
      )[] = []
      for (const group of children.get(parent?.id ?? null) ?? [])
        entries.push({ at: group.lastActivity, group })
      for (const id of projects) {
        const project = cards.get(id)
        if (project) entries.push({ at: project.lastActivity, project })
      }
      entries.sort((a, b) => byActivity(a.at, b.at))
      for (const entry of entries) {
        if ('project' in entry) {
          out.push({ kind: 'project', project: entry.project, depth })
          continue
        }
        const group = entry.group
        const label =
          group.kind === 'scratch'
            ? t('agentProjects.scratchName')
            : parent
              ? group.id.slice(parent.id.length + 1)
              : tildePath(group.id, data.homeFolder)
        out.push({ kind: 'folder', group, depth, label })
        if (searching || open.has(group.id))
          walk(group, group.projectIds, depth + 1)
      }
    }
    walk(null, data.tree.topLevelProjects, 0)
    return out
  }, [data, open, searching, t])

  const toggle = (id: string) =>
    setOpen((now) => {
      const next = new Set(now)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })

  return (
    <section
      aria-labelledby="folders-title"
      className="rounded-xl border bg-card shadow-card"
    >
      <div className="flex flex-wrap items-center gap-2 px-4 py-2.5">
        <h2 id="folders-title" className="text-[14px] font-semibold">
          {t('agentProjects.folders.title')}
        </h2>
        <span className="text-[12.5px] text-muted-foreground tabular">
          {data?.matching ?? ''}
        </span>
        {open.size > 0 && !searching && (
          <button
            type="button"
            onClick={() => setOpen(new Set())}
            className="h-7 rounded-md px-2 text-[12.5px] text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {t('agentProjects.pinned.collapseAll')}
          </button>
        )}
        <div className="ml-auto w-[280px]">
          <ProjectSearch
            value={text}
            label={t('agentProjects.list.searchLabel')}
            placeholder={t('agentProjects.list.searchPlaceholder')}
            onChange={setText}
          />
        </div>
      </div>
      <div className="border-t p-1.5">
        {list.isError && !data ? (
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
        ) : !data ? (
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
          <PageRows
            items={rows}
            keyOf={(row) =>
              row.kind === 'folder' ? `f:${row.group.id}` : row.project.id
            }
          >
            {(row) =>
              row.kind === 'project' ? (
                <ProjectRow project={row.project} depth={row.depth} />
              ) : (
                <button
                  type="button"
                  data-folder={row.group.id}
                  aria-expanded={searching || open.has(row.group.id)}
                  disabled={searching}
                  onClick={() => toggle(row.group.id)}
                  className={cn(
                    gridOf(false),
                    'h-10 w-full rounded-md px-2 text-left text-[13px] transition-colors duration-150 hover:bg-muted/60 disabled:cursor-default',
                  )}
                >
                  <span
                    className="flex min-w-0 items-center gap-2"
                    style={{ paddingLeft: row.depth * 16 }}
                  >
                    <ChevronRight
                      className={cn(
                        'size-4 shrink-0 text-muted-foreground transition-transform duration-200',
                        (searching || open.has(row.group.id)) && 'rotate-90',
                      )}
                      aria-hidden
                    />
                    <span className="truncate font-mono text-[12.5px]">
                      {row.label}
                    </span>
                    <span className="shrink-0 text-[12px] text-muted-foreground tabular">
                      {row.group.projectCount}
                    </span>
                  </span>
                  <span />
                  <span className="text-right text-muted-foreground/70 tabular">
                    {shared.f.number(row.group.conversationCount)}
                  </span>
                  <span className="text-right text-[12px] text-muted-foreground">
                    {lastActive(t, row.group.lastActivity, shared.zone)}
                  </span>
                  <span />
                </button>
              )
            }
          </PageRows>
        )}
      </div>
    </section>
  )
}
