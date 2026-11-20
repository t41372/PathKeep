/**
 * Folders that may belong to another project (`agent-history.md` §5.8,
 * §5.9): each suggestion says what it rests on (the same remote, a folder
 * that moved, a tool's worktree whose repository cannot be read) and is
 * answered Add / Not this / Later; "Not this" is remembered. The sheet
 * lists every one, unanswered first; project settings lists the ones for
 * that project. Answered ones can be taken back.
 */
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { toolName } from '@/features/data-kinds/tools'
import type { ProjectSuggestion } from '@/lib/backend-client/agent-projects'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { useProjectSuggestions } from '@/lib/queries/agent-projects'
import { tildePath } from './project-words'
import { keepOpenForNotices } from './projects-context'
import { useProjectEdit } from './use-project-edit'

const kindKey: Record<ProjectSuggestion['kind'], MessageKey> = {
  sameRemote: 'agentProjects.suggestions.kind.sameRemote',
  moved: 'agentProjects.suggestions.kind.moved',
  toolWorktree: 'agentProjects.suggestions.kind.toolWorktree',
}

function Detail({
  suggestion,
  home,
}: {
  suggestion: ProjectSuggestion
  home: string | null
}) {
  const { t } = useI18n()
  const evidence = suggestion.evidence
  if (suggestion.kind === 'sameRemote')
    return t('agentProjects.suggestions.detail.sameRemote', {
      remote: evidence.remote ?? '',
    })
  if (suggestion.kind === 'moved')
    return t('agentProjects.suggestions.detail.moved', {
      from: tildePath(evidence.oldPath ?? suggestion.fromFolder, home),
      to: evidence.newPath ? tildePath(evidence.newPath, home) : '',
    })
  return t('agentProjects.suggestions.detail.toolWorktree', {
    tool: evidence.harness ? t(toolName(evidence.harness)) : '',
    branch: evidence.branch ?? '—',
  })
}

export function SuggestionCard({
  suggestion,
  home,
}: {
  suggestion: ProjectSuggestion
  home: string | null
}) {
  const { t } = useI18n()
  const format = useFormat()
  const edit = useProjectEdit()
  const path = tildePath(suggestion.fromFolder, home)
  const answer = (decision: 'added' | 'declined' | 'later') =>
    void edit(
      { kind: 'answer', id: suggestion.id, decision },
      decision === 'added'
        ? {
            title: t('agentProjects.toast.added', {
              path,
              name: suggestion.intoDisplayName,
            }),
            description: t('agentProjects.suggestions.addedBody', {
              count: suggestion.fromConversationCount,
              shown: format.number(suggestion.fromConversationCount),
              name: suggestion.intoDisplayName,
            }),
          }
        : null,
    )
  return (
    <article
      data-suggestion-id={suggestion.id}
      className="flex flex-col gap-2 rounded-lg border bg-card p-3"
    >
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge variant="secondary">{t(kindKey[suggestion.kind])}</Badge>
        {suggestion.decision === 'later' && (
          <Badge variant="outline">
            {t('agentProjects.suggestions.later')}
          </Badge>
        )}
      </div>
      <p className="text-[13px] leading-snug">
        {t('agentProjects.suggestions.question', {
          path,
          name: suggestion.intoDisplayName,
        })}{' '}
        <span className="text-muted-foreground">
          {t('agentProjects.suggestions.conversations', {
            count: suggestion.fromConversationCount,
            shown: format.number(suggestion.fromConversationCount),
          })}
          {suggestion.fromFolderGone && t('agentProjects.suggestions.gone')}
        </span>
      </p>
      <p className="font-mono text-[11.5px] break-all text-muted-foreground">
        <Detail suggestion={suggestion} home={home} />
      </p>
      <div className="flex flex-wrap gap-1.5">
        <Button size="sm" onClick={() => answer('added')}>
          {t('agentProjects.suggestions.add')}
        </Button>
        <Button size="sm" variant="outline" onClick={() => answer('declined')}>
          {t('agentProjects.suggestions.decline')}
        </Button>
        {suggestion.decision !== 'later' && (
          <Button size="sm" variant="ghost" onClick={() => answer('later')}>
            {t('agentProjects.suggestions.laterButton')}
          </Button>
        )}
      </div>
    </article>
  )
}

