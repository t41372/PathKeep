/**
 * Agent conversations' pane in the sources step: one row per tool found on
 * this computer, with the folders it reads, how many files look like
 * conversations and subagents, and their size. Nothing was opened to count
 * them. Under the list, what else a backup reads (each project's `.git`) and,
 * with Claude Code ticked, that it deletes old conversations by itself.
 *
 * Every state of the walk has its face: counting (rows as each folder is
 * counted, a skeleton for the rest), no tool found (where PathKeep looks,
 * and another check), failed (why, and another check), and a folder that
 * can't be read (its row says why; the other tools are as usual).
 * Not responsible for: the walk (`use-agent-sources.ts`), the pane's header
 * and list (`source-pane.tsx`) or keeping the ticks (the page).
 */
import {
  CircleAlert,
  FolderGit2,
  ScanSearch,
  TriangleAlert,
} from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Skeleton } from '@/components/ui/skeleton'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { folderLabel, inToolOrder, toolName } from '@/features/data-kinds/tools'
import { rowNote } from '@/features/enable-agent/preview-model'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import { walkFailureText } from './agent-words'
import { PaneHeader, SourceList } from './source-pane'
import { GROUP_AFTER, summaryText, type KindCounts } from './sources-model'
import type { AgentSources, AgentToolSource } from './use-agent-sources'

export function AgentSourcesPane({
  headingId,
  title,
  counts,
  sources,
  chosen,
  onChange,
  filter,
  onFilter,
}: {
  headingId: string
  title: string
  counts: KindCounts
  sources: AgentSources
  chosen: AgentToolId[]
  onChange: (tools: AgentToolId[]) => void
  filter: string
  onFilter: (filter: string) => void
}) {
  const { t } = useI18n()
  const { state, tools } = sources
  const ticked = new Set<string>(chosen)
  const items = tools.map((source) => ({
    id: source.tool,
    group: t(toolName(source.tool)),
    text: [t(toolName(source.tool)), ...source.roots.map((r) => r.path)].join(
      ' ',
    ),
    source,
  }))
  const walking = state.phase === 'walking'
  const stopped = state.phase === 'failed' || state.phase === 'cancelled'
  const claude = tools.find(
    (source) => source.tool === 'claude-code' && source.readable,
  )

  function set(ids: string[], on: boolean) {
    const next = new Set<string>(chosen)
    for (const id of ids) {
      if (on) next.add(id)
      else next.delete(id)
    }
    onChange(inToolOrder([...next] as AgentToolId[]))
  }

  return (
    <>
      <PaneHeader
        headingId={headingId}
        title={title}
        summary={summaryText(t, counts)}
        alert={!walking && counts.readableChosen === 0}
        filter={filter}
        onFilter={onFilter}
        showFilter={items.length > GROUP_AFTER}
        ids={items.map((item) => item.id)}
        chosen={ticked}
        onSet={set}
      />
      {stopped ? (
        <Notice
          icon={CircleAlert}
          tone="problem"
          title={t('onboarding.sources.agent.failed')}
          body={walkFailureText(
            t,
            state.phase === 'failed' ? state.code : 'cancelled',
          )}
          onCheck={sources.rerun}
        />
      ) : items.length === 0 && !walking ? (
        <Notice
          icon={ScanSearch}
          tone="plain"
          title={t('onboarding.sources.agent.nothing.title')}
          body={t('onboarding.sources.agent.nothing.body')}
          onCheck={sources.rerun}
        />
      ) : (
        <>
          {items.length > 0 && (
            <SourceList
              label={t('onboarding.sources.agent.listLabel')}
              items={items}
              chosen={ticked}
              filter={filter}
              onSet={set}
              row={(item) => (
                <ToolRow
                  key={item.id}
                  source={item.source}
                  checked={ticked.has(item.id)}
                  onCheckedChange={(on) => set([item.id], on)}
                />
              )}
            />
          )}
          {walking && (
            <Counting
              rows={items.length === 0 ? 3 : 1}
              checked={state.checked}
            />
          )}
        </>
      )}
      {items.length > 0 && (
        <div className="flex flex-col gap-2 border-t bg-muted/30 px-5 py-3.5 text-[13px] leading-relaxed text-muted-foreground">
          <span className="flex items-start gap-3">
            <FolderGit2 className="mt-0.5 size-4 shrink-0" aria-hidden />
            <span>{t('onboarding.sources.agent.gitNote')}</span>
          </span>
          {claude && ticked.has('claude-code') && (
            <span className="flex animate-rise items-start gap-3">
              <TriangleAlert
                className="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-300"
                aria-hidden
              />
              <span>
                {t('onboarding.sources.agent.claudeNote', {
                  count: claude.conversationFiles,
                })}
              </span>
            </span>
          )}
        </div>
      )}
    </>
  )
}