/** The cards still open, and the answered ones folded below them. */
export function SuggestionList({
  items,
  home,
}: {
  items: ProjectSuggestion[]
  home: string | null
}) {
  const { t } = useI18n()
  const edit = useProjectEdit()
  const [open, setOpen] = useState(false)
  const pending = items.filter(
    (item) => item.decision === 'pending' || item.decision === 'later',
  )
  const handled = items.filter(
    (item) => item.decision === 'added' || item.decision === 'declined',
  )
  return (
    <div className="flex flex-col gap-2.5">
      {pending.length === 0 ? (
        <p className="py-3 text-center text-[12.5px] text-muted-foreground">
          {t('agentProjects.suggestions.allDone')}
        </p>
      ) : (
        pending.map((item) => (
          <SuggestionCard key={item.id} suggestion={item} home={home} />
        ))
      )}
      {handled.length > 0 && (
        <Collapsible open={open} onOpenChange={setOpen}>
          <CollapsibleTrigger className="group flex items-center gap-1 text-[12.5px] text-muted-foreground hover:text-foreground">
            {t('agentProjects.suggestions.handled', { count: handled.length })}
            <ChevronDown
              className="size-3.5 transition-transform duration-200 group-data-[state=open]:rotate-180"
              aria-hidden
            />
          </CollapsibleTrigger>
          <CollapsibleContent>
            <ul className="mt-1.5 flex flex-col gap-1">
              {handled.map((item) => (
                <li
                  key={item.id}
                  data-handled-id={item.id}
                  className="flex items-center gap-2 text-[12.5px]"
                >
                  <span className="shrink-0 font-medium">
                    {item.decision === 'added'
                      ? t('agentProjects.suggestions.wasAdded')
                      : t('agentProjects.suggestions.wasDeclined')}
                  </span>
                  <span className="min-w-0 truncate text-muted-foreground">
                    {t('agentProjects.suggestions.handledRow', {
                      path: tildePath(item.fromFolder, home),
                      name: item.intoDisplayName,
                    })}
                  </span>
                  <Button
                    size="xs"
                    variant="ghost"
                    className="ml-auto"
                    onClick={() =>
                      void edit({
                        kind: 'answer',
                        id: item.id,
                        decision: 'pending',
                      })
                    }
                  >
                    {t('agentProjects.toast.undo')}
                  </Button>
                </li>
              ))}
            </ul>
          </CollapsibleContent>
        </Collapsible>
      )}
    </div>
  )
}

export function SuggestionsSheet({
  open,
  home,
  onClose,
}: {
  open: boolean
  home: string | null
  onClose: () => void
}) {
  const { t } = useI18n()
  const suggestions = useProjectSuggestions(null, open)
  const items = [...(suggestions.data?.items ?? [])]
  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent
        className="w-[540px] gap-0 sm:max-w-[540px]"
        onInteractOutside={keepOpenForNotices}
      >
        <SheetHeader className="border-b">
          <SheetTitle>{t('agentProjects.suggestions.title')}</SheetTitle>
          <SheetDescription>
            {t('agentProjects.suggestions.body')}
          </SheetDescription>
        </SheetHeader>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {suggestions.data ? (
            <SuggestionList items={items} home={home} />
          ) : (
            <p role="status" className="text-[12.5px] text-muted-foreground">
              {suggestions.isError
                ? t('agentProjects.states.failed')
                : t('agentProjects.states.loading')}
            </p>
          )}
        </div>
      </SheetContent>
    </Sheet>
  )
}