function ToolRow({
  source,
  checked,
  onCheckedChange,
}: {
  source: AgentToolSource
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const id = useId()
  return (
    <li
      data-tool={source.tool}
      className="animate-rise border-b last:border-b-0"
    >
      <label
        htmlFor={id}
        className="flex cursor-pointer items-start gap-3.5 px-5 py-3 transition-colors hover:bg-muted/60"
      >
        <Checkbox
          id={id}
          aria-labelledby={`${id}-name`}
          checked={checked}
          onCheckedChange={(value) => onCheckedChange(value === true)}
          className="mt-1.5 size-[18px] rounded-[5px]"
        />
        <span className="flex size-7 shrink-0 items-center justify-center">
          <ToolIcon id={source.tool} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span id={`${id}-name`} className="font-medium">
            {t(toolName(source.tool))}
          </span>
          {source.roots.map((root) => {
            const label = folderLabel(root.tool, root.path)
            const note = rowNote(root)
            return (
              <span key={root.rootId} className="flex flex-col gap-0.5">
                {label && (
                  <span className="text-xs text-muted-foreground">
                    {t(label)}
                  </span>
                )}
                <span className="font-mono text-[11.5px] [overflow-wrap:anywhere] text-muted-foreground">
                  {root.path}
                </span>
                {note && (
                  <span
                    className={cn(
                      'flex gap-1.5 text-xs leading-snug',
                      root.readable
                        ? 'text-muted-foreground'
                        : 'text-foreground',
                    )}
                  >
                    <TriangleAlert
                      aria-hidden
                      className="mt-px size-3.5 shrink-0 text-brand"
                    />
                    {t(note)}
                  </span>
                )}
              </span>
            )
          })}
        </span>
        {source.readable && (
          <span className="flex shrink-0 flex-col items-end gap-0.5 text-right">
            <span className="font-mono text-xs text-muted-foreground tabular">
              {source.counted
                ? t('onboarding.sources.agent.files', {
                    count: source.conversationFiles,
                  })
                : t('enableAgent.preview.noCount')}
            </span>
            <span className="text-xs text-muted-foreground tabular">
              {source.subagentFiles > 0
                ? t('onboarding.sources.agent.subagents', {
                    count: source.subagentFiles,
                    size: format.bytes(source.bytes),
                  })
                : format.bytes(source.bytes)}
            </span>
          </span>
        )}
      </label>
    </li>
  )
}

/** Skeleton rows for the folders not counted yet, and the running count. */
function Counting({ rows, checked }: { rows: number; checked: number | null }) {
  const { t } = useI18n()
  return (
    <div aria-busy className="flex flex-col">
      {Array.from({ length: rows }, (_, index) => (
        <div
          key={index}
          aria-hidden
          className="flex items-center gap-3.5 border-b px-5 py-3.5 last:border-b-0"
        >
          <Skeleton className="size-[18px] rounded-[5px]" />
          <Skeleton className="size-7 rounded-md" />
          <span className="flex flex-1 flex-col gap-1.5">
            <Skeleton className="h-4 w-28" />
            <Skeleton className="h-3 w-56" />
          </span>
          <Skeleton className="h-4 w-20" />
        </div>
      ))}
      <p
        aria-live="polite"
        className="px-5 py-2.5 text-xs text-muted-foreground tabular"
      >
        {checked === null
          ? t('onboarding.sources.agent.counting')
          : t('onboarding.sources.agent.checked', { count: checked })}
      </p>
    </div>
  )
}

function Notice({
  icon: Icon,
  tone,
  title,
  body,
  onCheck,
}: {
  icon: typeof ScanSearch
  tone: 'plain' | 'problem'
  title: string
  body: string
  onCheck: () => void
}) {
  const { t } = useI18n()
  return (
    <div
      role={tone === 'problem' ? 'alert' : undefined}
      className="m-5 flex animate-rise items-start gap-3 rounded-xl border px-4 py-3.5"
    >
      <Icon
        aria-hidden
        className={cn(
          'mt-0.5 size-4 shrink-0',
          tone === 'problem' ? 'text-destructive' : 'text-muted-foreground',
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">{title}</span>
        <span className="text-[13px] text-muted-foreground">{body}</span>
      </div>
      <Button size="sm" variant="outline" onClick={onCheck}>
        <ScanSearch />
        {t('onboarding.sources.agent.checkAgain')}
      </Button>
    </div>
  )
}
